#![cfg(feature = "zugferd")]
use serde_json::{json, Value};
use typstgen::profiles::{list_document_profiles, resolve_document_profile, validate_document};

fn catalog() -> Value {
    json!({"version":"1","profiles":[
        {"id":"base","version":"1","status":"abstract","languages":["de","en"],
         "format_locales":["de-DE","en-GB"],"currencies":["EUR"],"layouts":["din","international"],
         "outputs":["pdf-preview"],"required":["facts.confirmed"],"rules":[
             {"id":"DOMESTIC","path":"facts.country","op":"equals","value":"DE"}],
         "sources":[{"url":"https://example.invalid/rule","checked_on":"2026-10-26"}]},
        {"id":"de-standard","version":"1","extends":{"id":"base","version":"1"},"status":"released",
         "country":"DE","tax_case":"standard","required":["facts.evidence_refs"],
         "rules":[{"id":"RATE","path":"data.items.*.vat_rate","op":"mapping","key":"code","value_key":"percentage","mapping":{"Standard":["19"],"Reduced":["7"]}}]},
        {"id":"fr-preview","version":"1","extends":{"id":"base","version":"1"},"status":"draft","country":"FR","tax_case":"standard"}
    ]})
}
fn selection() -> Value {
    json!({"profile_id":"de-standard","profile_version":"1","language":"en","format_locale":"en-GB",
        "currency":"EUR","layout":"international","output_profile":"pdf-preview"})
}
fn request() -> Value {
    json!({"selection":selection(),"data":serde_json::from_str::<Value>(include_str!("fixtures/zugferd/invoice.json")).unwrap(),
        "company":serde_json::from_str::<Value>(include_str!("fixtures/zugferd/company.json")).unwrap(),
        "facts":{"confirmed":true,"country":"DE","evidence_refs":["synthetic"]}})
}
#[test]
fn selection_keeps_rules_independent_of_language_and_rejects_unknown_outputs() {
    let mut input = selection();
    let english = resolve_document_profile(&catalog(), &input);
    assert_eq!(english["status"], "resolved");
    assert_eq!(english["profile"]["country"], "DE");
    input["language"] = json!("de");
    assert_eq!(
        resolve_document_profile(&catalog(), &input)["profile"],
        english["profile"]
    );
    input["output_profile"] = json!("xrechnung");
    assert_eq!(
        resolve_document_profile(&catalog(), &input)["status"],
        "blocked"
    );
    input["profile_id"] = json!("missing");
    assert!(resolve_document_profile(&catalog(), &input)["diagnostics"]
        .to_string()
        .contains("PROFILE-IDENTITY"));
    let listed = list_document_profiles(&catalog(), false);
    assert_eq!(listed["profiles"][0]["country"], "DE");
    assert_eq!(listed["profiles"][0]["outputs"], json!(["pdf-preview"]));
    assert_eq!(listed["profiles"].as_array().unwrap().len(), 1);
}
#[test]
fn missing_facts_have_paths_rules_and_sources_without_invented_defaults() {
    let mut input = request();
    input["facts"] = json!({"country":"FR"});
    let report = validate_document(&catalog(), &input);
    assert_eq!(report["status"], "blocked");
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|d| d["path"] == "facts.confirmed"));
    assert!(diagnostics
        .iter()
        .any(|d| d["rule_id"] == "DOMESTIC" && !d["sources"].as_array().unwrap().is_empty()));
    assert_eq!(report["checks"]["transmission"], "not_run");
    assert_eq!(report["checks"]["en16931"], "not_run");
}
#[test]
fn inherited_required_fields_cannot_be_removed_and_rule_conflicts_are_explicit() {
    let mut input = catalog();
    input["profiles"][1]["rules"] =
        json!([{"id":"DOMESTIC","path":"facts.country","op":"equals","value":"FR"}]);
    assert_eq!(
        resolve_document_profile(&input, &selection())["status"],
        "blocked"
    );
    input["profiles"][1]["replace_rules"] = json!(["DOMESTIC"]);
    let resolved = resolve_document_profile(&input, &selection());
    assert_eq!(resolved["status"], "resolved");
    assert!(resolved["profile"]["required"]
        .as_array()
        .unwrap()
        .contains(&json!("facts.confirmed")));
    input["profiles"][0]["extends"] = json!({"id":"de-standard","version":"1"});
    assert!(
        resolve_document_profile(&input, &selection())["diagnostics"]
            .to_string()
            .contains("cycle")
    );
}
#[test]
fn drafts_require_opt_in_and_external_checks_stay_not_run() {
    let mut input = selection();
    input["profile_id"] = json!("fr-preview");
    assert_eq!(
        resolve_document_profile(&catalog(), &input)["status"],
        "blocked"
    );
    input["allow_draft"] = json!(true);
    assert_eq!(
        resolve_document_profile(&catalog(), &input)["status"],
        "resolved"
    );
    let report = validate_document(&catalog(), &request());
    assert_eq!(report["status"], "ready_for_preview", "{report}");
    assert!(report["normalized"]["issue_date"]
        .as_str()
        .unwrap()
        .contains('-'));
    assert_eq!(report["normalized"]["lines"][0]["tax"]["category"], "S");
}
#[test]
fn a_rate_change_and_mismatched_totals_block_the_document() {
    let mut input = request();
    input["data"]["items"][0]["vat_rate"]["percentage"] = json!("20");
    let report = validate_document(&catalog(), &input);
    assert_eq!(report["status"], "blocked");
    assert!(report["diagnostics"].to_string().contains("RATE"));
    assert_eq!(report["checks"]["arithmetic"], "failed");
}

