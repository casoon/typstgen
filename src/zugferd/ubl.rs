//! OASIS UBL 2.1 invoices for XRechnung 3.0 and Peppol BIS Billing 3.0.
//!
//! Element order follows the UBL schema; the elements are the subset that
//! [`Invoice`] carries. Both formats share the syntax and differ in their
//! specification identifier (BT-24) and in the fields they make mandatory.

use std::fmt::Write;
use std::str::FromStr;

use super::cii::escape;
use super::model::{Address, Invoice, Party, Tax};
use crate::Error;

/// The business process (BT-23) both formats use.
pub const PROFILE_ID: &str = "urn:fdc:peppol.eu:2017:poacc:billing:01:1.0";

/// An XML-only e-invoice format: one syntax, one specification version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UblFormat {
    /// XRechnung 3.0 (KoSIT), UBL syntax.
    XRechnung30,
    /// Peppol BIS Billing 3.0 (OpenPeppol), UBL syntax.
    PeppolBis30,
}

impl UblFormat {
    /// Every supported format.
    pub const ALL: [UblFormat; 2] = [UblFormat::XRechnung30, UblFormat::PeppolBis30];

    /// The name used by the CLI and MCP, e.g. `xrechnung-3.0`.
    pub fn id(self) -> &'static str {
        match self {
            UblFormat::XRechnung30 => "xrechnung-3.0",
            UblFormat::PeppolBis30 => "peppol-bis-3.0",
        }
    }

    /// The specification identifier (BT-24, `cbc:CustomizationID`).
    pub fn customization_id(self) -> &'static str {
        match self {
            UblFormat::XRechnung30 => {
                "urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0"
            }
            UblFormat::PeppolBis30 => {
                "urn:cen.eu:en16931:2017#compliant#urn:fdc:peppol.eu:2017:poacc:billing:3.0"
            }
        }
    }

    /// The validation states of an XML this crate returned. Arithmetic and
    /// required fields are checked before writing; schema and Schematron
    /// validation need external validators and are never reported as passed.
    pub fn checks(self) -> [(&'static str, &'static str); 6] {
        [
            ("arithmetic", "passed"),
            ("required_fields", "passed"),
            ("xml_schema", "not_run"),
            ("en16931", "not_run"),
            (
                match self {
                    UblFormat::XRechnung30 => "xrechnung",
                    UblFormat::PeppolBis30 => "peppol",
                },
                "not_run",
            ),
            ("transmission", "not_run"),
        ]
    }
}

impl FromStr for UblFormat {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self, Error> {
        UblFormat::ALL
            .into_iter()
            .find(|format| format.id() == name)
            .ok_or_else(|| {
                let known: Vec<_> = UblFormat::ALL.iter().map(|format| format.id()).collect();
                Error::Zugferd(format!(
                    "unsupported e-invoice format {name:?}, expected one of: {}",
                    known.join(", ")
                ))
            })
    }
}

