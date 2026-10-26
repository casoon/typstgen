//! The invoice as EN 16931 sees it, built from docgen's invoice JSON and
//! company.json, and checked before any XML is written.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::decimal::Dec;

/// A postal address with an ISO 3166-1 country code.
#[derive(Debug, Clone, Serialize)]
pub struct Address {
    pub line: String,
    pub postcode: String,
    pub city: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Party {
    pub name: String,
    /// BT-46 for the buyer; unused for the seller.
    pub id: Option<String>,
    /// BT-56: contact person.
    pub contact: Option<String>,
    pub address: Address,
    /// BT-34 / BT-49: electronic address (e-mail).
    pub email: Option<String>,
    /// BT-31 / BT-48: VAT identifier.
    pub vat_id: Option<String>,
    /// BT-32: tax registration number (Steuernummer).
    pub tax_number: Option<String>,
}

/// VAT category (UNTDID 5305) with what EN 16931 needs for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Tax {
    /// S, Z, E or AE.
    pub category: &'static str,
    pub rate: Dec,
    /// BT-120 (text) or BT-121 (code) for E and AE.
    pub exemption_reason: Option<String>,
    pub exemption_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub quantity: Dec,
    /// UN/ECE Recommendation 20 unit code.
    pub unit: String,
    pub net_price: Dec,
    pub net_amount: Dec,
    pub tax: Tax,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaxBreakdown {
    pub tax: Tax,
    pub basis: Dec,
    pub amount: Dec,
}

#[derive(Debug, Clone, Serialize)]
pub struct Payment {
    pub iban: String,
    pub bic: Option<String>,
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Invoice {
    pub number: String,
    /// YYYYMMDD.
    pub issue_date: String,
    pub due_date: Option<String>,
    /// BT-72: actual delivery date.
    pub delivery_date: Option<String>,
    /// BT-73 / BT-74: billing period (start, end).
    pub billing_period: Option<(String, String)>,
    pub currency: String,
    pub notes: Vec<String>,
    pub seller: Party,
    pub buyer: Party,
    pub lines: Vec<Line>,
    pub breakdown: Vec<TaxBreakdown>,
    pub line_total: Dec,
    pub tax_total: Dec,
    pub grand_total: Dec,
    pub payment: Option<Payment>,
    pub payment_terms: Option<String>,
}

/// Collects every problem instead of stopping at the first.
struct Problems(Vec<String>);

impl Problems {
    fn add(&mut self, message: impl Into<String>) {
        self.0.push(message.into());
    }

    fn text(&mut self, value: &Value, path: &str) -> Option<String> {
        self.text_at(value, path, path)
    }

