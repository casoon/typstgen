//! Wasm bindings. No filesystem is available in a JS host, so callers pass
//! the `.typ` source and any imported template sources directly instead of
//! paths — see docs/PLAN.md for the planned virtual-file request shape.

use wasm_bindgen::prelude::*;

/// Placeholder entry point until Phase 1 (embedded `typst::World`) lands.
#[wasm_bindgen]
pub fn compile(_typst_source: &str) -> Result<Vec<u8>, JsValue> {
    Err(JsValue::from_str(
        "not yet implemented — see docs/PLAN.md Phase 1",
    ))
}
