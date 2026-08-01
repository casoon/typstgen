//! Config-driven template path resolution.
//!
//! typstgen never hardcodes where templates live. Resolution order:
//!   1. `--config <path>` (explicit file, CLI/lib caller)
//!   2. `./typstgen.toml` (project-local)
//!   3. [`Config::default`] fallback paths
//!
//! See docs/PLAN.md for why this exists (docgen only ever shipped the
//! project-local half of its own planned 3-tier resolution strategy).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Directories searched (in order) for `#import` package resolution.
    /// First entry that exists wins.
    pub template_paths: Vec<PathBuf>,
    /// Additional font directories (native builds only).
    pub font_paths: Vec<PathBuf>,
    /// Scan system fonts. Always forced to `false` on the wasm target,
    /// regardless of this setting — see src/world.rs.
    pub use_system_fonts: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            template_paths: vec![
                PathBuf::from("./templates"),
                PathBuf::from("./.typstgen/templates"),
            ],
            font_paths: vec![],
            use_system_fonts: true,
        }
    }
}

impl Config {
    /// Load config, following the resolution order documented on [`Config`].
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let candidate = match explicit_path {
            Some(path) => Some(path.to_path_buf()),
            None => {
                let default_path = PathBuf::from("typstgen.toml");
                default_path.exists().then_some(default_path)
            }
        };

        let Some(path) = candidate else {
            return Ok(Self::default());
        };

        let raw = std::fs::read_to_string(&path).map_err(|source| Error::Config {
            path: path.clone(),
            source,
        })?;
        Ok(toml::from_str(&raw)?)
    }

    /// First configured template directory that actually exists on disk.
    pub fn resolve_template_path(&self) -> Result<PathBuf> {
        self.template_paths
            .iter()
            .find(|path| path.exists())
            .cloned()
            .ok_or(Error::TemplatePathNotFound)
    }
}
