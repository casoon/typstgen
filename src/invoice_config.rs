//! User-owned invoice configuration, independent of national profile catalogs.
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    profiles::{validate_document, validate_document_with_model},
    zugferd::{decimal::Dec, model::Invoice},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvoiceConfig {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    version: u32,
    #[serde(default = "vat_model")]
    tax_model: String,
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
    #[serde(default = "a4")]
    paper: String,
    #[serde(default = "auto_currency")]
    currency_display: String,
    tax_label: Option<String>,
}

fn vat_model() -> String {
    "vat".to_owned()
}
fn a4() -> String {
    "a4".to_owned()
}
fn auto_currency() -> String {
    "auto".to_owned()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tax {
    #[serde(default)]
    category: String,
    #[serde(default)]
    rate: String,
    exemption_reason: Option<String>,
    exemption_reason_code: Option<String>,
    components: Option<Vec<TaxComponent>>,
}

#[derive(Clone, Deserialize, serde::Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct TaxComponent {
    id: String,
    label: String,
    rate: String,
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
    let sales_tax = parsed.tax_model == "sales-tax";
    if !["vat", "sales-tax"].contains(&parsed.tax_model.as_str()) {
        return blocked("config.tax_model", "expected vat or sales-tax");
    }
    if !["de", "en", "es", "fr", "it", "nl", "pt"].contains(&p.language.as_str())
        || ![
            "de-DE", "en-GB", "en-US", "fr-FR", "es-ES", "it-IT", "nl-NL", "pt-PT",
        ]
        .contains(&p.format_locale.as_str())
        || !["din", "international"].contains(&p.layout.as_str())
        || !["a4", "us-letter"].contains(&p.paper.as_str())
        || !["auto", "code", "symbol"].contains(&p.currency_display.as_str())
        || (p.currency_display == "symbol" && p.currency != "USD")
        || p.tax_label
            .as_ref()
            .is_some_and(|label| label.trim().is_empty())
        || p.currency.len() != 3
        || !p.currency.bytes().all(|b| b.is_ascii_uppercase())
    {
        return blocked(
            "config.presentation",
            "unsupported language, format_locale, layout, paper, currency display or currency code",
        );
    }
    if !parsed.taxes.contains_key(&parsed.default_tax) {
        return blocked("config.default_tax", "must name a configured tax");
    }
    let mut component_definitions = BTreeMap::new();
    for (key, tax) in &parsed.taxes {
        if sales_tax {
            let Some(components) = &tax.components else {
                return blocked(
                    &format!("config.taxes.{key}.components"),
                    "sales-tax requires an array of additive components (empty for untaxed items)",
                );
            };
            if key.trim().is_empty()
                || !tax.category.is_empty()
                || !tax.rate.is_empty()
                || tax.exemption_reason.is_some()
                || tax.exemption_reason_code.is_some()
            {
                return blocked(
                    &format!("config.taxes.{key}"),
                    "sales-tax uses components, not VAT categories or exemption fields",
                );
            }
            let mut seen = std::collections::BTreeSet::new();
            for component in components {
                if component.id.trim().is_empty()
                    || component.label.trim().is_empty()
                    || !seen.insert(&component.id)
                    || !Dec::parse(&component.rate)
                        .is_some_and(|rate| rate >= Dec::ZERO && rate <= Dec::parse("100").unwrap())
                {
                    return blocked(
                        &format!("config.taxes.{key}.components"),
                        "each component needs a unique id, a label and a rate from 0 to 100",
                    );
                }
                if component_definitions
                    .insert(component.id.clone(), component.clone())
                    .is_some_and(|previous| previous != *component)
                {
                    return blocked(
                        &format!("config.taxes.{key}.components"),
                        "a shared component id must have the same label and rate in every tax key",
                    );
                }
            }
            continue;
        }
        if tax.components.is_some() {
            return blocked(
                &format!("config.taxes.{key}.components"),
                "VAT taxes use category and rate, not sales-tax components",
            );
        }
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
    for field in [
        "subtotal",
        "vat_total",
        "tax_total",
        "total",
        "discount_total",
    ] {
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
    let mut line_components = Vec::new();
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
        if sales_tax {
            if item.get("vat_rate").is_some() {
                return blocked(
                    &format!("data.items[{index}].vat_rate"),
                    "sales-tax items select component sets through tax; VAT input is not accepted",
                );
            }
            line_components.push(tax.components.clone().expect("validated components"));
            // The existing model validates net amounts, parties, dates and payment.
            // Its temporary zero-VAT view is replaced by component totals below.
            item["vat_rate"] = json!({"code":"Zero", "percentage":"0"});
            continue;
        }
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
        "layout":p.layout,"paper":p.paper,"currency_display":p.currency_display,"output_profile":"pdf-preview"});
    if let Some(label) = &p.tax_label {
        input["selection"]["tax_label"] = json!(label);
    }
    let catalog = json!({"version":"1","profiles":[{
        "id":"configured-invoice","version":"1","status":"released",
        "languages":[p.language],"format_locales":[p.format_locale],"currencies":[p.currency],
        "layouts":[p.layout],"outputs":["pdf-preview"],"required":parsed.required_fields,
        "display_fields":parsed.display_fields,"rules":[],"sources":[]
    }],"output_adapters":[]});
    let mut report = if sales_tax {
        prepare_sales_tax(&catalog, &input, &line_components)
    } else {
        validate_document(&catalog, &input)
    };
    report["profile"]["status"] = json!("configured");
    report["configuration"] = config.clone();
    report["checks"]["jurisdiction"] = json!("not_assessed");
    report
}