#[test]
fn zero_rate_categories_remain_separate_in_the_shared_model() {
    let mut input = request();
    let mut lines = Vec::new();
    for (index, code) in ["Zero", "Exempt", "ReverseCharge"].iter().enumerate() {
        lines.push(json!({"position":index+1,"description":"Synthetic zero-tax item","quantity":"1","unit":"stunde",
            "unit_price":{"amount":"100","currency":"EUR"},"total":{"amount":"100","currency":"EUR"},
            "vat_rate":{"code":code,"percentage":"0","exemption_reason":"Synthetic reason"}}));
    }
    input["data"]["items"] = json!(lines);
    input["data"]["recipient"]["vat_id"] = json!("DE987654321");
    input["data"]["totals"] = json!({"subtotal":{"amount":"300","currency":"EUR"},"vat_total":{"amount":"0","currency":"EUR"},"total":{"amount":"300","currency":"EUR"}});
    let mut profiles = catalog();
    profiles["profiles"][1]["rules"] = json!([]);
    let report = validate_document(&profiles, &input);
    assert_eq!(report["status"], "ready_for_preview", "{report}");
    let categories: std::collections::BTreeSet<_> = report["normalized"]["breakdown"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["tax"]["category"].as_str().unwrap())
        .collect();
    assert_eq!(
        categories,
        std::collections::BTreeSet::from(["Z", "E", "AE"])
    );
    input["data"]["recipient"]
        .as_object_mut()
        .unwrap()
        .remove("vat_id");
    let report = validate_document(&profiles, &input);
    assert_eq!(report["status"], "blocked");
    assert!(report["diagnostics"]
        .to_string()
        .contains("recipient.vat_id"));
}

#[test]
fn mismatched_tax_bases_currencies_and_impossible_dates_fail() {
    let mut input = request();
    input["data"]["totals"]["vat_breakdown"] = json!([{"rate":{"code":"Standard","percentage":"19"},"base":{"amount":"1"},"amount":{"amount":"456"}}]);
    assert_eq!(validate_document(&catalog(), &input)["status"], "blocked");
    let mut input = request();
    input["data"]["metadata"]["invoice_date"] = json!("2026-02-30");
    assert_eq!(validate_document(&catalog(), &input)["status"], "blocked");
    let mut input = request();
    input["data"]["items"][0]["unit_price"]["currency"] = json!("USD");
    assert_eq!(validate_document(&catalog(), &input)["status"], "blocked");
}

#[test]
fn optional_dates_cannot_be_invalid_or_disagree_with_payment() {
    for (field, value) in [("due_date", "2026-02-30"), ("delivery_date", "2026-02-30")] {
        let mut input = request();
        if field == "due_date" {
            input["data"]["payment"][field] = json!(value);
        } else {
            input["data"]["metadata"][field] = json!(value);
        }
        let report = validate_document(&catalog(), &input);
        assert_eq!(report["status"], "blocked", "{report}");
        assert!(report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["path"].as_str().unwrap().ends_with(field)));
    }
    let mut input = request();
    input["data"]["metadata"]["due_date"] = json!("2026-11-30");
    let report = validate_document(&catalog(), &input);
    assert_eq!(report["status"], "blocked", "{report}");
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["path"] == "data.metadata.due_date"));
}

#[test]
fn company_diagnostics_point_to_company_fields() {
    let mut input = request();
    input["company"].as_object_mut().unwrap().remove("name");
    input["company"]["address"]
        .as_object_mut()
        .unwrap()
        .remove("street");
    let report = validate_document(&catalog(), &input);
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|d| d["path"] == "company.name"));
    assert!(diagnostics
        .iter()
        .any(|d| d["path"] == "company.address.street"));
}

#[test]
fn unsupported_discounts_are_not_silently_ignored() {
    let mut input = request();
    input["data"]["totals"]["discount_total"] = json!({"amount":"10", "currency":"EUR"});
    let report = validate_document(&catalog(), &input);
    assert_eq!(report["status"], "blocked", "{report}");
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["path"] == "data.totals.discount_total"));
}
