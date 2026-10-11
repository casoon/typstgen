//! Archive manifests: record how each PDF was built, then prove later that
//! nothing changed and that the PDF can be rebuilt byte for byte
//! (`archive` feature).
//!
//! A manifest (`archive.json`) maps each PDF to its source file, the compile
//! options, and the SHA-256 of the PDF, of every file Typst read (templates,
//! data, images, package files) and of every font file loaded from disk.
//! Paths are relative to the working directory, like the paths in
//! `typstgen.toml`, so record and verify from the project directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CompileOptions, Config, Error, Output, PdfStandard, Result};

/// One build to record: what [`crate::compile_with_options`] gets, plus where
/// the PDF goes and where the config came from.
#[derive(Debug, Clone)]
pub struct Job {
    pub input: PathBuf,
    pub output: PathBuf,
    /// The `typstgen.toml` used, if any.
    pub config: Option<PathBuf>,
    pub inputs: BTreeMap<String, String>,
    pub pdf_standards: Vec<PdfStandard>,
    pub creation_timestamp: Option<i64>,
    pub pdf_tags: bool,
    /// Invoice JSON and company JSON for a ZUGFeRD invoice.
    pub zugferd: Option<(PathBuf, PathBuf)>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    documents: BTreeMap<String, Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<String>,
    sha256: String,
    pages: usize,
    typstgen: String,
    typst: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    inputs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pdf_standards: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creation_timestamp: Option<i64>,
    pdf_tags: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    zugferd: Option<Zugferd>,
    /// SHA-256 of every file read, by path (packages: `@ns/name:version/path`).
    files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    fonts: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Zugferd {
    invoice: String,
    company: String,
}

/// The result of checking one document.
#[derive(Debug, Clone)]
pub struct Report {
    /// The PDF path as recorded.
    pub document: String,
    /// Empty when the PDF is unchanged and the rebuild is identical.
    pub problems: Vec<String>,
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `path` relative to the working directory where possible.
fn display(path: &Path) -> String {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.canonicalize().ok())
        .and_then(|cwd| absolute.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or(absolute)
        .display()
        .to_string()
}

fn typst_version() -> String {
    let version = typst::utils::version();
    format!(
        "{}.{}.{}",
        version.major(),
        version.minor(),
        version.patch()
    )
}

fn load(manifest: &Path) -> Result<Manifest> {
    if !manifest.exists() {
        return Ok(Manifest {
            version: 1,
            ..Manifest::default()
        });
    }
    let text = std::fs::read_to_string(manifest)?;
    serde_json::from_str(&text)
        .map_err(|error| Error::Archive(format!("{}: {error}", manifest.display())))
}

struct Built {
    output: Output,
    files: BTreeMap<String, String>,
    fonts: BTreeMap<String, String>,
}

fn build(job: &Job, config: &Config) -> Result<Built> {
    #[allow(unused_mut)]
    let mut options = CompileOptions {
        inputs: job.inputs.clone(),
        pdf_standards: job.pdf_standards.clone(),
        creation_timestamp: job.creation_timestamp,
        pdf_tags: job.pdf_tags,
        ..CompileOptions::default()
    };
    let mut files = BTreeMap::new();
    files.insert(display(&job.input), sha256(&std::fs::read(&job.input)?));
    if let Some(path) = &job.config {
        files.insert(display(path), sha256(&std::fs::read(path)?));
    }
    if let Some((invoice, company)) = &job.zugferd {
        #[cfg(feature = "zugferd")]
        {
            let invoice_json = std::fs::read_to_string(invoice)?;
            let company_json = std::fs::read_to_string(company)?;
            files.insert(display(invoice), sha256(invoice_json.as_bytes()));
            files.insert(display(company), sha256(company_json.as_bytes()));
            options.zugferd_xml =
                Some(crate::zugferd::xml_from_docgen(&invoice_json, &company_json)?.into_bytes());
        }
        #[cfg(not(feature = "zugferd"))]
        {
            let _ = (invoice, company);
            return Err(Error::Archive(
                "this build of typstgen has no ZUGFeRD support".to_owned(),
            ));
        }
    }

    let (output, world) = crate::compile_keeping_world(&job.input, config, &options)?;
    for (name, path, bytes) in world.dependencies() {
        let key = if name.starts_with('@') {
            name
        } else {
            display(&path)
        };
        files.insert(key, sha256(bytes.as_slice()));
    }
    let fonts = world
        .used_font_files()
        .into_iter()
        .map(|(path, bytes)| (display(&path), sha256(bytes.as_slice())))
        .collect();
    Ok(Built {
        output,
        files,
        fonts,
    })
}

/// Compiles `job`, writes the PDF and records it in `manifest` (created if
/// missing, the entry for the same PDF replaced).
///
/// # Errors
///
/// Compile errors as from [`crate::compile_with_options`]; I/O errors; and
/// [`Error::Archive`] for an unreadable manifest.
pub fn compile_and_record(manifest: &Path, job: &Job, config: &Config) -> Result<Output> {
    let built = build(job, config)?;
    std::fs::write(&job.output, &built.output.pdf)?;

    let mut archive = load(manifest)?;
    archive.documents.insert(
        display(&job.output),
        Entry {
            source: display(&job.input),
            config: job.config.as_deref().map(display),
            sha256: sha256(&built.output.pdf),
            pages: built.output.pages,
            typstgen: env!("CARGO_PKG_VERSION").to_owned(),
            typst: typst_version(),
            inputs: job.inputs.clone(),
            pdf_standards: job.pdf_standards.iter().map(ToString::to_string).collect(),
            creation_timestamp: job.creation_timestamp,
            pdf_tags: job.pdf_tags,
            zugferd: job.zugferd.as_ref().map(|(invoice, company)| Zugferd {
                invoice: display(invoice),
                company: display(company),
            }),
            files: built.files,
            fonts: built.fonts,
        },
    );
    let text = serde_json::to_string_pretty(&archive)
        .map_err(|error| Error::Archive(error.to_string()))?;
    std::fs::write(manifest, text + "\n")?;
    Ok(built.output)
}

/// Checks every document in `manifest`: the stored PDF is unchanged, the
/// sources and fonts are the recorded ones, and a rebuild with the recorded
/// options produces the same bytes.
///
/// With `rebuild_only`, the stored PDFs are not read: the rebuild is only
/// compared with the recorded SHA-256. That is the check for a CI job, which
/// has the sources from version control but not the archived PDFs.
///
/// # Errors
///
/// [`Error::Archive`] if the manifest cannot be read. Problems with single
/// documents are reported in their [`Report`].
pub fn verify(manifest: &Path, rebuild_only: bool) -> Result<Vec<Report>> {
    let archive = load(manifest)?;
    if archive.documents.is_empty() {
        return Err(Error::Archive(format!(
            "{}: no documents recorded",
            manifest.display()
        )));
    }
    let mut reports = Vec::new();
    for (document, entry) in &archive.documents {
        let mut problems = Vec::new();
        if !rebuild_only {
            match std::fs::read(document) {
                Ok(pdf) if sha256(&pdf) == entry.sha256 => {}
                Ok(_) => problems.push("the PDF differs from the recorded one".to_owned()),
                Err(error) => problems.push(format!("the PDF cannot be read: {error}")),
            }
        }
        let pdf_standards: std::result::Result<Vec<PdfStandard>, String> = entry
            .pdf_standards
            .iter()
            .map(|name| name.parse())
            .collect();
        let job = Job {
            input: PathBuf::from(&entry.source),
            output: PathBuf::from(document),
            config: entry.config.as_ref().map(PathBuf::from),
            inputs: entry.inputs.clone(),
            pdf_standards: pdf_standards.map_err(Error::Archive)?,
            creation_timestamp: entry.creation_timestamp,
            pdf_tags: entry.pdf_tags,
            zugferd: entry
                .zugferd
                .as_ref()
                .map(|z| (PathBuf::from(&z.invoice), PathBuf::from(&z.company))),
        };
        let rebuilt = Config::load(job.config.as_deref()).and_then(|config| build(&job, &config));
        match rebuilt {
            Err(error) => problems.push(format!("the rebuild failed: {error}")),
            Ok(built) => {
                let before = problems.len();
                for (path, hash) in &entry.files {
                    match built.files.get(path) {
                        Some(now) if now == hash => {}
                        Some(_) => problems.push(format!("changed: {path}")),
                        None => problems.push(format!("no longer used or missing: {path}")),
                    }
                }
                for path in built
                    .files
                    .keys()
                    .filter(|path| !entry.files.contains_key(*path))
                {
                    problems.push(format!("newly used: {path}"));
                }
                for (path, hash) in &entry.fonts {
                    if built.fonts.get(path) != Some(hash) {
                        problems.push(format!("font changed or missing: {path}"));
                    }
                }
                let inputs_changed = problems.len() > before;
                if sha256(&built.output.pdf) != entry.sha256 {
                    let mut reason = String::from("the rebuild differs from the recorded PDF");
                    // With unchanged inputs, these are the remaining causes.
                    if !inputs_changed && entry.typst != typst_version() {
                        reason.push_str(&format!(
                            " (recorded with Typst {}, rebuilt with {})",
                            entry.typst,
                            typst_version()
                        ));
                    }
                    if !inputs_changed && entry.creation_timestamp.is_none() {
                        reason.push_str(" (recorded without --creation-timestamp)");
                    }
                    problems.push(reason);
                }
            }
        }
        reports.push(Report {
            document: document.clone(),
            problems,
        });
    }
    Ok(reports)
}
