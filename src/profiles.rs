//! Data-driven document selection and validation. The catalog belongs to the template package;
//! country rules are not hardcoded in the compiler. External validation and delivery stay separate.
use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde_json::{json, Value};

use crate::zugferd::{decimal::Dec, model::Invoice};

fn array(value: &Value, field: &str) -> Vec<Value> {
    value
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn diagnostic(path: &str, rule: &str, message: &str, sources: &Value) -> Value {
    json!({"path": path, "rule_id": rule, "message": message, "sources": sources})
}

/// Resolve a versioned profile, rejecting cycles and implicit replacement of rules.
fn expand(
    catalog: &Value,
    id: &str,
    version: &str,
    active: &mut BTreeSet<String>,
) -> Result<Value, String> {
    let key = format!("{id}@{version}");
    if !active.insert(key.clone()) {
        return Err(format!("profile inheritance cycle: {key}"));
    }
    let profiles = catalog
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or("catalog.profiles: expected array")?;
    let matches: Vec<_> = profiles
        .iter()
        .filter(|p| p["id"] == id && p["version"] == version)
        .collect();
    if matches.len() != 1 {
        return Err(format!("unknown or duplicate profile: {key}"));
    }
    let child = matches[0];
    let mut resolved = if let Some(parent) = child.get("extends") {
        expand(
            catalog,
            parent["id"].as_str().ok_or("extends.id: missing")?,
            parent["version"]
                .as_str()
                .ok_or("extends.version: missing")?,
            active,
        )?
    } else {
        json!({})
    };
    let mut required = array(&resolved, "required");
    for field in array(child, "required") {
        if !required.contains(&field) {
            required.push(field);
        }
    }
    let mut rules = array(&resolved, "rules");
    let replacements = array(child, "replace_rules");
    for replacement in &replacements {
        if !rules.iter().any(|r| r.get("id") == Some(replacement)) {
            return Err(format!("unknown replacement rule: {replacement}"));
        }
    }
    for rule in array(child, "rules") {
        let rule_id = rule
            .get("id")
            .and_then(Value::as_str)
            .ok_or("rules[].id: missing")?;
        if let Some(position) = rules.iter().position(|r| r["id"] == rule_id) {
            if !replacements.iter().any(|r| r == rule_id) {
                return Err(format!(
                    "rule conflict: {rule_id}; declare replace_rules explicitly"
                ));
            }
            rules[position] = rule;
        } else {
            rules.push(rule);
        }
    }
    let mut sources = array(&resolved, "sources");
    for source in array(child, "sources") {
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    for (key, value) in child.as_object().ok_or("profile: expected object")? {
        resolved[key] = value.clone();
    }
    resolved["required"] = json!(required);
    resolved["rules"] = json!(rules);
    resolved["sources"] = json!(sources);
    active.remove(&key);
    Ok(resolved)
}

/// List declared capabilities and their release status. Draft entries are explicit opt-in.
pub fn list_document_profiles(catalog: &Value, include_drafts: bool) -> Value {
    let mut profiles = Vec::new();
    let mut errors = Vec::new();
    for profile in array(catalog, "profiles") {
        if !include_drafts && profile["status"] != "released" {
            continue;
        }
        let id = profile["id"].as_str().unwrap_or("");
        let version = profile["version"].as_str().unwrap_or("");
        match expand(catalog, id, version, &mut BTreeSet::new()) {
            Ok(profile) => profiles.push(profile),
            Err(message) => errors.push(diagnostic(
                "catalog.profiles",
                "PROFILE-IDENTITY",
                &message,
                &json!([]),
            )),
        }
    }
    json!({"status":if errors.is_empty() {"listed"} else {"blocked"}, "catalog_version":catalog["version"],
        "profiles":profiles,"diagnostics":errors})
}

/// Resolve only explicit profile identities. Language never changes the rule jurisdiction.
pub fn resolve_document_profile(catalog: &Value, selection: &Value) -> Value {
    let mut errors = Vec::new();
    let mut profile = Value::Null;
    let id = selection["profile_id"].as_str();
    let version = selection["profile_version"].as_str();
    if let (Some(id), Some(version)) = (id, version) {
        match expand(catalog, id, version, &mut BTreeSet::new()) {
            Ok(value) => profile = value,
            Err(message) => errors.push(diagnostic(
                "selection.profile_id",
                "PROFILE-IDENTITY",
                &message,
                &json!([]),
            )),
        }
    } else {
        errors.push(diagnostic(
            "selection",
            "PROFILE-IDENTITY",
            "profile_id and profile_version are required",
            &json!([]),
        ));
    }
    if let Some(fields) = selection.as_object() {
        for field in fields.keys() {
            if ![
                "profile_id",
                "profile_version",
                "language",
                "format_locale",
                "currency",
                "layout",
                "output_profile",
                "allow_draft",
            ]
            .contains(&field.as_str())
            {
                errors.push(diagnostic(
                    &format!("selection.{field}"),
                    "PROFILE-FIELD",
                    "unknown selection field",
                    &json!([]),
                ));
            }
        }
    }
    if !profile.is_null() {
        if profile["status"] == "abstract" || profile["status"] == "unsupported" {
            errors.push(diagnostic(
                "selection.profile_id",
                "PROFILE-STATUS",
                "abstract or unsupported profile cannot be selected",
                &profile["sources"],
            ));
        }
        if profile["status"] != "released" && selection["allow_draft"] != true {
            errors.push(diagnostic(
                "selection.profile_id",
                "PROFILE-STATUS",
                "profile is not released; preview requires allow_draft",
                &profile["sources"],
            ));
        }
        for (field, choices) in [
            ("language", "languages"),
            ("format_locale", "format_locales"),
            ("currency", "currencies"),
            ("layout", "layouts"),
            ("output_profile", "outputs"),
        ] {
            if !array(&profile, choices).contains(&selection[field]) {
                errors.push(diagnostic(
                    &format!("selection.{field}"),
                    "PROFILE-CAPABILITY",
                    "missing or unsupported choice",
                    &profile["sources"],
                ));
            }
        }
        if profile["status"] != "released" && selection["output_profile"] != "pdf-preview" {
            errors.push(diagnostic(
                "selection.output_profile",
                "PROFILE-DRAFT-OUTPUT",
                "draft profiles only produce previews",
                &profile["sources"],
            ));
        }
    }
    json!({"status": if errors.is_empty() {"resolved"} else {"blocked"}, "selection": selection,
        "profile": profile, "diagnostics": errors})
}

fn values<'a>(node: &'a Value, parts: &[&str], path: &str) -> Vec<(String, &'a Value)> {
    if parts.is_empty() {
        return vec![(path.to_owned(), node)];
    }
    let (head, tail) = (parts[0], &parts[1..]);
    if head == "*" {
        return node
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .flat_map(|(i, item)| values(item, tail, &format!("{path}[{i}]")))
                    .collect()
            })
            .unwrap_or_default();
    }
    let next = if path.is_empty() {
        head.to_owned()
    } else {
        format!("{path}.{head}")
    };
    values(node.get(head).unwrap_or(&Value::Null), tail, &next)
}

