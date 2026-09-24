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
        } => {
            let options = CompileOptions {
                inputs: inputs.into_iter().collect(),
                pdf_standards,
                creation_timestamp,
                pages,
                pdf_tags: !no_pdf_tags,
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
    }
}

fn parse_input(raw: &str) -> Result<(String, String), String> {
    match raw.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
        _ => Err(format!("expected KEY=VALUE, got {raw:?}")),
    }
}
