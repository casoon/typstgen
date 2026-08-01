//! Typst's [`World`] implementation for native and virtual-file compilation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use fontdb::Database;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, World};

use crate::{Config, Result};

/// A cache of font metadata and data used by Typst during one compilation.
struct FontCache {
    book: typst::utils::LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl FontCache {
    fn new(config: Option<&Config>) -> Self {
        let mut database = Database::new();

        #[cfg(target_arch = "wasm32")]
        let _ = config;

        // The bundled Typst fonts ensure every target, especially wasm, can
        // render a document without depending on host-installed fonts.
        for font in typst_assets::fonts() {
            database.load_font_data(font.to_vec());
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(config) = config {
            if config.use_system_fonts {
                database.load_system_fonts();
            }

            for path in &config.font_paths {
                if path.is_dir() {
                    database.load_fonts_dir(path);
                } else if path.is_file() {
                    let _ = database.load_font_file(path);
                }
            }
        }

        let mut book = FontBook::new();
        let mut fonts = Vec::new();
        for face in database.faces() {
            let data = match &face.source {
                fontdb::Source::File(path) => std::fs::read(path).ok(),
                fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
                fontdb::Source::SharedFile(_, data) => Some(data.as_ref().as_ref().to_vec()),
            };

            if let Some(data) = data {
                for font in Font::iter(Bytes::new(data)) {
                    book.push(font.info().clone());
                    fonts.push(font);
                }
            }
        }

        Self {
            book: typst::utils::LazyHash::new(book),
            fonts,
        }
    }
}

/// Lazy cache entry for a file in the virtual filesystem.
struct FileSlot {
    source: OnceLock<FileResult<Source>>,
    bytes: OnceLock<FileResult<Bytes>>,
}

impl FileSlot {
    fn new() -> Self {
        Self {
            source: OnceLock::new(),
            bytes: OnceLock::new(),
        }
    }
}

/// A Typst world backed by a virtual filesystem and, on native targets,
/// configured filesystem roots.
pub(crate) struct TypstWorld {
    main: Source,
    library: typst::utils::LazyHash<Library>,
    fonts: FontCache,
    files: RwLock<HashMap<FileId, FileSlot>>,
    roots: Vec<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    now: OnceLock<Option<Datetime>>,
}

impl TypstWorld {
    /// Create a native world for `input`. Template paths are roots of the
    /// virtual filesystem; imports first use the input's project root where
    /// possible, then the configured template root.
    pub(crate) fn from_file(input: &Path, config: &Config) -> Result<Self> {
        let template_root = config.resolve_template_path()?;
        let input = input.canonicalize()?;
        let source = std::fs::read_to_string(&input)?;

        let (main_path, roots) = match VirtualPath::within_root(&input, &template_root) {
            Some(path) => (path, vec![template_root]),
            None => {
                let input_root = input.parent().unwrap_or(Path::new(".")).to_path_buf();
                (
                    VirtualPath::new("main.typ"),
                    vec![input_root, template_root],
                )
            }
        };

        Ok(Self::new(
            FileId::new(None, main_path),
            source,
            roots,
            Some(config),
        ))
    }

    /// Create a filesystem-free world. `files` keys are virtual paths such as
    /// `templates/shared.typ`, and values are their UTF-8 source or asset data.
    #[cfg(feature = "wasm")]
    pub(crate) fn from_virtual(
        source: String,
        files: impl IntoIterator<Item = (PathBuf, Vec<u8>)>,
    ) -> Self {
        let world = Self::new(
            FileId::new(None, VirtualPath::new("main.typ")),
            source,
            vec![],
            None,
        );

        for (path, contents) in files {
            world.add_file(path, contents);
        }

        world
    }

    fn new(main_id: FileId, source: String, roots: Vec<PathBuf>, config: Option<&Config>) -> Self {
        Self {
            main: Source::new(main_id, source),
            library: typst::utils::LazyHash::new(Library::default()),
            fonts: FontCache::new(config),
            files: RwLock::new(HashMap::new()),
            roots,
            #[cfg(not(target_arch = "wasm32"))]
            now: OnceLock::new(),
        }
    }

    #[cfg(feature = "wasm")]
    fn add_file(&self, path: PathBuf, contents: Vec<u8>) {
        let id = FileId::new(None, VirtualPath::new(path));
        let mut files = self
            .files
            .write()
            .expect("virtual filesystem lock poisoned");
        let slot = files.entry(id).or_insert_with(FileSlot::new);
        let _ = slot.bytes.set(Ok(Bytes::new(contents)));
    }

    fn cached_or_load<T: Clone>(
        &self,
        id: FileId,
        cell: impl Fn(&FileSlot) -> &OnceLock<FileResult<T>>,
        load: impl FnOnce(&Path) -> FileResult<T>,
    ) -> FileResult<T> {
        {
            let files = self.files.read().expect("virtual filesystem lock poisoned");
            if let Some(result) = files.get(&id).and_then(|slot| cell(slot).get()) {
                return result.clone();
            }
        }

        let result = self
            .path_for(id)
            .ok_or_else(|| FileError::NotFound(id.vpath().as_rooted_path().to_path_buf()))
            .and_then(|path| load(&path));

        let mut files = self
            .files
            .write()
            .expect("virtual filesystem lock poisoned");
        let slot = files.entry(id).or_insert_with(FileSlot::new);
        cell(slot).get_or_init(|| result.clone()).clone()
    }

    fn path_for(&self, id: FileId) -> Option<PathBuf> {
        if id.package().is_some() {
            return None;
        }

        self.roots
            .iter()
            .filter_map(|root| id.vpath().resolve(root))
            .find(|path| path.is_file())
    }
}

impl World for TypstWorld {
    fn library(&self) -> &typst::utils::LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &typst::utils::LazyHash<FontBook> {
        &self.fonts.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }

        {
            let files = self.files.read().expect("virtual filesystem lock poisoned");
            if let Some(Ok(bytes)) = files.get(&id).and_then(|slot| slot.bytes.get()) {
                let text =
                    std::str::from_utf8(bytes.as_slice()).map_err(|_| FileError::InvalidUtf8)?;
                return Ok(Source::new(id, text.to_owned()));
            }
        }

        self.cached_or_load(
            id,
            |slot| &slot.source,
            |path| {
                std::fs::read_to_string(path)
                    .map_err(|error| FileError::from_io(error, path))
                    .map(|text| Source::new(id, text))
            },
        )
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.cached_or_load(
            id,
            |slot| &slot.bytes,
            |path| {
                std::fs::read(path)
                    .map_err(|error| FileError::from_io(error, path))
                    .map(Bytes::new)
            },
        )
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            *self.now.get_or_init(|| {
                let now = chrono::Local::now();
                Datetime::from_ymd_hms(
                    now.format("%Y").to_string().parse().ok()?,
                    now.format("%m").to_string().parse().ok()?,
                    now.format("%d").to_string().parse().ok()?,
                    now.format("%H").to_string().parse().ok()?,
                    now.format("%M").to_string().parse().ok()?,
                    now.format("%S").to_string().parse().ok()?,
                )
            })
        }
    }
}
