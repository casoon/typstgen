use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use typstgen::Config;

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
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile {
            input,
            output,
            config,
        } => {
            let config = match Config::load(config.as_deref()) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("error: {err}");
                    return ExitCode::FAILURE;
                }
            };

            match typstgen::compile(&input, &config) {
                Ok(pdf_bytes) => {
                    let output = output.unwrap_or_else(|| input.with_extension("pdf"));
                    if let Err(err) = std::fs::write(&output, pdf_bytes) {
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
