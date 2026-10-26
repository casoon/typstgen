//! User-owned invoice configuration, independent of national profile catalogs.
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{profiles::validate_document, zugferd::decimal::Dec};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvoiceConfig {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    version: u32,
    presentation: Presentation,
    default_tax: String,
    taxes: BTreeMap<String, Tax>,
    #[serde(default)]
    required_fields: Vec<String>,
    #[serde(default)]
    notes: Vec<String>,
    #[serde(default)]
    display_fields: Vec<DisplayField>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Presentation {
    language: String,
    format_locale: String,
    currency: String,
    layout: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tax {
    category: String,
    rate: String,
    exemption_reason: Option<String>,
    exemption_reason_code: Option<String>,
}

#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct DisplayField {
    path: String,
    labels: BTreeMap<String, String>,
}

fn blocked(path: &str, message: impl AsRef<str>) -> Value {
    json!({"status":"blocked","diagnostics":[{
        "path":path,"rule_id":"INVOICE-CONFIG","message":message.as_ref(),"sources":[]
    }]})
}

/// Apply a version-1 configuration to `{company, data, facts?}` and prepare the
/// canonical rendering view. Tax keys are explicit; no jurisdiction is inferred.
/// Supplied amounts remain assertions and must match the calculated amounts.
pub fn prepare_invoice(config: &Value, request: &Value) -> Value {
    let parsed: InvoiceConfig = match serde_json::from_value(config.clone()) {
        Ok(value) => value,
        Err(error) => return blocked("config", error.to_string()),
    };
    if parsed.version != 1 {
        return blocked("config.version", "unsupported configuration version");
    }
    let p = &parsed.presentation;
    if !["de", "en", "es", "fr", "it", "nl", "pt"].contains(&p.language.as_str())
        || !["de-DE", "en-GB", "fr-FR"].contains(&p.format_locale.as_str())
        || !["din", "international"].contains(&p.layout.as_str())
        || p.currency.len() != 3
        || !p.currency.bytes().all(|b| b.is_ascii_uppercase())
    {
        return blocked(
            "config.presentation",
            "unsupported language, format_locale, layout or currency code",
        );
    }
    if !parsed.taxes.contains_key(&parsed.default_tax) {
        return blocked("config.default_tax", "must name a configured tax");
    }
    for (key, tax) in &parsed.taxes {
        let rate = Dec::parse(&tax.rate);
        let valid_rate = rate.is_some_and(|rate| match tax.category.as_str() {
            "S" => rate.is_positive() && rate <= Dec::parse("100").unwrap(),
            "Z" | "E" | "AE" => rate.is_zero(),
            _ => false,
        });
        if key.trim().is_empty() || !valid_rate {
            return blocked(
                &format!("config.taxes.{key}"),
                "S requires a rate above 0 up to 100; Z, E and AE require rate 0",
            );
        }
        if tax.category == "E"
            && !tax
                .exemption_reason
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
            && !tax
                .exemption_reason_code
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
        {
            return blocked(
                &format!("config.taxes.{key}"),
                "exempt tax requires an exemption reason or code",
            );
        }
    }
    for field in &parsed.display_fields {
        if !field
            .labels
            .get(&p.language)
            .is_some_and(|s| !s.trim().is_empty())
        {
            return blocked(
                "config.display_fields",
                "each field needs a label for the presentation language",
            );
        }
    }
    if !request.is_object()
        || !request["company"].is_object()
        || !request["data"].is_object()
        || !request["data"]["items"].is_array()
    {
        return blocked(
            "request",
            "expected company and data objects with a data.items array",
        );
    }
    if request
        .as_object()
        .unwrap()
        .keys()
        .any(|key| !["$schema", "company", "data", "facts"].contains(&key.as_str()))
    {
        return blocked("request", "expected only company, data and optional facts; presentation comes from the configuration");
    }
    let mut input = request.clone();
    for field in ["metadata", "totals", "payment"] {
        if input["data"].get(field).is_none() {
            input["data"][field] = json!({});
        } else if !input["data"][field].is_object() {
            return blocked(&format!("data.{field}"), "expected object");
        }
    }
    for field in ["subtotal", "vat_total", "total", "discount_total"] {
        if let Some(value) = input["data"]["totals"].get(field) {
            if Dec::from_json(value.get("amount").unwrap_or(value)).is_none() {
                return blocked(
                    &format!("data.totals.{field}"),
                    "expected a decimal amount with up to four decimal places",
                );
            }
        }
    }
    if input["data"]["totals"].get("total").is_none() {
        input["data"]["totals"]["total"] = json!({"currency":p.currency});
    } else if !input["data"]["totals"]["total"].is_object() {
        let amount = input["data"]["totals"]["total"].clone();
        input["data"]["totals"]["total"] = json!({"amount":amount,"currency":p.currency});
    } else if input["data"]["totals"]["total"].get("currency").is_none() {
        input["data"]["totals"]["total"]["currency"] = json!(p.currency);
    } else if input["data"]["totals"]["total"]["currency"] != p.currency {
        return blocked(
            "data.totals.total.currency",
            "differs from config.presentation.currency",
        );
    }
    for (index, item) in input["data"]["items"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        if !item.is_object() {
            return blocked(&format!("data.items[{index}]"), "expected object");
        }
        if let Some(value) = item.get("total") {
            if Dec::from_json(value.get("amount").unwrap_or(value)).is_none() {
                return blocked(
                    &format!("data.items[{index}].total"),
                    "expected a decimal amount with up to four decimal places",
                );
            }
        }
        let key = match item.get("tax") {
            None => parsed.default_tax.as_str(),
            Some(Value::String(key)) => key,
            _ => {
                return blocked(
                    &format!("data.items[{index}].tax"),
                    "expected a configured tax key",
                )
            }
        };
        let Some(tax) = parsed.taxes.get(key) else {
            return blocked(
                &format!("data.items[{index}].tax"),
                "unknown configured tax key",
            );
        };
        let code = match tax.category.as_str() {
            "S" => "Standard",
            "Z" => "Zero",
            "E" => "Exempt",
            "AE" => "ReverseCharge",
            _ => unreachable!(),
        };
        let mut vat = json!({"code":code,"percentage":tax.rate});
        if let Some(reason) = &tax.exemption_reason {
            vat["exemption_reason"] = json!(reason);
        }
        if let Some(code) = &tax.exemption_reason_code {
            vat["exemption_reason_code"] = json!(code);
        }
        if let Some(stated) = item.get("vat_rate") {
            if stated != &vat {
                return blocked(
                    &format!("data.items[{index}].vat_rate"),
                    "conflicts with the configured tax; select a tax key instead",
                );
            }
        }
        item["vat_rate"] = vat;
    }
    let mut notes = match input["data"].get("notes") {
        None => Vec::new(),
        Some(value) => match serde_json::from_value::<Vec<String>>(value.clone()) {
            Ok(notes) => notes,
            Err(_) => return blocked("data.notes", "expected an array of strings"),
        },
    };
    notes.extend(parsed.notes);
    input["data"]["notes"] = json!(notes);
    input["selection"] = json!({"profile_id":"configured-invoice","profile_version":"1",
        "language":p.language,"format_locale":p.format_locale,"currency":p.currency,
        "layout":p.layout,"output_profile":"pdf-preview"});
    let catalog = json!({"version":"1","profiles":[{
        "id":"configured-invoice","version":"1","status":"released",
        "languages":[p.language],"format_locales":[p.format_locale],"currencies":[p.currency],
        "layouts":[p.layout],"outputs":["pdf-preview"],"required":parsed.required_fields,
        "display_fields":parsed.display_fields,"rules":[],"sources":[]
    }],"output_adapters":[]});
    let mut report = validate_document(&catalog, &input);
    report["profile"]["status"] = json!("configured");
    report["configuration"] = config.clone();
    report["checks"]["jurisdiction"] = json!("not_assessed");
    report
}
