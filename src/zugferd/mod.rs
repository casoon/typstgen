//! ZUGFeRD / Factur-X e-invoices (profile EN 16931), `zugferd` feature.
//!
//! The invoice data is the business-templates invoice JSON plus company.json,
//! the same files its invoice template renders. [`xml_from_docgen`] checks them and
//! writes the CII XML; [`CompileOptions::zugferd_xml`](crate::CompileOptions)
//! embeds it into a PDF/A-3 as `factur-x.xml` and adds the Factur-X metadata.
//!
//! The checks cover the EN 16931 rules this data can violate (required fields,
//! VAT categories, line and total arithmetic). Validate the result with an
//! external validator such as Mustang before relying on it.
//! Sales-tax invoices use a separate component model and are rejected by this
//! VAT adapter; their configured PDF data must not be treated as CII input.
//!
//! [`ubl_xml_from_docgen`] writes the same invoice as a pure-XML e-invoice in
//! UBL 2.1: XRechnung 3.0 or Peppol BIS Billing 3.0 ([`UblFormat`]).

mod cii;
pub(crate) mod decimal;
pub(crate) mod model;
pub(crate) mod pdf;
mod ubl;

pub use cii::GUIDELINE;
pub use pdf::FILE_NAME;
pub use ubl::{UblFormat, PROFILE_ID};

use crate::{Error, Result};

/// Checks a business-templates invoice and returns its CII XML (EN 16931 profile).
///
/// # Errors
///
/// [`Error::Zugferd`] lists every problem found, one per line, for example a
/// missing VAT ID, an unknown unit or totals that do not add up.
pub fn xml_from_docgen(invoice_json: &str, company_json: &str) -> Result<String> {
    Ok(cii::write(&invoice(invoice_json, company_json)?))
}

/// Checks a business-templates invoice and returns it as UBL 2.1 XML in
/// `format` (XRechnung 3.0 or Peppol BIS Billing 3.0).
///
/// The XML is written from the same checked invoice model as the CII XML, never
/// from a rendered PDF. Writing the file is not a transmission: sending it over
/// the Peppol network or to a public-sector portal is a separate step.
///
/// # Errors
///
/// [`Error::Zugferd`] lists every problem found, one per line, each starting
/// with the JSON field to fix: the EN 16931 checks of [`xml_from_docgen`] plus
/// the fields `format` requires (electronic addresses, buyer reference, …).
pub fn ubl_xml_from_docgen(
    format: UblFormat,
    invoice_json: &str,
    company_json: &str,
) -> Result<String> {
    let invoice = invoice(invoice_json, company_json)?;
    ubl::check(format, &invoice).map_err(|problems| Error::Zugferd(problems.join("\n")))?;
    Ok(ubl::write(format, &invoice))
}

fn invoice(invoice_json: &str, company_json: &str) -> Result<model::Invoice> {
    let parse = |name: &str, text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .map_err(|error| Error::Zugferd(format!("{name}: {error}")))
    };
    let invoice = parse("invoice JSON", invoice_json)?;
    let company = parse("company JSON", company_json)?;
    model::Invoice::from_docgen(&invoice, &company)
        .map_err(|problems| Error::Zugferd(problems.join("\n")))
}

/// The line typstgen appends to the main file to embed the invoice XML. The
/// name in the PDF is the root-relative path, so `/factur-x.xml`; the bytes
/// come from the in-memory file at [`VIRTUAL_PATH`].
pub(crate) const ATTACH: &str = "\n#pdf.attach(\"/factur-x.xml\", read(\"/.typstgen/factur-x.xml\", encoding: none), relationship: \"alternative\", mime-type: \"text/xml\", description: \"Factur-X/ZUGFeRD invoice\")\n";

/// The virtual path of the embedded XML in the project.
pub(crate) const VIRTUAL_PATH: &str = ".typstgen/factur-x.xml";
