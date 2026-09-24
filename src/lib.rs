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
//! let pdf_bytes = compile("example.typ".as_ref(), &config)?;
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

use typst::diag::{Severity, SourceDiagnostic};
use typst::{World, WorldExt};

/// Compile a `.typ` file to PDF bytes using the embedded Typst engine.
///
/// `#import` targets inside the file are resolved against the input's own
/// directory and every directory from [`Config::template_roots`]. Template
/// directories are optional: a document without imports needs none.
///
/// Warnings are discarded; use [`compile_with_warnings`] to keep them.
///
/// # Errors
///
/// Returns [`Error::InputNotFound`] if `input` does not exist, or
/// [`Error::Compile`] if Typst reports errors.
pub fn compile(input: &Path, config: &Config) -> Result<Vec<u8>> {
    compile_with_warnings(input, config).map(|output| output.pdf)
}

/// A compiled PDF together with the warnings Typst reported.
#[derive(Debug, Clone)]
pub struct Output {
    /// The PDF bytes.
    pub pdf: Vec<u8>,
    /// Formatted warnings, each starting with `path:line:column: warning:`
    /// when Typst knows the location.
    pub warnings: Vec<String>,
}

/// Like [`compile`], but also returns Typst's warnings.
///
/// # Errors
///
/// See [`compile`]. The message of [`Error::Compile`] lists all errors and
/// warnings with file, line and column.
pub fn compile_with_warnings(input: &Path, config: &Config) -> Result<Output> {
    if !input.exists() {
        return Err(Error::InputNotFound(input.to_path_buf()));
    }
    let world = world::TypstWorld::from_file(input, config)?;
    compile_world(&world)
}

fn compile_world(world: &world::TypstWorld) -> Result<Output> {
    let warned = typst::compile(world);
    let warnings: Vec<String> = warned
        .warnings
        .iter()
        .map(|warning| format_diagnostic(world, warning))
        .collect();
    let fail = |errors: typst::ecow::EcoVec<SourceDiagnostic>| {
        let mut lines: Vec<String> = errors
            .iter()
            .map(|error| format_diagnostic(world, error))
            .collect();
        lines.extend(warnings.iter().cloned());
        Error::Compile(lines.join("\n"))
    };

    let document = warned.output.map_err(fail)?;
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(fail)?;
    Ok(Output { pdf, warnings })
}

/// Formats a diagnostic as `path:line:column: severity: message`, followed
/// by its hints.
fn format_diagnostic(world: &world::TypstWorld, diagnostic: &SourceDiagnostic) -> String {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let location = diagnostic
        .span
        .id()
        .map(|id| {
            let path = world.display_path(id);
            let line_column = world.range(diagnostic.span).and_then(|range| {
                world
                    .source(id)
                    .ok()?
                    .lines()
                    .byte_to_line_column(range.start)
            });
            match line_column {
                Some((line, column)) => format!("{path}:{}:{}: ", line + 1, column + 1),
                None => format!("{path}: "),
            }
        })
        .unwrap_or_default();
    let hints: String = diagnostic
        .hints
        .iter()
        .map(|hint| format!("\n  hint: {}", hint.v))
        .collect();
    format!("{location}{severity}: {}{hints}", diagnostic.message)
}