/// The fields `format` requires beyond EN 16931, with the JSON path to fix.
pub(crate) fn check(format: UblFormat, invoice: &Invoice) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    // BT-34 / BT-49: PEPPOL-EN16931-R020 / R010, mandatory in XRechnung 3.0 as well.
    if invoice.seller.endpoint.is_none() {
        problems.push(
            "company.electronic_address: required ({\"scheme_id\": \"0204\", \"value\": \"…\"})"
                .to_owned(),
        );
    }
    if invoice.buyer.endpoint.is_none() {
        problems.push(
            "recipient.electronic_address: required ({\"scheme_id\": \"0204\", \"value\": \"…\"})"
                .to_owned(),
        );
    }
    match format {
        UblFormat::XRechnung30 => {
            // BR-DE-15.
            if invoice.buyer_reference.is_none() {
                problems.push(
                    "metadata.buyer_reference: required (the Leitweg-ID for public buyers)"
                        .to_owned(),
                );
            }
            // BR-DE-2, BR-DE-5, BR-DE-6, BR-DE-7.
            for (value, path) in [
                (&invoice.seller.contact, "company.contact.name"),
                (&invoice.seller.phone, "company.contact.phone"),
                (&invoice.seller.email, "company.contact.email"),
            ] {
                if value.is_none() {
                    problems.push(format!("{path}: required by XRechnung (seller contact)"));
                }
            }
            // BR-DE-10 and BR-DE-11: a deliver-to address needs city and post code.
            if invoice.delivery_country.is_some() {
                if invoice.delivery_city.is_none() {
                    problems.push(
                        "metadata.delivery_city: required by XRechnung when metadata.delivery_country_code is given"
                            .to_owned(),
                    );
                }
                if invoice.delivery_postal_code.is_none() {
                    problems.push(
                        "metadata.delivery_postal_code: required by XRechnung when metadata.delivery_country_code is given"
                            .to_owned(),
                    );
                }
            }
            // BR-DE-1; a credit transfer (30, 58) needs the account (BG-17).
            match payment_means_code(invoice) {
                None => problems.push(
                    "payment.method_code or a bank account (payment.bank_account, company.bank_account) is required by XRechnung"
                        .to_owned(),
                ),
                Some("30" | "58") if invoice.payment.is_none() => problems.push(
                    "payment.bank_account: an IBAN is required for a credit transfer".to_owned(),
                ),
                Some(_) => {}
            }
        }
        UblFormat::PeppolBis30 => {
            // PEPPOL-EN16931-R003.
            if invoice.buyer_reference.is_none() && invoice.order_reference.is_none() {
                problems.push(
                    "metadata.buyer_reference or metadata.order_reference: one is required by Peppol"
                        .to_owned(),
                );
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// The stated code, or SEPA credit transfer when a bank account is given.
fn payment_means_code(invoice: &Invoice) -> Option<&str> {
    invoice
        .payment_means_code
        .as_deref()
        .or(invoice.payment.as_ref().map(|_| "58"))
}

/// Writes `<tag>text</tag>` with escaped text.
fn el(out: &mut String, indent: usize, tag: &str, text: &str) {
    let _ = writeln!(out, "{:indent$}<{tag}>{}</{tag}>", "", escape(text));
}

/// Writes `<tag attribute="value">text</tag>`.
fn el_attr(out: &mut String, indent: usize, tag: &str, attribute: (&str, &str), text: &str) {
    let _ = writeln!(
        out,
        "{:indent$}<{tag} {}=\"{}\">{}</{tag}>",
        "",
        attribute.0,
        escape(attribute.1),
        escape(text)
    );
}

fn open(out: &mut String, indent: usize, tag: &str) {
    let _ = writeln!(out, "{:indent$}<{tag}>", "");
}

fn close(out: &mut String, indent: usize, tag: &str) {
    let _ = writeln!(out, "{:indent$}</{tag}>", "");
}

/// YYYYMMDD as YYYY-MM-DD.
fn date(out: &mut String, indent: usize, tag: &str, yyyymmdd: &str) {
    let iso = format!("{}-{}-{}", &yyyymmdd[..4], &yyyymmdd[4..6], &yyyymmdd[6..]);
    el(out, indent, tag, &iso);
}

fn amount(out: &mut String, indent: usize, tag: &str, currency: &str, value: &str) {
    el_attr(out, indent, tag, ("currencyID", currency), value);
}

fn address(out: &mut String, indent: usize, address: &Address) {
    open(out, indent, "cac:PostalAddress");
    el(out, indent + 2, "cbc:StreetName", &address.line);
    el(out, indent + 2, "cbc:CityName", &address.city);
    el(out, indent + 2, "cbc:PostalZone", &address.postcode);
    open(out, indent + 2, "cac:Country");
    el(out, indent + 4, "cbc:IdentificationCode", &address.country);
    close(out, indent + 2, "cac:Country");
    close(out, indent, "cac:PostalAddress");
}

fn tax_scheme(out: &mut String, indent: usize, id: &str) {
    open(out, indent, "cac:TaxScheme");
    el(out, indent + 2, "cbc:ID", id);
    close(out, indent, "cac:TaxScheme");
}

fn party(out: &mut String, indent: usize, tag: &str, party: &Party) {
    open(out, indent, tag);
    open(out, indent + 2, "cac:Party");
    let inner = indent + 4;
    if let Some(endpoint) = &party.endpoint {
        el_attr(
            out,
            inner,
            "cbc:EndpointID",
            ("schemeID", &endpoint.scheme),
            &endpoint.value,
        );
    }
    if let Some(id) = &party.id {
        open(out, inner, "cac:PartyIdentification");
        el(out, inner + 2, "cbc:ID", id);
        close(out, inner, "cac:PartyIdentification");
    }
    open(out, inner, "cac:PartyName");
    el(out, inner + 2, "cbc:Name", &party.name);
    close(out, inner, "cac:PartyName");
    address(out, inner, &party.address);
    // BT-31 / BT-48 and BT-32.
    for (id, scheme) in [(&party.vat_id, "VAT"), (&party.tax_number, "FC")] {
        if let Some(id) = id {
            open(out, inner, "cac:PartyTaxScheme");
            el(out, inner + 2, "cbc:CompanyID", id);
            tax_scheme(out, inner + 2, scheme);
            close(out, inner, "cac:PartyTaxScheme");
        }
    }
    open(out, inner, "cac:PartyLegalEntity");
    el(out, inner + 2, "cbc:RegistrationName", &party.name);
    close(out, inner, "cac:PartyLegalEntity");
    if party.contact.is_some() || party.phone.is_some() || party.email.is_some() {
        open(out, inner, "cac:Contact");
        for (value, tag) in [
            (&party.contact, "cbc:Name"),
            (&party.phone, "cbc:Telephone"),
            (&party.email, "cbc:ElectronicMail"),
        ] {
            if let Some(value) = value {
                el(out, inner + 2, tag, value);
            }
        }
        close(out, inner, "cac:Contact");
    }
    close(out, indent + 2, "cac:Party");
    close(out, indent, tag);
}

/// A `TaxCategory` (document level) or `ClassifiedTaxCategory` (line level,
/// which carries no exemption reason).
fn tax_category(out: &mut String, indent: usize, tag: &str, tax: &Tax) {
    open(out, indent, tag);
    el(out, indent + 2, "cbc:ID", tax.category);
    el(out, indent + 2, "cbc:Percent", &tax.rate.to_string());
    if tag == "cac:TaxCategory" {
        if let Some(code) = &tax.exemption_code {
            el(out, indent + 2, "cbc:TaxExemptionReasonCode", code);
        }
        if let Some(reason) = &tax.exemption_reason {
            el(out, indent + 2, "cbc:TaxExemptionReason", reason);
        }
    }
    tax_scheme(out, indent + 2, "VAT");
    close(out, indent, tag);
}

/// The UBL XML of `invoice` in `format` (UTF-8). Call [`check`] first.
pub(crate) fn write(format: UblFormat, invoice: &Invoice) -> String {
    let currency = invoice.currency.as_str();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<ubl:Invoice \
         xmlns:ubl=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\" \
         xmlns:cac=\"urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2\" \
         xmlns:cbc=\"urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2\">\n",
    );
    el(
        &mut out,
        2,
        "cbc:CustomizationID",
        format.customization_id(),
    );
    el(&mut out, 2, "cbc:ProfileID", PROFILE_ID);
    el(&mut out, 2, "cbc:ID", &invoice.number);
    date(&mut out, 2, "cbc:IssueDate", &invoice.issue_date);
    if let Some(due) = &invoice.due_date {
        date(&mut out, 2, "cbc:DueDate", due);
    }
    el(&mut out, 2, "cbc:InvoiceTypeCode", "380");
    match format {
        UblFormat::XRechnung30 => {
            for note in &invoice.notes {
                el(&mut out, 2, "cbc:Note", note);
            }
        }
        // PEPPOL-EN16931-R002: at most one note.
        UblFormat::PeppolBis30 if !invoice.notes.is_empty() => {
            el(&mut out, 2, "cbc:Note", &invoice.notes.join("\n"));
        }
        UblFormat::PeppolBis30 => {}
    }
    el(&mut out, 2, "cbc:DocumentCurrencyCode", currency);
    if let Some(reference) = &invoice.buyer_reference {
        el(&mut out, 2, "cbc:BuyerReference", reference);
    }
    if let Some((start, end)) = &invoice.billing_period {
        open(&mut out, 2, "cac:InvoicePeriod");
        date(&mut out, 4, "cbc:StartDate", start);
        date(&mut out, 4, "cbc:EndDate", end);
        close(&mut out, 2, "cac:InvoicePeriod");
    }
    if let Some(reference) = &invoice.order_reference {
        open(&mut out, 2, "cac:OrderReference");
        el(&mut out, 4, "cbc:ID", reference);
        close(&mut out, 2, "cac:OrderReference");
    }
    party(&mut out, 2, "cac:AccountingSupplierParty", &invoice.seller);
    party(&mut out, 2, "cac:AccountingCustomerParty", &invoice.buyer);
    if invoice.delivery_date.is_some() || invoice.delivery_country.is_some() {
        open(&mut out, 2, "cac:Delivery");
        if let Some(delivery) = &invoice.delivery_date {
            date(&mut out, 4, "cbc:ActualDeliveryDate", delivery);
        }
        if let Some(country) = &invoice.delivery_country {
            // BT-80.
            open(&mut out, 4, "cac:DeliveryLocation");
            open(&mut out, 6, "cac:Address");
            if let Some(city) = &invoice.delivery_city {
                el(&mut out, 8, "cbc:CityName", city);
            }
            if let Some(code) = &invoice.delivery_postal_code {
                el(&mut out, 8, "cbc:PostalZone", code);
            }
            open(&mut out, 8, "cac:Country");
            el(&mut out, 10, "cbc:IdentificationCode", country);
            close(&mut out, 8, "cac:Country");
            close(&mut out, 6, "cac:Address");
            close(&mut out, 4, "cac:DeliveryLocation");
        }
        close(&mut out, 2, "cac:Delivery");
    }
    if let Some(code) = payment_means_code(invoice) {
        open(&mut out, 2, "cac:PaymentMeans");
        el(&mut out, 4, "cbc:PaymentMeansCode", code);
        // BT-83, as in the CII adapter.
        el(&mut out, 4, "cbc:PaymentID", &invoice.number);
        if let Some(payment) = &invoice.payment {
            open(&mut out, 4, "cac:PayeeFinancialAccount");
            el(&mut out, 6, "cbc:ID", &payment.iban);
            if let Some(name) = &payment.account_name {
                el(&mut out, 6, "cbc:Name", name);
            }
            if let Some(bic) = &payment.bic {
                open(&mut out, 6, "cac:FinancialInstitutionBranch");
                el(&mut out, 8, "cbc:ID", bic);
                close(&mut out, 6, "cac:FinancialInstitutionBranch");
            }
            close(&mut out, 4, "cac:PayeeFinancialAccount");
        }
        close(&mut out, 2, "cac:PaymentMeans");
    }
    if let Some(terms) = &invoice.payment_terms {
        open(&mut out, 2, "cac:PaymentTerms");
        el(&mut out, 4, "cbc:Note", terms);
        close(&mut out, 2, "cac:PaymentTerms");
    }

    open(&mut out, 2, "cac:TaxTotal");
    amount(
        &mut out,
        4,
        "cbc:TaxAmount",
        currency,
        &invoice.tax_total.amount(),
    );
    for group in &invoice.breakdown {
        open(&mut out, 4, "cac:TaxSubtotal");
        amount(
            &mut out,
            6,
            "cbc:TaxableAmount",
            currency,
            &group.basis.amount(),
        );
        amount(
            &mut out,
            6,
            "cbc:TaxAmount",
            currency,
            &group.amount.amount(),
        );
        tax_category(&mut out, 6, "cac:TaxCategory", &group.tax);
        close(&mut out, 4, "cac:TaxSubtotal");
    }
    close(&mut out, 2, "cac:TaxTotal");

    open(&mut out, 2, "cac:LegalMonetaryTotal");
    for (tag, value) in [
        ("cbc:LineExtensionAmount", invoice.line_total),
        ("cbc:TaxExclusiveAmount", invoice.line_total),
        ("cbc:TaxInclusiveAmount", invoice.grand_total),
        ("cbc:PayableAmount", invoice.grand_total),
    ] {
        amount(&mut out, 4, tag, currency, &value.amount());
    }
    close(&mut out, 2, "cac:LegalMonetaryTotal");

    for line in &invoice.lines {
        open(&mut out, 2, "cac:InvoiceLine");
        el(&mut out, 4, "cbc:ID", &line.id);
        el_attr(
            &mut out,
            4,
            "cbc:InvoicedQuantity",
            ("unitCode", &line.unit),
            &line.quantity.to_string(),
        );
        amount(
            &mut out,
            4,
            "cbc:LineExtensionAmount",
            currency,
            &line.net_amount.amount(),
        );
        open(&mut out, 4, "cac:Item");
        if let Some(description) = &line.description {
            el(&mut out, 6, "cbc:Description", description);
        }
        el(&mut out, 6, "cbc:Name", &line.name);
        tax_category(&mut out, 6, "cac:ClassifiedTaxCategory", &line.tax);
        close(&mut out, 4, "cac:Item");
        open(&mut out, 4, "cac:Price");
        amount(
            &mut out,
            6,
            "cbc:PriceAmount",
            currency,
            &line.net_price.to_string(),
        );
        close(&mut out, 4, "cac:Price");
        close(&mut out, 2, "cac:InvoiceLine");
    }
    out.push_str("</ubl:Invoice>\n");
    out
}
