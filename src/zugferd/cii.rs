//! UN/CEFACT Cross Industry Invoice (D16B) for the EN 16931 profile.
//!
//! Element order follows the CII schema; the elements are the subset that
//! [`Invoice`] carries.

use std::fmt::Write;

use super::model::{Address, Invoice, Party, Tax};

/// The guideline identifier (BT-24) of the EN 16931 profile ("Comfort").
pub const GUIDELINE: &str = "urn:cen.eu:en16931:2017";

pub(super) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Writes `<tag>text</tag>` with escaped text.
fn el(out: &mut String, indent: usize, tag: &str, text: &str) {
    let _ = writeln!(out, "{:indent$}<{tag}>{}</{tag}>", "", escape(text));
}

fn open(out: &mut String, indent: usize, tag: &str) {
    let _ = writeln!(out, "{:indent$}<{tag}>", "");
}

fn close(out: &mut String, indent: usize, tag: &str) {
    let _ = writeln!(out, "{:indent$}</{tag}>", "");
}

fn date(out: &mut String, indent: usize, tag: &str, yyyymmdd: &str) {
    open(out, indent, tag);
    let _ = writeln!(
        out,
        "{:width$}<udt:DateTimeString format=\"102\">{yyyymmdd}</udt:DateTimeString>",
        "",
        width = indent + 2
    );
    close(out, indent, tag);
}

fn address(out: &mut String, indent: usize, address: &Address) {
    open(out, indent, "ram:PostalTradeAddress");
    el(out, indent + 2, "ram:PostcodeCode", &address.postcode);
    el(out, indent + 2, "ram:LineOne", &address.line);
    el(out, indent + 2, "ram:CityName", &address.city);
    el(out, indent + 2, "ram:CountryID", &address.country);
    close(out, indent, "ram:PostalTradeAddress");
}

fn party(out: &mut String, indent: usize, tag: &str, party: &Party) {
    open(out, indent, tag);
    if let Some(id) = &party.id {
        el(out, indent + 2, "ram:ID", id);
    }
    el(out, indent + 2, "ram:Name", &party.name);
    if let Some(contact) = &party.contact {
        open(out, indent + 2, "ram:DefinedTradeContact");
        el(out, indent + 4, "ram:PersonName", contact);
        close(out, indent + 2, "ram:DefinedTradeContact");
    }
    address(out, indent + 2, &party.address);
    if let Some(email) = &party.email {
        open(out, indent + 2, "ram:URIUniversalCommunication");
        let _ = writeln!(
            out,
            "{:width$}<ram:URIID schemeID=\"EM\">{}</ram:URIID>",
            "",
            escape(email),
            width = indent + 4
        );
        close(out, indent + 2, "ram:URIUniversalCommunication");
    }
    for (scheme, id) in [("VA", &party.vat_id), ("FC", &party.tax_number)] {
        if let Some(id) = id {
            open(out, indent + 2, "ram:SpecifiedTaxRegistration");
            let _ = writeln!(
                out,
                "{:width$}<ram:ID schemeID=\"{scheme}\">{}</ram:ID>",
                "",
                escape(id),
                width = indent + 4
            );
            close(out, indent + 2, "ram:SpecifiedTaxRegistration");
        }
    }
    close(out, indent, tag);
}

/// A line-level `ApplicableTradeTax`.
fn line_tax(out: &mut String, indent: usize, tax: &Tax) {
    open(out, indent, "ram:ApplicableTradeTax");
    el(out, indent + 2, "ram:TypeCode", "VAT");
    el(out, indent + 2, "ram:CategoryCode", tax.category);
    el(
        out,
        indent + 2,
        "ram:RateApplicablePercent",
        &tax.rate.to_string(),
    );
    close(out, indent, "ram:ApplicableTradeTax");
}

