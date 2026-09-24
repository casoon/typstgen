use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input file not found: {0}")]
    InputNotFound(PathBuf),

    #[error("failed to read config at {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid config: {0}")]
    InvalidConfig(#[from] toml::de::Error),

    #[error("typst compilation failed:\n{0}")]
    Compile(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
