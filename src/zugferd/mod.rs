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

mod cii;
pub(crate) mod decimal;
pub(crate) mod model;
pub(crate) mod pdf;

pub use cii::GUIDELINE;
pub use pdf::FILE_NAME;

use crate::{Error, Result};

/// Checks a business-templates invoice and returns its CII XML (EN 16931 profile).
///
/// # Errors
///
/// [`Error::Zugferd`] lists every problem found, one per line, for example a
/// missing VAT ID, an unknown unit or totals that do not add up.
pub fn xml_from_docgen(invoice_json: &str, company_json: &str) -> Result<String> {
    let parse = |name: &str, text: &str| {
        serde_json::from_str::<serde_json::Value>(text)
            .map_err(|error| Error::Zugferd(format!("{name}: {error}")))
    };
    let invoice = parse("invoice JSON", invoice_json)?;
    let company = parse("company JSON", company_json)?;
    let invoice = model::Invoice::from_docgen(&invoice, &company)
        .map_err(|problems| Error::Zugferd(problems.join("\n")))?;
    Ok(cii::write(&invoice))
}

/// The line typstgen appends to the main file to embed the invoice XML. The
/// name in the PDF is the root-relative path, so `/factur-x.xml`; the bytes
/// come from the in-memory file at [`VIRTUAL_PATH`].
pub(crate) const ATTACH: &str = "\n#pdf.attach(\"/factur-x.xml\", read(\"/.typstgen/factur-x.xml\", encoding: none), relationship: \"alternative\", mime-type: \"text/xml\", description: \"Factur-X/ZUGFeRD invoice\")\n";

/// The virtual path of the embedded XML in the project.
pub(crate) const VIRTUAL_PATH: &str = ".typstgen/factur-x.xml";