/// The CII XML of `invoice` (UTF-8).
pub fn write(invoice: &Invoice) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<rsm:CrossIndustryInvoice \
         xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\" \
         xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\" \
         xmlns:qdt=\"urn:un:unece:uncefact:data:standard:QualifiedDataType:100\" \
         xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\">\n",
    );

    open(&mut out, 2, "rsm:ExchangedDocumentContext");
    open(
        &mut out,
        4,
        "ram:GuidelineSpecifiedDocumentContextParameter",
    );
    el(&mut out, 6, "ram:ID", GUIDELINE);
    close(
        &mut out,
        4,
        "ram:GuidelineSpecifiedDocumentContextParameter",
    );
    close(&mut out, 2, "rsm:ExchangedDocumentContext");

    open(&mut out, 2, "rsm:ExchangedDocument");
    el(&mut out, 4, "ram:ID", &invoice.number);
    el(&mut out, 4, "ram:TypeCode", "380");
    date(&mut out, 4, "ram:IssueDateTime", &invoice.issue_date);
    for note in &invoice.notes {
        open(&mut out, 4, "ram:IncludedNote");
        el(&mut out, 6, "ram:Content", note);
        close(&mut out, 4, "ram:IncludedNote");
    }
    close(&mut out, 2, "rsm:ExchangedDocument");

    open(&mut out, 2, "rsm:SupplyChainTradeTransaction");
    for line in &invoice.lines {
        open(&mut out, 4, "ram:IncludedSupplyChainTradeLineItem");
        open(&mut out, 6, "ram:AssociatedDocumentLineDocument");
        el(&mut out, 8, "ram:LineID", &line.id);
        close(&mut out, 6, "ram:AssociatedDocumentLineDocument");
        open(&mut out, 6, "ram:SpecifiedTradeProduct");
        el(&mut out, 8, "ram:Name", &line.name);
        if let Some(description) = &line.description {
            el(&mut out, 8, "ram:Description", description);
        }
        close(&mut out, 6, "ram:SpecifiedTradeProduct");
        open(&mut out, 6, "ram:SpecifiedLineTradeAgreement");
        open(&mut out, 8, "ram:NetPriceProductTradePrice");
        el(
            &mut out,
            10,
            "ram:ChargeAmount",
            &line.net_price.to_string(),
        );
        close(&mut out, 8, "ram:NetPriceProductTradePrice");
        close(&mut out, 6, "ram:SpecifiedLineTradeAgreement");
        open(&mut out, 6, "ram:SpecifiedLineTradeDelivery");
        let _ = writeln!(
            out,
            "        <ram:BilledQuantity unitCode=\"{}\">{}</ram:BilledQuantity>",
            escape(&line.unit),
            line.quantity
        );
        close(&mut out, 6, "ram:SpecifiedLineTradeDelivery");
        open(&mut out, 6, "ram:SpecifiedLineTradeSettlement");
        line_tax(&mut out, 8, &line.tax);
        open(
            &mut out,
            8,
            "ram:SpecifiedTradeSettlementLineMonetarySummation",
        );
        el(
            &mut out,
            10,
            "ram:LineTotalAmount",
            &line.net_amount.amount(),
        );
        close(
            &mut out,
            8,
            "ram:SpecifiedTradeSettlementLineMonetarySummation",
        );
        close(&mut out, 6, "ram:SpecifiedLineTradeSettlement");
        close(&mut out, 4, "ram:IncludedSupplyChainTradeLineItem");
    }

    open(&mut out, 4, "ram:ApplicableHeaderTradeAgreement");
    party(&mut out, 6, "ram:SellerTradeParty", &invoice.seller);
    party(&mut out, 6, "ram:BuyerTradeParty", &invoice.buyer);
    close(&mut out, 4, "ram:ApplicableHeaderTradeAgreement");

    match &invoice.delivery_date {
        Some(delivery) => {
            open(&mut out, 4, "ram:ApplicableHeaderTradeDelivery");
            open(&mut out, 6, "ram:ActualDeliverySupplyChainEvent");
            date(&mut out, 8, "ram:OccurrenceDateTime", delivery);
            close(&mut out, 6, "ram:ActualDeliverySupplyChainEvent");
            close(&mut out, 4, "ram:ApplicableHeaderTradeDelivery");
        }
        // Required by the schema even without data.
        None => out.push_str("    <ram:ApplicableHeaderTradeDelivery/>\n"),
    }

    open(&mut out, 4, "ram:ApplicableHeaderTradeSettlement");
    el(&mut out, 6, "ram:PaymentReference", &invoice.number);
    el(&mut out, 6, "ram:InvoiceCurrencyCode", &invoice.currency);
    if let Some(payment) = &invoice.payment {
        open(&mut out, 6, "ram:SpecifiedTradeSettlementPaymentMeans");
        // 58: SEPA credit transfer.
        el(&mut out, 8, "ram:TypeCode", "58");
        open(&mut out, 8, "ram:PayeePartyCreditorFinancialAccount");
        el(&mut out, 10, "ram:IBANID", &payment.iban);
        if let Some(name) = &payment.account_name {
            el(&mut out, 10, "ram:AccountName", name);
        }
        close(&mut out, 8, "ram:PayeePartyCreditorFinancialAccount");
        if let Some(bic) = &payment.bic {
            open(
                &mut out,
                8,
                "ram:PayeeSpecifiedCreditorFinancialInstitution",
            );
            el(&mut out, 10, "ram:BICID", bic);
            close(
                &mut out,
                8,
                "ram:PayeeSpecifiedCreditorFinancialInstitution",
            );
        }
        close(&mut out, 6, "ram:SpecifiedTradeSettlementPaymentMeans");
    }
    for group in &invoice.breakdown {
        open(&mut out, 6, "ram:ApplicableTradeTax");
        el(&mut out, 8, "ram:CalculatedAmount", &group.amount.amount());
        el(&mut out, 8, "ram:TypeCode", "VAT");
        if let Some(reason) = &group.tax.exemption_reason {
            el(&mut out, 8, "ram:ExemptionReason", reason);
        }
        el(&mut out, 8, "ram:BasisAmount", &group.basis.amount());
        el(&mut out, 8, "ram:CategoryCode", group.tax.category);
        if let Some(code) = &group.tax.exemption_code {
            el(&mut out, 8, "ram:ExemptionReasonCode", code);
        }
        el(
            &mut out,
            8,
            "ram:RateApplicablePercent",
            &group.tax.rate.to_string(),
        );
        close(&mut out, 6, "ram:ApplicableTradeTax");
    }
    if let Some((start, end)) = &invoice.billing_period {
        open(&mut out, 6, "ram:BillingSpecifiedPeriod");
        date(&mut out, 8, "ram:StartDateTime", start);
        date(&mut out, 8, "ram:EndDateTime", end);
        close(&mut out, 6, "ram:BillingSpecifiedPeriod");
    }
    if invoice.payment_terms.is_some() || invoice.due_date.is_some() {
        open(&mut out, 6, "ram:SpecifiedTradePaymentTerms");
        if let Some(terms) = &invoice.payment_terms {
            el(&mut out, 8, "ram:Description", terms);
        }
        if let Some(due) = &invoice.due_date {
            date(&mut out, 8, "ram:DueDateDateTime", due);
        }
        close(&mut out, 6, "ram:SpecifiedTradePaymentTerms");
    }
    open(
        &mut out,
        6,
        "ram:SpecifiedTradeSettlementHeaderMonetarySummation",
    );
    el(
        &mut out,
        8,
        "ram:LineTotalAmount",
        &invoice.line_total.amount(),
    );
    el(
        &mut out,
        8,
        "ram:TaxBasisTotalAmount",
        &invoice.line_total.amount(),
    );
    let _ = writeln!(
        out,
        "        <ram:TaxTotalAmount currencyID=\"{}\">{}</ram:TaxTotalAmount>",
        escape(&invoice.currency),
        invoice.tax_total.amount()
    );
    el(
        &mut out,
        8,
        "ram:GrandTotalAmount",
        &invoice.grand_total.amount(),
    );
    el(
        &mut out,
        8,
        "ram:DuePayableAmount",
        &invoice.grand_total.amount(),
    );
    close(
        &mut out,
        6,
        "ram:SpecifiedTradeSettlementHeaderMonetarySummation",
    );
    close(&mut out, 4, "ram:ApplicableHeaderTradeSettlement");

    close(&mut out, 2, "rsm:SupplyChainTradeTransaction");
    out.push_str("</rsm:CrossIndustryInvoice>\n");
    out
}
