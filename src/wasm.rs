//! Wasm bindings. No filesystem is available in a JS host, so callers pass
//! the `.typ` source and imported templates as a virtual filesystem.

use std::collections::BTreeMap;

use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::{compile_world, world::TypstWorld, CompileOptions};

/// Filesystem-free input for [`compile`]. Each `files` key is a virtual path
/// used by Typst imports, for example `templates/shared.typ`. The remaining
/// fields mirror [`CompileOptions`]; all are optional.
#[derive(Deserialize)]
struct CompileRequest {
    source: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    pdf_standards: Vec<String>,
    creation_timestamp: Option<i64>,
    /// Comma-separated page ranges such as `1-3,5`.
    pages: Option<String>,
    pdf_tags: Option<bool>,
}

impl CompileRequest {
    fn options(&self) -> Result<CompileOptions, String> {
        Ok(CompileOptions {
            inputs: self.inputs.clone(),
            pdf_standards: self
                .pdf_standards
                .iter()
                .map(|name| name.parse())
                .collect::<Result<_, _>>()?,
            creation_timestamp: self.creation_timestamp,
            pages: match &self.pages {
                Some(pages) => pages
                    .split(',')
                    .map(|range| range.trim().parse())
                    .collect::<Result<_, _>>()?,
                None => Vec::new(),
            },
            pdf_tags: self.pdf_tags.unwrap_or(true),
        })
    }
}

/// Compile Typst source and its imported text files to PDF bytes.
///
/// The argument must be JSON like
/// `{"source":"#import \"templates/shared.typ\": title", "files":{"templates/shared.typ":"#let title = [Hello]"}}`.
/// Optional fields: `inputs` (object of strings, visible as `sys.inputs`),
/// `pdf_standards` (for example `["a-2b"]`), `creation_timestamp` (Unix
/// seconds), `pages` (for example `"1-3,5"`) and `pdf_tags` (boolean).
#[wasm_bindgen]
pub fn compile(request_json: &str) -> Result<Vec<u8>, JsValue> {
    let request: CompileRequest = serde_json::from_str(request_json)
        .map_err(|error| JsValue::from_str(&format!("invalid compile request JSON: {error}")))?;
    let options = request
        .options()
        .map_err(|error| JsValue::from_str(&format!("invalid compile request: {error}")))?;
    let world = TypstWorld::from_virtual(
        request.source,
        request
            .files
            .into_iter()
            .map(|(path, contents)| (path, contents.into_bytes())),
        &options,
    )
    .map_err(|error| JsValue::from_str(&error))?;

    compile_world(&world, &options)
        .map(|output| output.pdf)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}