fn present(v: &Value) -> bool {
    !v.is_null()
        && v.as_str().is_none_or(|s| !s.trim().is_empty())
        && v.as_array().is_none_or(|a| !a.is_empty())
}
fn equal(left: &Value, right: &Value) -> bool {
    left == right
        || Dec::from_json(left)
            .zip(Dec::from_json(right))
            .is_some_and(|(a, b)| a == b)
}
fn iso(v: &Value) -> Option<NaiveDate> {
    let text = v.get("date").unwrap_or(v).as_str()?;
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Evaluate package rules and the existing exact-decimal invoice model. No external validator
/// run, PDF/A check or transmission is implied by a successful business-rule result.
pub fn validate_document(catalog: &Value, request: &Value) -> Value {
    let resolved = resolve_document_profile(catalog, &request["selection"]);
    let mut errors = array(&resolved, "diagnostics");
    let profile = &resolved["profile"];
    let sources = &profile["sources"];
    if !profile.is_null() {
        for field in array(profile, "required") {
            if let Some(path) = field.as_str() {
                let found = values(request, &path.split('.').collect::<Vec<_>>(), "");
                if found.is_empty() {
                    errors.push(diagnostic(
                        path,
                        "REQUIRED",
                        "required collection is empty",
                        sources,
                    ));
                }
                for (path, value) in found {
                    if !present(value) {
                        errors.push(diagnostic(
                            &path,
                            "REQUIRED",
                            "required value is missing",
                            sources,
                        ));
                    }
                }
            }
        }
        for rule in array(profile, "rules") {
            let id = rule["id"].as_str().unwrap_or("INVALID-RULE");
            let path = rule["path"].as_str().unwrap_or("");
            let found = values(request, &path.split('.').collect::<Vec<_>>(), "");
            for (actual, value) in found {
                let passed = match rule["op"].as_str() {
                    Some("equals") => equal(value, &rule["value"]),
                    Some("contains") => value
                        .as_str()
                        .zip(rule["value"].as_str())
                        .is_some_and(|(text, part)| text.contains(part)),
                    Some("mapping") => value
                        .get(rule["key"].as_str().unwrap_or(""))
                        .and_then(Value::as_str)
                        .and_then(|key| rule["mapping"].get(key))
                        .and_then(Value::as_array)
                        .is_some_and(|choices| {
                            choices.iter().any(|choice| {
                                equal(&value[rule["value_key"].as_str().unwrap_or("")], choice)
                            })
                        }),
                    Some("one_of") => array(&rule, "values")
                        .iter()
                        .any(|candidate| equal(value, candidate)),
                    Some("required") => present(value),
                    Some("within") => {
                        let start = rule["start_path"].as_str().and_then(|p| {
                            values(request, &p.split('.').collect::<Vec<_>>(), "")
                                .first()
                                .and_then(|(_, v)| iso(v))
                        });
                        let end = rule["end_path"].as_str().and_then(|p| {
                            values(request, &p.split('.').collect::<Vec<_>>(), "")
                                .first()
                                .and_then(|(_, v)| iso(v))
                        });
                        iso(value)
                            .zip(start)
                            .zip(end)
                            .is_some_and(|((date, start), end)| start <= date && date <= end)
                    }
                    _ => false,
                };
                if !passed {
                    errors.push(diagnostic(
                        &actual,
                        id,
                        rule["message"]
                            .as_str()
                            .unwrap_or("rule failed or unknown operation"),
                        sources,
                    ));
                }
            }
        }
    }
    if !profile.is_null() {
        let date = iso(&request["data"]["metadata"]["delivery_date"]);
        if let (Some(start), Some(end)) = (
            iso(&profile["effective_from"]),
            iso(&profile["effective_until"]),
        ) {
            if date.is_none_or(|date| date < start || date > end) {
                errors.push(diagnostic(
                    "data.metadata.delivery_date",
                    "PROFILE-VALIDITY",
                    "supply date is outside the reviewed profile period",
                    sources,
                ));
            }
        }
    }
    // A specified-but-unimplemented adapter still gives actionable missing-field
    // paths, while the unsupported output diagnostic remains blocking.
    if let Some(adapter) = array(catalog, "output_adapters")
        .iter()
        .find(|adapter| adapter["id"] == request["selection"]["output_profile"])
    {
        for field in array(adapter, "required_field_paths") {
            if let Some(path) = field.as_str() {
                let found = values(request, &path.split('.').collect::<Vec<_>>(), "");
                for (actual, value) in found {
                    if !present(value) {
                        errors.push(diagnostic(
                            &actual,
                            "OUTPUT-REQUIRED",
                            "required by the requested output adapter",
                            &json!([]),
                        ));
                    }
                }
            }
        }
    }
    let model = Invoice::from_docgen(&request["data"], &request["company"]);
    let arithmetic = if model.is_ok() { "passed" } else { "failed" };
    let normalized = match model {
        Ok(invoice) => {
            let mut value = serde_json::to_value(invoice).expect("serializable invoice model");
            for field in ["issue_date", "due_date", "delivery_date"] {
                if let Some(date) = value[field].as_str() {
                    if date.len() == 8 {
                        value[field] =
                            json!(format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]));
                    }
                }
            }
            if let Some(dates) = value["billing_period"].as_array_mut() {
                for date in dates {
                    if let Some(text) = date.as_str().filter(|s| s.len() == 8) {
                        *date = json!(format!("{}-{}-{}", &text[..4], &text[4..6], &text[6..]));
                    }
                }
            }
            value
        }
        Err(problems) => {
            for problem in problems {
                let path = problem.split([':', ' ']).next().unwrap_or("data");
                errors.push(diagnostic(
                    &if path.starts_with("company.") {
                        path.to_owned()
                    } else {
                        format!("data.{path}")
                    },
                    "INVOICE-MODEL",
                    &problem,
                    &json!([]),
                ));
            }
            Value::Null
        }
    };
    let (data, company) = if errors.is_empty() {
        rendering_view(request, &normalized, profile)
    } else {
        (Value::Null, Value::Null)
    };
    json!({"status": if errors.is_empty() { "ready_for_preview" } else { "blocked" },
        "selection": request["selection"], "profile": profile, "normalized": normalized, "data": data, "company": company,
        "diagnostics": errors,
        "checks": {"schema": "not_run", "business_rules": if errors.is_empty() {"passed"} else {"failed"},
            "arithmetic": arithmetic, "en16931": "not_run", "output_profile": "not_run", "pdfa": "not_run", "transmission": "not_run"}})
}