    fn text_at(&mut self, value: &Value, path: &str, diagnostic_path: &str) -> Option<String> {
        match lookup(value, path).and_then(Value::as_str).map(str::trim) {
            Some(text) if !text.is_empty() => Some(text.to_owned()),
            _ => {
                self.add(format!("{diagnostic_path} is missing"));
                None
            }
        }
    }
}

fn lookup<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}

fn optional(value: &Value, path: &str) -> Option<String> {
    lookup(value, path)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// `{"date": "2025-01-22"}`, `"2025-01-22"` or `"22.01.2025"` as YYYYMMDD.
fn date(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let text = value.get("date").unwrap_or(value).as_str()?.trim();
    let (year, month, day) = if let Some((d, rest)) = text.split_once('.') {
        let (m, y) = rest.split_once('.')?;
        (y, m, d)
    } else {
        let mut parts = text.splitn(3, '-');
        (parts.next()?, parts.next()?, parts.next()?)
    };
    let valid = year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && [year, month, day]
            .iter()
            .all(|part| part.chars().all(|c| c.is_ascii_digit()));
    let calendar_valid = year
        .parse::<i32>()
        .ok()
        .zip(month.parse::<u32>().ok())
        .zip(day.parse::<u32>().ok())
        .and_then(|((year, month), day)| chrono::NaiveDate::from_ymd_opt(year, month, day))
        .is_some();
    (valid && calendar_valid).then(|| format!("{year}{month}{day}"))
}

fn optional_date(value: Option<&Value>, path: &str, problems: &mut Problems) -> Option<String> {
    let parsed = date(value);
    if value.is_some_and(|value| !value.is_null()) && parsed.is_none() {
        problems.add(format!(
            "{path}: invalid calendar date (expected YYYY-MM-DD)"
        ));
    }
    parsed
}

/// Money object `{"amount": "1400", "currency": "EUR"}` or a plain number.
fn money(value: Option<&Value>) -> Option<Dec> {
    let value = value?;
    Dec::from_json(value.get("amount").unwrap_or(value))
}

/// `"01/2025"` or `"2025-01"` as the first and last day of that month.
fn month_period(text: &str) -> Option<(String, String)> {
    let (month, year) = match text.trim().split_once('/') {
        Some((month, year)) => (month, year),
        None => {
            let (year, month) = text.trim().split_once('-')?;
            (month, year)
        }
    };
    let month: u32 = month
        .parse()
        .ok()
        .filter(|month| (1..=12).contains(month))?;
    let year: u32 = year
        .parse()
        .ok()
        .filter(|year| (1000..=9999).contains(year))?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let last = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    Some((
        format!("{year}{month:02}01"),
        format!("{year}{month:02}{last}"),
    ))
}

fn country_code(name: Option<&str>) -> Option<String> {
    let name = name.map(str::trim).unwrap_or("Deutschland");
    let code = match name.to_lowercase().as_str() {
        "" | "deutschland" | "germany" => "DE",
        "österreich" | "oesterreich" | "austria" => "AT",
        "schweiz" | "switzerland" | "suisse" => "CH",
        "frankreich" | "france" => "FR",
        "niederlande" | "netherlands" | "nederland" => "NL",
        "italien" | "italy" | "italia" => "IT",
        "spanien" | "spain" | "españa" => "ES",
        "belgien" | "belgium" => "BE",
        "luxemburg" | "luxembourg" => "LU",
        "polen" | "poland" => "PL",
        "dänemark" | "denmark" => "DK",
        _ if name.len() == 2 && name.chars().all(|c| c.is_ascii_alphabetic()) => {
            return Some(name.to_uppercase())
        }
        _ => return None,
    };
    Some(code.to_owned())
}

/// UN/ECE Recommendation 20 code for the units docgen templates use.
fn unit_code(unit: &str) -> Option<&'static str> {
    let code = match unit.trim().trim_end_matches('.').to_lowercase().as_str() {
        "tag" | "tage" | "day" | "days" | "pt" | "personentag" | "personentage" => "DAY",
        "std" | "stunde" | "stunden" | "h" | "hour" | "hours" => "HUR",
        "min" | "minute" | "minuten" | "minutes" => "MIN",
        "monat" | "monate" | "month" | "months" => "MON",
        "woche" | "wochen" | "week" | "weeks" => "WEE",
        "jahr" | "jahre" | "year" | "years" => "ANN",
        "stk" | "stück" | "stueck" | "piece" | "pieces" | "pcs" => "H87",
        "pauschal" | "pauschale" | "psch" | "pauschalpreis" | "lump sum" | "flat" => "LS",
        "km" => "KMT",
        "kg" => "KGM",
        "m²" | "qm" | "m2" => "MTK",
        "lizenz" | "lizenzen" | "license" | "licenses" | "einheit" | "einheiten" | "unit"
        | "units" | "x" => "C62",
        _ => return None,
    };
    Some(code)
}

fn tax(item: &Value, buyer_has_vat_id: bool, problems: &mut Problems, label: &str) -> Tax {
    let rate = lookup(item, "vat_rate.percentage")
        .and_then(Dec::from_json)
        .unwrap_or_else(|| {
            problems.add(format!("{label}: vat_rate.percentage is missing"));
            Dec::ZERO
        });
    let code = optional(item, "vat_rate.code")
        .unwrap_or_default()
        .to_lowercase();
    let reason = optional(item, "vat_rate.exemption_reason");
    let reason_code = optional(item, "vat_rate.exemption_reason_code");
    let category = match code.as_str() {
        "reversecharge" | "reverse_charge" | "reverse-charge" => "AE",
        "exempt" | "smallbusiness" | "small_business" | "kleinunternehmer" => "E",
        "zero" => "Z",
        _ if rate.is_zero() => {
            problems.add(format!(
                "{label}: VAT rate 0 needs vat_rate.code \"Zero\", \"Exempt\" or \"ReverseCharge\""
            ));
            "Z"
        }
        _ => "S",
    };
    match category {
        "S" if rate.is_zero() => {
            problems.add(format!("{label}: standard VAT needs a rate above 0"))
        }
        "Z" | "E" | "AE" if !rate.is_zero() => {
            problems.add(format!("{label}: VAT category {category} needs rate 0"))
        }
        "E" if reason.is_none() && reason_code.is_none() => problems.add(format!(
            "{label}: exempt VAT needs vat_rate.exemption_reason (text) or exemption_reason_code"
        )),
        "AE" if !buyer_has_vat_id => {
            problems.add("recipient.vat_id: required for reverse charge (the buyer's VAT ID)")
        }
        _ => {}
    }
    let (exemption_reason, exemption_code) = match category {
        "E" => (reason, reason_code),
        "AE" => (
            reason,
            Some(reason_code.unwrap_or_else(|| "VATEX-EU-AE".to_owned())),
        ),
        _ => (None, None),
    };
    Tax {
        category,
        rate,
        exemption_reason,
        exemption_code,
    }
}

fn address(value: &Value, prefix: &str, diagnostic_root: &str, problems: &mut Problems) -> Address {
    let label = format!("{diagnostic_root}{prefix}");
    let street = problems
        .text_at(
            value,
            &format!("{prefix}.street"),
            &format!("{label}.street"),
        )
        .unwrap_or_default();
    let number = optional(value, &format!("{prefix}.house_number")).unwrap_or_default();
    let country = optional(value, &format!("{prefix}.country_code"))
        .or_else(|| optional(value, &format!("{prefix}.country")));
    let country = country_code(country.as_deref()).unwrap_or_else(|| {
        problems.add(format!(
            "{label}.country: unknown country, set {label}.country_code (ISO 3166-1, e.g. \"DE\")"
        ));
        String::new()
    });
    Address {
        line: format!("{street} {number}").trim().to_owned(),
        postcode: problems
            .text_at(
                value,
                &format!("{prefix}.postal_code"),
                &format!("{label}.postal_code"),
            )
            .unwrap_or_default(),
        city: problems
            .text_at(value, &format!("{prefix}.city"), &format!("{label}.city"))
            .unwrap_or_default(),
        country,
    }
}

impl Invoice {
    /// Builds and checks the invoice. `invoice` has the shape of docgen's
    /// invoice JSON, `company` that of company.json. Every problem found is
    /// returned, one per line.
    pub fn from_docgen(invoice: &Value, company: &Value) -> Result<Invoice, Vec<String>> {
        let mut problems = Problems(Vec::new());

        let number = problems
            .text(invoice, "metadata.invoice_number")
            .unwrap_or_default();
        let issue_date = date(lookup(invoice, "metadata.invoice_date")).unwrap_or_else(|| {
            problems.add("metadata.invoice_date is missing or not a date (YYYY-MM-DD)");
            String::new()
        });
        let payment_due = optional_date(
            lookup(invoice, "payment.due_date"),
            "payment.due_date",
            &mut problems,
        );
        let metadata_due = optional_date(
            lookup(invoice, "metadata.due_date"),
            "metadata.due_date",
            &mut problems,
        );
        if payment_due
            .as_ref()
            .zip(metadata_due.as_ref())
            .is_some_and(|(payment, metadata)| payment != metadata)
        {
            problems.add("metadata.due_date: differs from payment.due_date");
        }
        let due_date = payment_due.or(metadata_due);
        let currency = lookup(invoice, "totals.total.currency")
            .and_then(Value::as_str)
            .or_else(|| lookup(company, "default_terms.currency").and_then(Value::as_str))
            .unwrap_or("EUR")
            .to_owned();

        let seller = Party {
            name: problems
                .text_at(company, "name", "company.name")
                .unwrap_or_default(),
            id: None,
            contact: None,
            address: address(company, "address", "company.", &mut problems),
            email: optional(company, "contact.email"),
            vat_id: optional(company, "vat_id"),
            tax_number: optional(company, "tax_id"),
        };
        if seller.vat_id.is_none() && seller.tax_number.is_none() {
            problems.add("company.vat_id or company.tax_id is needed (seller tax registration)");
        }

        let recipient_company = optional(invoice, "recipient.company");
        let recipient_name = optional(invoice, "recipient.name");
        let buyer = Party {
            name: recipient_company
                .clone()
                .or_else(|| recipient_name.clone())
                .unwrap_or_else(|| {
                    problems.add("recipient.company or recipient.name is missing");
                    String::new()
                }),
            id: optional(invoice, "metadata.customer_number"),
            contact: recipient_company.and(recipient_name),
            address: address(invoice, "recipient.address", "", &mut problems),
            email: optional(invoice, "recipient.email"),
            vat_id: optional(invoice, "recipient.vat_id"),
            tax_number: None,
        };

        let items = lookup(invoice, "items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if items.is_empty() {
            problems.add("items: the invoice has no lines");
        }
        let mut lines = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let label = format!("items[{index}]");
            let id = item
                .get("position")
                .map(|value| value.to_string().trim_matches('"').to_owned())
                .unwrap_or_else(|| (index + 1).to_string());
            let name = problems
                .text(item, "description")
                .map(|name| name.replace(['\n'], " "))
                .unwrap_or_default();
            let description = item
                .get("sub_items")
                .and_then(Value::as_array)
                .map(|sub| {
                    sub.iter()
                        .filter_map(Value::as_str)
                        .map(|text| text.trim().trim_start_matches("- ").to_owned())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|text| !text.is_empty());
            let quantity = item.get("quantity").and_then(Dec::from_json);
            let net_price = money(item.get("unit_price"));
            let unit_text = optional(item, "unit").unwrap_or_default();
            let unit = optional(item, "unit_code")
                .or_else(|| unit_code(&unit_text).map(str::to_owned))
                .unwrap_or_else(|| {
                    problems.add(format!(
                        "{label}: unit \"{unit_text}\" has no UN/ECE code, set {label}.unit_code (e.g. \"C62\")"
                    ));
                    String::new()
                });
            let (Some(quantity), Some(net_price)) = (quantity, net_price) else {
                problems.add(format!("{label}: quantity or unit_price.amount is missing"));
                continue;
            };
            let net_amount = quantity.mul_money(net_price);
            if let Some(stated) = money(item.get("total")) {
                if stated.round2() != net_amount {
                    problems.add(format!(
                        "{label}: total {} does not match quantity × unit price = {}",
                        stated.amount(),
                        net_amount.amount()
                    ));
                }
            }
            let tax = tax(item, buyer.vat_id.is_some(), &mut problems, &label);
            lines.push(Line {
                id,
                name,
                description,
                quantity,
                unit,
                net_price,
                net_amount,
                tax,
            });
        }

        // BR-CO-10, BR-S-08 / BR-Z-08 / BR-E-08 / BR-AE-08, BR-S-09.
        let mut groups: BTreeMap<Tax, Dec> = BTreeMap::new();
        for line in &lines {
            let basis = groups.entry(line.tax.clone()).or_default();
            *basis = *basis + line.net_amount;
        }
        let breakdown: Vec<TaxBreakdown> = groups
            .into_iter()
            .map(|(tax, basis)| TaxBreakdown {
                amount: basis.percent_money(tax.rate),
                tax,
                basis,
            })
            .collect();
        let line_total: Dec = lines.iter().map(|line| line.net_amount).sum();
        let tax_total: Dec = breakdown.iter().map(|group| group.amount).sum();
        let grand_total = line_total + tax_total;

        // Stated VAT groups also appear in the PDF. Comparing only grand totals
        // could hide a swapped rate or taxable base while CII says something else.
        if let Some(stated_groups) =
            lookup(invoice, "totals.vat_breakdown").and_then(Value::as_array)
        {
            let stated_sum: Option<Dec> = stated_groups
                .iter()
                .map(|group| money(group.get("amount")))
                .sum();
            if stated_sum.is_none_or(|sum| sum.round2() != tax_total) {
                problems.add("totals.vat_breakdown: stated VAT amounts do not sum to the normalized tax total");
            }
            for (index, group) in stated_groups.iter().enumerate() {
                for field in ["base", "amount"] {
                    if group
                        .get(field)
                        .and_then(|money| money.get("currency"))
                        .and_then(Value::as_str)
                        .is_some_and(|code| code != currency)
                    {
                        problems.add(format!("totals.vat_breakdown[{index}].{field}.currency differs from invoice currency"));
                    }
                }
                let rate = group
                    .get("rate")
                    .and_then(|r| r.get("percentage").or(Some(r)))
                    .and_then(Dec::from_json);
                let matches: Vec<_> = breakdown
                    .iter()
                    .filter(|computed| {
                        let stated_category = group
                            .get("rate")
                            .and_then(|r| r.get("category").or_else(|| r.get("code")))
                            .and_then(Value::as_str)
                            .map(|code| match code.to_lowercase().as_str() {
                                "standard" | "reduced" | "s" => "S",
                                "zero" | "z" => "Z",
                                "exempt" | "smallbusiness" | "kleinunternehmer" | "e" => "E",
                                "reversecharge" | "ae" => "AE",
                                _ => "unknown",
                            });
                        Some(computed.tax.rate) == rate
                            && stated_category
                                .is_none_or(|category| computed.tax.category == category)
                    })
                    .collect();
                let amount: Dec = matches.iter().map(|group| group.amount).sum();
                let basis: Dec = matches.iter().map(|group| group.basis).sum();
                if matches.is_empty()
                    || money(group.get("amount")).is_none_or(|value| value.round2() != amount)
                {
                    problems.add(format!("totals.vat_breakdown[{index}].amount does not match the normalized tax groups"));
                }
                if let Some(stated) = money(group.get("base")) {
                    if stated.round2() != basis {
                        problems.add(format!("totals.vat_breakdown[{index}].base does not match the normalized taxable base"));
                    }
                }
            }
        }
        for (index, item) in items.iter().enumerate() {
            for field in ["unit_price", "total"] {
                if item
                    .get(field)
                    .and_then(|money| money.get("currency"))
                    .and_then(Value::as_str)
                    .is_some_and(|code| code != currency)
                {
                    problems.add(format!(
                        "items[{index}].{field}.currency differs from invoice currency"
                    ));
                }
            }
        }

        for field in ["subtotal", "vat_total", "total", "discount_total"] {
            if lookup(invoice, &format!("totals.{field}.currency"))
                .and_then(Value::as_str)
                .is_some_and(|code| code != currency)
            {
                problems.add(format!(
                    "totals.{field}.currency differs from invoice currency"
                ));
            }
        }

        // The PDF shows docgen's totals; the XML must say the same.
        for (path, computed) in [
            ("totals.subtotal", line_total),
            ("totals.vat_total", tax_total),
            ("totals.total", grand_total),
        ] {
            if let Some(stated) = money(lookup(invoice, path)) {
                if stated.round2() != computed {
                    problems.add(format!(
                        "{path} is {}, but the lines add up to {}",
                        stated.amount(),
                        computed.amount()
                    ));
                }
            }
        }

        let bank = lookup(invoice, "payment.bank_account").or_else(|| company.get("bank_account"));
        let payment = bank.and_then(|bank| {
            let iban = optional(bank, "iban")?.replace(' ', "");
            Some(Payment {
                iban,
                bic: optional(bank, "bic").map(|bic| bic.replace(' ', "")),
                account_name: optional(bank, "account_holder"),
            })
        });
        let payment_terms = optional(invoice, "payment.payment_terms");
        // BR-CO-25.
        if grand_total.is_positive() && due_date.is_none() && payment_terms.is_none() {
            problems.add("payment.due_date or payment.payment_terms is needed for an amount due");
        }

        let mut notes = Vec::new();
        if let Some(value) = invoice.get("notes") {
            match serde_json::from_value::<Vec<String>>(value.clone()) {
                Ok(values) => notes.extend(values),
                Err(_) => problems.add("notes: expected an array of strings"),
            }
        }
        if let Some(project) = optional(invoice, "metadata.project_reference") {
            notes.push(format!("Projekt: {project}"));
        }
        let period_text = optional(invoice, "metadata.performance_period");
        let billing_period = period_text.as_deref().and_then(month_period);
        if let (Some(period), None) = (&period_text, &billing_period) {
            notes.push(format!("Leistungszeitraum: {period}"));
        }
        let delivery_date = optional_date(
            lookup(invoice, "metadata.delivery_date"),
            "metadata.delivery_date",
            &mut problems,
        );
        if lookup(invoice, "totals.discount_total").is_some_and(|value| !value.is_null())
            && money(lookup(invoice, "totals.discount_total"))
                .is_none_or(|amount| !amount.is_zero())
        {
            problems.add(
                "totals.discount_total: invoice allowances are not supported by this CII model",
            );
        }

        if !problems.0.is_empty() {
            return Err(problems.0);
        }
        Ok(Invoice {
            number,
            issue_date,
            due_date,
            delivery_date,
            billing_period,
            currency,
            notes,
            seller,
            buyer,
            lines,
            breakdown,
            line_total,
            tax_total,
            grand_total,
            payment,
            payment_terms,
        })
    }
}
