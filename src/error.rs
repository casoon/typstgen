//! The error type returned by every fallible typstgen function.

use std::path::PathBuf;

/// Everything that can go wrong while loading a config or compiling a document.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The input `.typ` file does not exist.
    #[error("input file not found: {0}")]
    InputNotFound(PathBuf),

    /// The config file could not be read.
    #[error("failed to read config at {path}: {source}")]
    Config {
        /// The config file that was requested.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The config file is not valid TOML for [`Config`](crate::Config).
    #[error("invalid config: {0}")]
    InvalidConfig(#[from] toml::de::Error),

    /// Typst reported errors. The message lists each as
    /// `path:line:column: error: …` with its hints, followed by any warnings.
    #[error("typst compilation failed:\n{0}")]
    Compile(String),

    /// Any other I/O error, for example while reading the input.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
