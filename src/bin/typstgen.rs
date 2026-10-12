use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use typstgen::{CompileOptions, Config, PageRange, PdfStandard};

#[derive(Parser)]
#[command(name = "typstgen")]
#[command(version)]
#[command(about = "Compile .typ files to PDF with an embedded Typst engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a single .typ file to PDF
    Compile {
        /// Path to the .typ file
        input: PathBuf,
        /// Output PDF path (defaults to input with .pdf extension)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Path to typstgen.toml (defaults to ./typstgen.toml if present)
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Add a string key-value pair visible through `sys.inputs`
        #[arg(long = "input", value_name = "KEY=VALUE", value_parser = parse_input)]
        inputs: Vec<(String, String)>,
        /// PDF standards to enforce, comma-separated (e.g. a-2b, ua-1, 1.7)
        #[arg(long = "pdf-standard", value_name = "STANDARD", value_delimiter = ',')]
        pdf_standards: Vec<PdfStandard>,
        /// Creation date as a Unix timestamp; also used by datetime.today()
        #[arg(long, value_name = "UNIX_SECONDS", env = "SOURCE_DATE_EPOCH")]
        creation_timestamp: Option<i64>,
        /// Pages to export, comma-separated (e.g. 1-3,5,8-); implies an untagged PDF
        #[arg(
            long,
            value_name = "PAGES",
            value_delimiter = ',',
            allow_hyphen_values = true
        )]
        pages: Vec<PageRange>,
        /// Write an untagged PDF (smaller, but without document structure)
        #[arg(long)]
        no_pdf_tags: bool,
        /// Embed a ZUGFeRD / Factur-X invoice (EN 16931) built from this docgen invoice JSON;
        /// needs --company and a PDF/A-3 standard
        #[cfg(feature = "zugferd")]
        #[arg(long, value_name = "INVOICE_JSON", requires = "company")]
        zugferd: Option<PathBuf>,
        /// company.json with the seller's data, for --zugferd
        #[cfg(feature = "zugferd")]
        #[arg(long, value_name = "COMPANY_JSON", requires = "zugferd")]
        company: Option<PathBuf>,
        /// Record the build (options, SHA-256 of the PDF and of every file read) in this
        /// manifest, e.g. archive.json; check it later with `typstgen verify`
        #[cfg(feature = "archive")]
        #[arg(long, value_name = "MANIFEST", conflicts_with = "pages")]
        manifest: Option<PathBuf>,
    },
    /// Check the PDFs recorded in a manifest: unchanged, same sources, identical rebuild
    #[cfg(feature = "archive")]
    Verify {
        /// The manifest written by `compile --manifest`
        #[arg(default_value = "archive.json")]
        manifest: PathBuf,
        /// Do not read the stored PDFs; only rebuild and compare with the recorded SHA-256
        /// (for CI, where the archived PDFs are not checked out)
        #[arg(long)]
        rebuild_only: bool,
    },
    /// Serve the Model Context Protocol on stdin/stdout for AI agents
    #[cfg(feature = "mcp")]
    Mcp {
        /// Workspace directory; tools only read and write inside it (default: current directory)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Path to typstgen.toml (defaults to ./typstgen.toml in the workspace)
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            output,
            config,
            inputs,
            pdf_standards,
            creation_timestamp,
            pages,
            no_pdf_tags,
            #[cfg(feature = "zugferd")]
            zugferd,
            #[cfg(feature = "zugferd")]
            company,
            #[cfg(feature = "archive")]
            manifest,
        } => {
            #[cfg(feature = "archive")]
            if let Some(manifest) = manifest {
                #[cfg(feature = "zugferd")]
                let zugferd = zugferd.zip(company);
                #[cfg(not(feature = "zugferd"))]
                let zugferd = None;
                let job = typstgen::archive::Job {
                    output: output.unwrap_or_else(|| input.with_extension("pdf")),
                    input,
                    // Config::load falls back to ./typstgen.toml; record that file too.
                    config: config.or_else(|| {
                        let local = PathBuf::from("typstgen.toml");
                        local.exists().then_some(local)
                    }),
                    inputs: inputs.into_iter().collect(),
                    pdf_standards,
                    creation_timestamp,
                    pdf_tags: !no_pdf_tags,
                    zugferd,
                };
                return record(&manifest, &job);
            }

            #[cfg(feature = "zugferd")]
            let zugferd_xml = match zugferd
                .zip(company)
                .map(|(invoice, company)| zugferd_xml(&invoice, &company))
            {
                Some(Ok(xml)) => Some(xml.into_bytes()),
                Some(Err(err)) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
                None => None,
            };
            let options = CompileOptions {
                inputs: inputs.into_iter().collect(),
                pdf_standards,
                creation_timestamp,
                pages,
                pdf_tags: !no_pdf_tags,
                #[cfg(feature = "zugferd")]
                zugferd_xml,
            };
            let config = match Config::load(config.as_deref()) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
            };

            match typstgen::compile_with_options(&input, &config, &options) {
                Ok(compiled) => {
                    for warning in &compiled.warnings {
                        eprintln!("{warning}");
                    }
                    let output = output.unwrap_or_else(|| input.with_extension("pdf"));
                    if let Err(err) = std::fs::write(&output, compiled.pdf) {
                        eprintln!("error: failed to write {}: {err}", output.display());
                        return ExitCode::FAILURE;
                    }
                    println!("{}", output.display());
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        #[cfg(feature = "archive")]
        Commands::Verify {
            manifest,
            rebuild_only,
        } => verify(&manifest, rebuild_only),
        #[cfg(feature = "mcp")]
        Commands::Mcp { root, config } => match serve_mcp(root, config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("error: {err}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(feature = "archive")]
fn record(manifest: &std::path::Path, job: &typstgen::archive::Job) -> ExitCode {
    let result = Config::load(job.config.as_deref())
        .and_then(|config| typstgen::archive::compile_and_record(manifest, job, &config));
    match result {
        Ok(compiled) => {
            for warning in &compiled.warnings {
                eprintln!("{warning}");
            }
            println!("{}", job.output.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "archive")]
fn verify(manifest: &std::path::Path, rebuild_only: bool) -> ExitCode {
    let reports = match typstgen::archive::verify(manifest, rebuild_only) {
        Ok(reports) => reports,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut failed = 0;
    for report in &reports {
        if report.problems.is_empty() {
            println!("ok      {}", report.document);
        } else {
            failed += 1;
            println!("FAILED  {}", report.document);
            for problem in &report.problems {
                println!("        {problem}");
            }
        }
        for note in &report.notes {
            println!("        note: {note}");
        }
    }
    println!(
        "{} of {} documents verified",
        reports.len() - failed,
        reports.len()
    );
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(feature = "mcp")]
fn serve_mcp(root: Option<PathBuf>, config: Option<PathBuf>) -> Result<(), String> {
    // Config paths are relative to the working directory, so the workspace
    // becomes the working directory before the config is read.
    let config = config
        .map(|path| {
            path.canonicalize()
                .map_err(|err| format!("{}: {err}", path.display()))
        })
        .transpose()?;
    if let Some(root) = &root {
        std::env::set_current_dir(root).map_err(|err| format!("{}: {err}", root.display()))?;
    }
    let root = std::env::current_dir()
        .and_then(|dir| dir.canonicalize())
        .map_err(|err| err.to_string())?;
    let config = Config::load(config.as_deref()).map_err(|err| err.to_string())?;
    typstgen::mcp::serve(root, config).map_err(|err| err.to_string())
}

#[cfg(feature = "zugferd")]
fn zugferd_xml(invoice: &std::path::Path, company: &std::path::Path) -> Result<String, String> {
    let read = |path: &std::path::Path| {
        std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))
    };
    typstgen::zugferd::xml_from_docgen(&read(invoice)?, &read(company)?)
        .map_err(|err| err.to_string())
}

fn parse_input(raw: &str) -> Result<(String, String), String> {
    match raw.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
        _ => Err(format!("expected KEY=VALUE, got {raw:?}")),
    }
}