fn prepare_sales_tax(catalog: &Value, input: &Value, components: &[Vec<TaxComponent>]) -> Value {
    if input["data"]
        .get("tax_model")
        .is_some_and(|v| v != "sales-tax")
        || input["data"]["totals"].get("vat_total").is_some()
        || input["data"]["totals"].get("vat_breakdown").is_some()
    {
        return blocked(
            "data.totals",
            "sales-tax invoices use tax_total and tax_breakdown, not VAT fields",
        );
    }
    let mut base = input["data"].clone();
    base.as_object_mut().unwrap().remove("tax_model");
    base["totals"]["total"] = json!({"currency":input["selection"]["currency"]});
    let mut report = validate_document_with_model(
        catalog,
        input,
        Invoice::from_sales_tax_base(&base, &input["company"]),
    );
    if report["status"] == "blocked" {
        return report;
    }
    let mut bases: BTreeMap<String, (TaxComponent, Dec)> = BTreeMap::new();
    for (index, taxes) in components.iter().enumerate() {
        let amount = Dec::from_json(&report["normalized"]["lines"][index]["net_amount"])
            .expect("canonical amount");
        let rate: Dec = taxes
            .iter()
            .map(|c| Dec::parse(&c.rate).expect("validated rate"))
            .sum();
        for component in taxes {
            let group = bases
                .entry(component.id.clone())
                .or_insert_with(|| (component.clone(), Dec::ZERO));
            group.1 = group.1 + amount;
        }
        let line = &mut report["normalized"]["lines"][index];
        line.as_object_mut().unwrap().remove("tax");
        line["tax_components"] = json!(taxes);
        let item = &mut report["data"]["items"][index];
        item.as_object_mut().unwrap().remove("vat_rate");
        item["tax_components"] = json!(taxes);
        item["tax_rate"] = json!(rate.to_string());
    }
    let mut tax_total = Dec::ZERO;
    let mut breakdown = Vec::new();
    for (component, basis) in bases.values() {
        let amount = basis.percent_money(Dec::parse(&component.rate).expect("validated rate"));
        tax_total = tax_total + amount;
        breakdown.push(
            json!({"id":component.id,"label":component.label,"rate":component.rate,
            "basis":basis.to_string(),"amount":amount.to_string()}),
        );
    }
    let subtotal = Dec::from_json(&report["normalized"]["line_total"]).expect("canonical subtotal");
    let total = subtotal + tax_total;
    let currency = input["selection"]["currency"]
        .as_str()
        .expect("validated currency");
    for (field, computed) in [("tax_total", tax_total), ("total", total)] {
        let stated = &input["data"]["totals"][field];
        if stated.get("currency").is_some_and(|v| v != currency)
            || Dec::from_json(stated.get("amount").unwrap_or(stated))
                .is_some_and(|amount| amount.round2() != computed)
        {
            return blocked(
                &format!("data.totals.{field}"),
                "stated amount or currency differs from configured sales-tax calculation",
            );
        }
    }
    if input["data"]["totals"]
        .get("tax_breakdown")
        .is_some_and(|value| value != &json!(breakdown))
    {
        return blocked(
            "data.totals.tax_breakdown",
            "stated components differ from the configured sales-tax breakdown",
        );
    }
    report["normalized"]["tax_model"] = json!("sales-tax");
    report["normalized"]["tax_total"] = json!(tax_total.to_string());
    report["normalized"]["grand_total"] = json!(total.to_string());
    report["normalized"]["breakdown"] = json!(breakdown);
    report["data"]["tax_model"] = json!("sales-tax");
    let totals = &mut report["data"]["totals"];
    totals.as_object_mut().unwrap().remove("vat_total");
    totals.as_object_mut().unwrap().remove("vat_breakdown");
    totals["tax_total"] = json!({"amount":tax_total.to_string(),"currency":currency});
    totals["total"] = json!({"amount":total.to_string(),"currency":currency});
    totals["tax_breakdown"] = json!(breakdown);
    if report["normalized"]["payment"].is_null() {
        let bank = input["data"]["payment"]
            .get("bank_account")
            .unwrap_or(&input["company"]["bank_account"]);
        if bank.get("account_number").is_some() && bank.get("routing_number").is_some() {
            report["normalized"]["payment"] = bank.clone();
            report["data"]["payment"]["bank_account"] = bank.clone();
        }
    }
    report["checks"]["en16931"] = json!("not_applicable");
    report
}
