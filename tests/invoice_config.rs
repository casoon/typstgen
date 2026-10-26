#![cfg(feature = "zugferd")]
use serde_json::{json, Value};
use typstgen::invoice_config::prepare_invoice;

fn config() -> Value {
    json!({"version":1,"presentation":{"language":"en","format_locale":"en-GB","currency":"EUR","layout":"international"},
        "default_tax":"regular","taxes":{"regular":{"category":"S","rate":"19"},
        "reduced":{"category":"S","rate":"7"},"exempt":{"category":"E","rate":"0","exemption_reason":"Configured exemption"},
        "reverse":{"category":"AE","rate":"0","exemption_reason":"Reverse charge"}},
        "required_fields":["company.tax_id","data.metadata.delivery_date"],"notes":["Configured invoice note"],
        "display_fields":[{"path":"company.tax_id","labels":{"en":"Registration"}}]})
}

fn request() -> Value {
    let mut company: Value =
        serde_json::from_str(include_str!("fixtures/zugferd/company.json")).unwrap();
    company["tax_id"] = json!("SYNTHETIC-REGISTRATION");
    let mut data: Value =
        serde_json::from_str(include_str!("fixtures/zugferd/invoice.json")).unwrap();
    data.as_object_mut().unwrap().remove("totals");
    data["metadata"]["delivery_date"] = json!("2026-10-26");
    data["recipient"]["vat_id"] = json!("DE987654321");
    data["items"] = json!([
        {"description":"First line","quantity":"3","unit":"piece","unit_code":"C62","unit_price":{"amount":"0.3333","currency":"EUR"}},
        {"description":"Reduced line","quantity":"1","unit":"piece","unit_code":"C62","unit_price":{"amount":"100","currency":"EUR"},"tax":"reduced"}]);
    json!({"company":company,"data":data})
}

#[test]
fn configuration_builds_mixed_taxes_totals_notes_and_xml_from_sparse_input() {
    let report = prepare_invoice(&config(), &request());
    assert_eq!(report["status"], "ready_for_preview", "{report:#}");
    assert_eq!(report["normalized"]["grand_total"], "108.19");
    assert_eq!(report["data"]["items"][0]["unit_price"]["amount"], "0.3333");
    assert_eq!(report["data"]["items"][0]["total"]["amount"], "1");
    assert_eq!(report["data"]["profile_fields"][0]["label"], "Registration");
    assert_eq!(report["checks"]["jurisdiction"], "not_assessed");
    assert_eq!(report["profile"]["status"], "configured");
    let xml = typstgen::zugferd::xml_from_docgen(
        &report["data"].to_string(),
        &report["company"].to_string(),
    )
    .unwrap();
    assert!(xml.contains("108.19"));
    assert!(xml.contains("Configured invoice note"));
}

#[test]
fn changing_configuration_changes_rates_currency_and_presentation_without_country_code() {
    let mut config = config();
    config["taxes"]["regular"]["rate"] = json!("20");
    config["taxes"]["reduced"]["rate"] = json!("10");
    config["presentation"]["currency"] = json!("GBP");
    config["presentation"]["layout"] = json!("din");
    let mut request = request();
    for item in request["data"]["items"].as_array_mut().unwrap() {
        item["unit_price"]["currency"] = json!("GBP");
    }
    let report = prepare_invoice(&config, &request);
    assert_eq!(report["status"], "ready_for_preview", "{report:#}");
    assert_eq!(report["normalized"]["grand_total"], "111.2");
    assert_eq!(report["normalized"]["currency"], "GBP");
    assert_eq!(report["selection"]["layout"], "din");
}

#[test]
fn exemptions_and_reverse_charge_use_explicit_categories_and_require_buyer_vat_id() {
    let mut request = request();
    request["data"]["items"][0]["tax"] = json!("exempt");
    request["data"]["items"][1]["tax"] = json!("reverse");
    let report = prepare_invoice(&config(), &request);
    assert_eq!(report["status"], "ready_for_preview", "{report:#}");
    assert_eq!(report["normalized"]["tax_total"], "0");
    assert_eq!(report["normalized"]["lines"][0]["tax"]["category"], "E");
    assert_eq!(report["normalized"]["lines"][1]["tax"]["category"], "AE");
    request["data"]["recipient"]
        .as_object_mut()
        .unwrap()
        .remove("vat_id");
    assert_eq!(prepare_invoice(&config(), &request)["status"], "blocked");
}

#[test]
fn required_fields_stated_amounts_unknown_taxes_and_conflicting_rates_block_preparation() {
    for change in 0..4 {
        let mut request = request();
        match change {
            0 => {
                request["company"].as_object_mut().unwrap().remove("tax_id");
            }
            1 => request["data"]["items"][0]["total"] = json!({"amount":"99","currency":"EUR"}),
            2 => request["data"]["items"][0]["tax"] = json!("unknown"),
            _ => {
                request["data"]["items"][0]["vat_rate"] =
                    json!({"code":"Standard","percentage":"21"})
            }
        }
        assert_eq!(
            prepare_invoice(&config(), &request)["status"],
            "blocked",
            "case {change}"
        );
    }
}

#[test]
fn malformed_configuration_and_input_are_rejected_without_panics() {
    for input in [
        Value::Null,
        json!({}),
        json!({"company":{},"data":{"items":[null]}}),
        json!({"company":{},"data":{"items":[],"totals":null}}),
    ] {
        assert_eq!(prepare_invoice(&config(), &input)["status"], "blocked");
    }
    for (path, value) in [
        ("version", json!(2)),
        ("default_tax", json!("missing")),
        ("unknown", json!(true)),
    ] {
        let mut config = config();
        config[path] = value;
        assert_eq!(prepare_invoice(&config, &request())["status"], "blocked");
    }
    let mut config = config();
    config["taxes"]["regular"]["rate"] = json!("-1");
    assert_eq!(prepare_invoice(&config, &request())["status"], "blocked");
}
