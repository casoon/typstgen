//! # typstgen
//!
//! Compile an existing `.typ` file to PDF using an embedded Typst engine —
//! no external `typst` binary, no subprocess. Template resolution is
//! config-driven (see [`Config`]) rather than hardcoded.
//!
//! Usable as a CLI (`cli` feature), a Rust library (always), and from
//! JavaScript via Wasm (`wasm` feature).
//!
//! ```rust,ignore
//! use typstgen::{compile, Config};
//!
//! let config = Config::load(None)?;
//! let pdf_bytes = compile("documents/concepts/2026/example.typ".as_ref(), &config)?;
//! std::fs::write("example.pdf", pdf_bytes)?;
//! # Ok::<(), typstgen::Error>(())
//! ```
//!
//! See `docs/PLAN.md` in the repository for the implementation plan —
//! [`compile`] is a stub until Phase 1 lands.

pub mod config;
pub mod error;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use config::Config;
pub use error::{Error, Result};

use std::path::Path;

/// Compile a `.typ` file to PDF bytes using the embedded Typst engine.
///
/// `#import` targets inside the file are resolved against
/// [`Config::resolve_template_path`].
///
/// # Errors
///
/// Returns [`Error::InputNotFound`] if `input` does not exist, or
/// [`Error::TemplatePathNotFound`] if no configured template directory
/// exists on disk.
pub fn compile(input: &Path, config: &Config) -> Result<Vec<u8>> {
    if !input.exists() {
        return Err(Error::InputNotFound(input.to_path_buf()));
    }
    let _template_path = config.resolve_template_path()?;

    // TODO(Phase 1, see docs/PLAN.md): embed a typst::World implementation
    // (adapt casoon/typst-business-templates:src/world.rs and
    // casoon/renderreport:src/engine/world.rs — both already solve this,
    // this crate should not solve it a third time from scratch) and drive
    // `typst::compile` + `typst_pdf::pdf` from here.
    Err(Error::NotImplemented(
        "embedded typst::World compilation lands in Phase 1 — see docs/PLAN.md",
    ))
}