// Adapt the canonical model back to the established template contract. Both this
// view and CII serialization originate in the same exact-decimal Invoice model.
fn rendering_view(request: &Value, model: &Value, profile: &Value) -> (Value, Value) {
    let mut data = request["data"].clone();
    let mut company = request["company"].clone();
    let money = |amount: &Value| json!({"amount": amount, "currency": model["currency"]});
    data["metadata"]["invoice_number"] = model["number"].clone();
    data["metadata"]["invoice_date"] = model["issue_date"].clone();
    if !model["delivery_date"].is_null() {
        data["metadata"]["delivery_date"] = model["delivery_date"].clone();
    }
    if !model["due_date"].is_null() {
        data["payment"]["due_date"] = model["due_date"].clone();
    }
    for (index, line) in model["lines"]
        .as_array()
        .expect("canonical lines")
        .iter()
        .enumerate()
    {
        let item = &mut data["items"][index];
        item["position"] = line["id"].clone();
        item["description"] = line["name"].clone();
        item["quantity"] = line["quantity"].clone();
        item["unit_code"] = line["unit"].clone();
        item["unit_price"] = money(&line["net_price"]);
        item["total"] = money(&line["net_amount"]);
        item["vat_rate"]["percentage"] = line["tax"]["rate"].clone();
    }
    data["totals"]["subtotal"] = money(&model["line_total"]);
    data["totals"]["vat_total"] = money(&model["tax_total"]);
    data["totals"]["total"] = money(&model["grand_total"]);
    data["totals"]["vat_breakdown"] = Value::Array(model["breakdown"].as_array().expect("canonical breakdown").iter().map(|group| {
        json!({"rate":{"percentage":group["tax"]["rate"],"category":group["tax"]["category"]},
            "base":money(&group["basis"]),"amount":money(&group["amount"])})
    }).collect());
    if !model["payment"].is_null() {
        let mut bank = request["company"]["bank_account"].clone();
        if !bank.is_object() {
            bank = json!({});
        }
        bank["iban"] = model["payment"]["iban"].clone();
        bank["bic"] = model["payment"]["bic"]
            .as_str()
            .map_or(json!(""), |value| json!(value));
        bank["account_holder"] = model["payment"]["account_name"]
            .as_str()
            .map_or(json!(""), |value| json!(value));
        if bank.get("bank_name").is_none() {
            bank["bank_name"] = json!("");
        }
        data["payment"]["bank_account"] = bank.clone();
        company["bank_account"] = bank;
    }
    data["profile_fields"] = Value::Array(
        array(profile, "display_fields")
            .iter()
            .filter_map(|field| {
                let path = field["path"].as_str()?;
                let language = request["selection"]["language"].as_str()?;
                let found = values(request, &path.split('.').collect::<Vec<_>>(), "");
                let value = found.first()?.1;
                present(value).then(|| json!({"label":field["labels"][language],"value":value}))
            })
            .collect(),
    );
    (data, company)
}
