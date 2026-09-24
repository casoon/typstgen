//! Wasm bindings. No filesystem is available in a JS host, so callers pass
//! the `.typ` source and imported templates as a virtual filesystem.

use std::collections::BTreeMap;

use serde::Deserialize;
use wasm_bindgen::prelude::*;

use crate::{compile_world, world::TypstWorld};

/// Filesystem-free input for [`compile`]. Each `files` key is a virtual path
/// used by Typst imports, for example `templates/shared.typ`.
#[derive(Deserialize)]
struct CompileRequest {
    source: String,
    #[serde(default)]
    files: BTreeMap<String, String>,
}

/// Compile Typst source and its imported text files to PDF bytes.
///
/// The argument must be JSON like
/// `{"source":"#import \"templates/shared.typ\": title", "files":{"templates/shared.typ":"#let title = [Hello]"}}`.
#[wasm_bindgen]
pub fn compile(request_json: &str) -> Result<Vec<u8>, JsValue> {
    let request: CompileRequest = serde_json::from_str(request_json)
        .map_err(|error| JsValue::from_str(&format!("invalid compile request JSON: {error}")))?;
    let world = TypstWorld::from_virtual(
        request.source,
        request
            .files
            .into_iter()
            .map(|(path, contents)| (path, contents.into_bytes())),
    )
    .map_err(|error| JsValue::from_str(&error))?;

    compile_world(&world).map_err(|error| JsValue::from_str(&error.to_string()))
}
