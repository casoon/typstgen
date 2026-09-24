//! Typst's [`World`] implementation for native and virtual-file compilation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration, IntoValue};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::{Library, LibraryExt, World};

use crate::{CompileOptions, Config, Result};

/// Font metadata for Typst plus lazily loaded font data. The bundled fonts
/// are static and loaded up front; fonts found on disk are only read into
/// memory when Typst actually uses them.
struct FontCache {
    book: typst::utils::LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

/// A single font face, either already loaded or located in a file on disk.
struct FontSlot {
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    index: u32,
    font: OnceLock<Option<Font>>,
}

impl FontSlot {
    fn loaded(font: Font) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            path: None,
            #[cfg(not(target_arch = "wasm32"))]
            index: 0,
            font: OnceLock::from(Some(font)),
        }
    }

    fn get(&self) -> Option<Font> {
        self.font
            .get_or_init(|| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let data = std::fs::read(self.path.as_ref()?).ok()?;
                    Font::new(Bytes::new(data), self.index)
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None
                }
            })
            .clone()
    }
}

impl FontCache {
    fn new(config: Option<&Config>) -> Self {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();

        // The bundled Typst fonts ensure every target, especially wasm, can
        // render a document without depending on host-installed fonts.
        for data in typst_assets::fonts() {
            for font in Font::iter(Bytes::new(data)) {
                book.push(font.info().clone());
                fonts.push(FontSlot::loaded(font));
            }
        }

        #[cfg(target_arch = "wasm32")]
        let _ = config;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(config) = config {
            let mut database = fontdb::Database::new();
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

            // fontdb lists every face of a collection (.ttc/.otc) separately;
            // register each file's faces exactly once, reading fontdb's
            // (memory-mapped) data instead of loading the file again.
            let mut seen = std::collections::HashSet::new();
            let faces: Vec<_> = database
                .faces()
                .filter_map(|face| match &face.source {
                    fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => {
                        seen.insert(path.clone()).then(|| (face.id, path.clone()))
                    }
                    fontdb::Source::Binary(_) => None,
                })
                .collect();

            for (id, path) in faces {
                database.with_face_data(id, |data, _| {
                    for (index, info) in typst::text::FontInfo::iter(data).enumerate() {
                        book.push(info);
                        fonts.push(FontSlot {
                            path: Some(path.clone()),
                            index: index as u32,
                            font: OnceLock::new(),
                        });
                    }
                });
            }
        }

        Self {
            book: typst::utils::LazyHash::new(book),
            fonts,
        }
    }
}

/// The id of a file in the project (not in a package).
fn project_file(path: VirtualPath) -> FileId {
    RootedPath::new(VirtualRoot::Project, path).intern()
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
    /// The creation timestamp from [`CompileOptions`], if any.
    fixed_now: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg(not(target_arch = "wasm32"))]
    now: OnceLock<chrono::DateTime<chrono::Utc>>,
}

impl TypstWorld {
    /// Create a native world for `input`. The project root is the first
    /// template directory that contains `input`, otherwise the input's own
    /// directory. Imports are looked up in the project root first, then in
    /// every other existing template directory.
    pub(crate) fn from_file(
        input: &Path,
        config: &Config,
        options: &CompileOptions,
    ) -> Result<Self> {
        let input = input.canonicalize()?;
        let source = std::fs::read_to_string(&input)?;
        let template_roots = config.template_roots();

        let project_root = template_roots
            .iter()
            .find(|root| input.starts_with(root))
            .cloned()
            .unwrap_or_else(|| input.parent().unwrap_or(Path::new("/")).to_path_buf());
        let main_path = VirtualPath::virtualize(&project_root, &input)
            .map_err(|error| std::io::Error::other(format!("{}: {error}", input.display())))?;

        let mut roots = template_roots;
        roots.retain(|root| *root != project_root);
        roots.insert(0, project_root);

        Ok(Self::new(
            project_file(main_path),
            source,
            roots,
            Some(config),
            options,
        ))
    }

    /// Create a filesystem-free world. `files` keys are virtual paths such as
    /// `templates/shared.typ`, and values are their UTF-8 source or asset data.
    #[cfg(feature = "wasm")]
    pub(crate) fn from_virtual(
        source: String,
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
        options: &CompileOptions,
    ) -> std::result::Result<Self, String> {
        let main = VirtualPath::new("main.typ").expect("valid virtual path");
        let world = Self::new(project_file(main), source, vec![], None, options);

        for (path, contents) in files {
            let vpath = VirtualPath::new(&path)
                .map_err(|error| format!("invalid path {path:?}: {error}"))?;
            world.add_file(vpath, contents);
        }

        Ok(world)
    }

    fn new(
        main_id: FileId,
        source: String,
        roots: Vec<PathBuf>,
        config: Option<&Config>,
        options: &CompileOptions,
    ) -> Self {
        let inputs = options
            .inputs
            .iter()
            .map(|(key, value)| (key.as_str().into(), value.as_str().into_value()))
            .collect();
        Self {
            main: Source::new(main_id, source),
            library: typst::utils::LazyHash::new(Library::builder().with_inputs(inputs).build()),
            fonts: FontCache::new(config),
            files: RwLock::new(HashMap::new()),
            roots,
            fixed_now: options
                .creation_timestamp
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0)),
            #[cfg(not(target_arch = "wasm32"))]
            now: OnceLock::new(),
        }
    }

    #[cfg(feature = "wasm")]
    fn add_file(&self, path: VirtualPath, contents: Vec<u8>) {
        let id = project_file(path);
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
            .ok_or_else(|| FileError::NotFound(id.vpath().get_with_slash().into()))
            .and_then(|path| load(&path));

        let mut files = self
            .files
            .write()
            .expect("virtual filesystem lock poisoned");
        let slot = files.entry(id).or_insert_with(FileSlot::new);
        cell(slot).get_or_init(|| result.clone()).clone()
    }

    /// The path shown in diagnostics: the real file, relative to the working
    /// directory where possible, otherwise the virtual path.
    pub(crate) fn display_path(&self, id: FileId) -> String {
        let Some(path) = self.path_for(id) else {
            return id.vpath().get_without_slash().to_owned();
        };
        std::env::current_dir()
            .ok()
            .and_then(|cwd| cwd.canonicalize().ok())
            .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
            .unwrap_or(path)
            .display()
            .to_string()
    }

    fn path_for(&self, id: FileId) -> Option<PathBuf> {
        if *id.root() != VirtualRoot::Project {
            return None;
        }

        self.roots
            .iter()
            .filter_map(|root| id.vpath().realize(root).ok())
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
        self.fonts.fonts.get(index)?.get()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        use chrono::Datelike;

        // A fixed creation timestamp is interpreted as UTC so the result does
        // not depend on the machine's time zone. The system clock is read
        // once per compilation, so every call agrees.
        let (now, local) = match self.fixed_now {
            Some(now) => (now, false),
            #[cfg(not(target_arch = "wasm32"))]
            None => (*self.now.get_or_init(chrono::Utc::now), true),
            #[cfg(target_arch = "wasm32")]
            None => return None,
        };
        let date = match offset {
            #[cfg(not(target_arch = "wasm32"))]
            None if local => now.with_timezone(&chrono::Local).date_naive(),
            None => now.date_naive(),
            Some(offset) => {
                (now + chrono::Duration::try_seconds(offset.seconds() as i64)?).date_naive()
            }
        };
        #[cfg(target_arch = "wasm32")]
        let _ = local;
        Datetime::from_ymd(
            date.year(),
            date.month().try_into().ok()?,
            date.day().try_into().ok()?,
        )
    }
}
