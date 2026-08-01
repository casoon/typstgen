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
pub mod config;
pub mod error;
mod world;

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
    let world = world::TypstWorld::from_file(input, config)?;
    compile_world(&world)
}

fn compile_world(world: &world::TypstWorld) -> Result<Vec<u8>> {
    let document = typst::compile(world)
        .output
        .map_err(|errors| Error::Compile(format_diagnostics(&errors)))?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| Error::Compile(format_diagnostics(&errors)))
}

fn format_diagnostics(errors: &[typst::diag::SourceDiagnostic]) -> String {
    errors
        .iter()
        .map(|error| {
            let hints = error
                .hints
                .iter()
                .map(|hint| format!("\n  hint: {hint}"))
                .collect::<String>();
            format!("{}{}", error.message, hints)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
