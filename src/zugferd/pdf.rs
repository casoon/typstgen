//! Adds the Factur-X XMP schema to a PDF/A-3 that Typst wrote.
//!
//! Typst embeds `factur-x.xml` itself (`pdf.attach` with relationship
//! "alternative"), but it cannot write custom XMP. The updated metadata stream
//! and catalog are appended as an incremental update, so Typst's bytes stay
//! untouched. This relies on the classic cross-reference table Typst writes.

use std::fmt::Write;

const FX_NAMESPACE: &str = "urn:factur-x:pdfa:CrossIndustryDocument:invoice:1p0#";

/// The name of the embedded invoice file required by Factur-X / ZUGFeRD 2.x.
pub const FILE_NAME: &str = "factur-x.xml";

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|window| window == needle)
}

/// The text of the dictionary starting at `start` (which points at `<<`).
fn dictionary(bytes: &[u8], start: usize) -> Option<&str> {
    let mut depth = 0usize;
    let mut i = start;
    while i + 1 < bytes.len() {
        match &bytes[i..i + 2] {
            b"<<" => {
                depth += 1;
                i += 2;
            }
            b">>" => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return std::str::from_utf8(&bytes[start..i]).ok();
                }
            }
            _ => i += 1,
        }
    }
    None
}

/// The integer following `key` in a dictionary text (`/Size 12`).
fn integer_after(text: &str, key: &str) -> Option<u64> {
    let rest = &text[text.find(key)? + key.len()..];
    let digits: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Offset of `n 0 obj` at the start of a line.
fn find_object(bytes: &[u8], number: u64) -> Option<usize> {
    let header = format!("{number} 0 obj");
    let mut from = 0;
    while let Some(at) = find(bytes, header.as_bytes(), from) {
        if at == 0 || matches!(bytes[at - 1], b'\n' | b'\r') {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// The raw value of `key` up to the next key or the end (`/ID [<..> <..>]`).
fn raw_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let start = text.find(key)? + key.len();
    let rest = &text[start..];
    let end = rest.find(']').map(|end| end + 1)?;
    Some(rest[..end].trim())
}

fn xmp_extension() -> String {
    let property = |name: &str, description: &str| {
        format!(
            "<rdf:li rdf:parseType=\"Resource\"><pdfaProperty:name>{name}</pdfaProperty:name>\
             <pdfaProperty:valueType>Text</pdfaProperty:valueType>\
             <pdfaProperty:category>external</pdfaProperty:category>\
             <pdfaProperty:description>{description}</pdfaProperty:description></rdf:li>"
        )
    };
    format!(
        "<rdf:li rdf:parseType=\"Resource\">\
         <pdfaSchema:schema>Factur-X PDFA Extension Schema</pdfaSchema:schema>\
         <pdfaSchema:namespaceURI>{FX_NAMESPACE}</pdfaSchema:namespaceURI>\
         <pdfaSchema:prefix>fx</pdfaSchema:prefix>\
         <pdfaSchema:property><rdf:Seq>{}{}{}{}</rdf:Seq></pdfaSchema:property></rdf:li>",
        property("DocumentFileName", "The name of the embedded XML document"),
        property(
            "DocumentType",
            "The type of the hybrid document in capital letters, e.g. INVOICE or ORDER"
        ),
        property(
            "Version",
            "The actual version of the standard applying to the embedded XML document"
        ),
        property(
            "ConformanceLevel",
            "The conformance level of the embedded XML document"
        ),
    )
}

fn xmp_description() -> String {
    format!(
        "<rdf:Description rdf:about=\"\" xmlns:fx=\"{FX_NAMESPACE}\">\
         <fx:DocumentType>INVOICE</fx:DocumentType>\
         <fx:DocumentFileName>{FILE_NAME}</fx:DocumentFileName>\
         <fx:Version>1.0</fx:Version>\
         <fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>\
         </rdf:Description>"
    )
}

/// Returns `pdf` with the Factur-X schema added to its XMP metadata.
pub fn add_factur_x_metadata(mut pdf: Vec<u8>) -> Result<Vec<u8>, String> {
    let unsupported = |what: &str| format!("unsupported PDF structure: {what}");

    // Current XMP packet.
    let packet_start = find(&pdf, b"<?xpacket begin", 0).ok_or_else(|| unsupported("no XMP"))?;
    let packet_end_tag = find(&pdf, b"<?xpacket end", packet_start)
        .ok_or_else(|| unsupported("unterminated XMP"))?;
    let packet_end = find(&pdf, b"?>", packet_end_tag)
        .map(|end| end + 2)
        .ok_or_else(|| unsupported("unterminated XMP"))?;
    let mut xmp = std::str::from_utf8(&pdf[packet_start..packet_end])
        .map_err(|_| unsupported("XMP is not UTF-8"))?
        .to_owned();
    let bag = "<pdfaExtension:schemas><rdf:Bag>";
    let at = xmp
        .find(bag)
        .map(|at| at + bag.len())
        .ok_or_else(|| unsupported("XMP has no PDF/A extension schemas"))?;
    xmp.insert_str(at, &xmp_extension());
    let rdf_end = xmp
        .rfind("</rdf:RDF>")
        .ok_or_else(|| unsupported("XMP has no rdf:RDF"))?;
    xmp.insert_str(rdf_end, &xmp_description());

    // Trailer of the last cross-reference section.
    let startxref = rfind(&pdf, b"startxref").ok_or_else(|| unsupported("no startxref"))?;
    let previous = std::str::from_utf8(&pdf[startxref + 9..])
        .ok()
        .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
        .ok_or_else(|| unsupported("unreadable startxref"))?;
    let trailer = rfind(&pdf[..startxref], b"trailer")
        .ok_or_else(|| unsupported("no trailer (cross-reference stream)"))?;
    let trailer_dict_start =
        find(&pdf, b"<<", trailer).ok_or_else(|| unsupported("unreadable trailer"))?;
    let trailer_dict = dictionary(&pdf, trailer_dict_start)
        .ok_or_else(|| unsupported("unreadable trailer"))?
        .to_owned();
    let size = integer_after(&trailer_dict, "/Size").ok_or_else(|| unsupported("no /Size"))?;
    let root = integer_after(&trailer_dict, "/Root").ok_or_else(|| unsupported("no /Root"))?;
    let info = integer_after(&trailer_dict, "/Info");
    let id = raw_after(&trailer_dict, "/ID").ok_or_else(|| unsupported("no /ID"))?;

    // Catalog, with /Metadata pointing to the new stream.
    let catalog_obj = find_object(&pdf, root).ok_or_else(|| unsupported("catalog not found"))?;
    let catalog_start =
        find(&pdf, b"<<", catalog_obj).ok_or_else(|| unsupported("unreadable catalog"))?;
    let catalog =
        dictionary(&pdf, catalog_start).ok_or_else(|| unsupported("unreadable catalog"))?;
    let metadata_id = size;
    let catalog = match catalog.find("/Metadata") {
        Some(at) => {
            let rest = &catalog[at + "/Metadata".len()..];
            let r = rest
                .find('R')
                .ok_or_else(|| unsupported("unreadable /Metadata"))?;
            format!(
                "{}/Metadata {metadata_id} 0 R{}",
                &catalog[..at],
                &rest[r + 1..]
            )
        }
        None => format!(
            "{} /Metadata {metadata_id} 0 R>>",
            &catalog[..catalog.len() - 2]
        ),
    };

    // Append the update.
    if !pdf.ends_with(b"\n") {
        pdf.push(b'\n');
    }
    let metadata_offset = pdf.len();
    let _ = write!(
        Appender(&mut pdf),
        "{metadata_id} 0 obj\n<</Type /Metadata /Subtype /XML /Length {}>>\nstream\n{xmp}\nendstream\nendobj\n",
        xmp.len()
    );
    let catalog_offset = pdf.len();
    let _ = write!(Appender(&mut pdf), "{root} 0 obj\n{catalog}\nendobj\n");
    let xref_offset = pdf.len();
    let mut entries = vec![(root, catalog_offset), (metadata_id, metadata_offset)];
    entries.sort();
    let mut update = String::from("xref\n");
    for (number, offset) in entries {
        let _ = write!(update, "{number} 1\n{offset:010} 00000 n \n");
    }
    let info = info
        .map(|info| format!(" /Info {info} 0 R"))
        .unwrap_or_default();
    let _ = write!(
        update,
        "trailer\n<</Size {} /Root {root} 0 R{info} /ID {id} /Prev {previous}>>\nstartxref\n{xref_offset}\n%%EOF\n",
        metadata_id + 1
    );
    pdf.extend_from_slice(update.as_bytes());
    Ok(pdf)
}

/// `write!` into a byte vector.
struct Appender<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for Appender<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
