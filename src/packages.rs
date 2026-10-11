//! Package resolution for `#import "@namespace/name:version"` (native only).
//!
//! Packages are looked up in the same places as the typst CLI: the package
//! data directory (locally installed packages) first, then the package cache.
//! With the `download` feature and [`Config::download_packages`], a missing
//! `@preview` package is fetched from Typst Universe into the cache.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use typst::diag::{PackageError, PackageResult};
use typst::syntax::package::PackageSpec;
use typst_kit::packages::FsPackages;

use crate::Config;

pub(crate) struct Packages {
    data: Option<FsPackages>,
    cache: Option<FsPackages>,
    #[cfg(feature = "download")]
    universe: Option<typst_kit::packages::UniversePackages>,
    /// Resolved package roots, so each package is located (and downloaded)
    /// at most once per compilation.
    roots: Mutex<HashMap<PackageSpec, PackageResult<PathBuf>>>,
}

impl Packages {
    pub(crate) fn new(config: &Config) -> Self {
        Self {
            data: config
                .package_path
                .clone()
                .map(FsPackages::new)
                .or_else(FsPackages::system_data),
            cache: config
                .package_cache_path
                .clone()
                .map(FsPackages::new)
                .or_else(FsPackages::system_cache),
            #[cfg(feature = "download")]
            universe: config
                .download_packages
                .then(|| typst_kit::packages::UniversePackages::new(download::Http)),
            roots: Mutex::new(HashMap::new()),
        }
    }

    /// The directory holding the files of `spec`.
    pub(crate) fn root(&self, spec: &PackageSpec) -> PackageResult<PathBuf> {
        let mut roots = self.roots.lock().expect("package lock poisoned");
        roots
            .entry(spec.clone())
            .or_insert_with(|| self.obtain(spec))
            .clone()
    }

    fn obtain(&self, spec: &PackageSpec) -> PackageResult<PathBuf> {
        for packages in [&self.data, &self.cache].into_iter().flatten() {
            if let Some(root) = packages.obtain(spec) {
                return Ok(root.path().to_path_buf());
            }
        }

        #[cfg(feature = "download")]
        if let (Some(universe), Some(cache)) = (&self.universe, &self.cache) {
            if spec.namespace == typst_kit::packages::UniversePackages::NAMESPACE {
                let mut archive = universe.package(spec)?;
                cache.store(spec, |dir| {
                    archive.unpack(dir).map_err(|error| {
                        PackageError::MalformedArchive(Some(error.to_string().into()))
                    })
                })?;
                if let Some(root) = cache.obtain(spec) {
                    return Ok(root.path().to_path_buf());
                }
            }
        }

        Err(PackageError::NotFound(spec.clone()))
    }
}

#[cfg(feature = "download")]
mod download {
    use std::any::Any;
    use std::io::{self, Read};

    /// Fetches Typst Universe archives over HTTPS, honouring proxy variables.
    pub(super) struct Http;

    impl typst_kit::downloader::Downloader for Http {
        fn stream(&self, _: &dyn Any, url: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
            let agent = ureq::AgentBuilder::new()
                .user_agent(concat!("typstgen/", env!("CARGO_PKG_VERSION")))
                .try_proxy_from_env(true)
                .build();
            let response = agent.get(url).call().map_err(|error| match error {
                ureq::Error::Status(404, _) => io::Error::new(io::ErrorKind::NotFound, error),
                error => io::Error::other(error),
            })?;
            let length = response
                .header("Content-Length")
                .and_then(|value| value.parse().ok());
            Ok((length, Box::new(response.into_reader())))
        }
    }
}
