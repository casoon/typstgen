//! Builds the [`EmbeddedWorld`] for a compilation from [`Config`] and
//! [`CompileOptions`].

use std::path::Path;
use std::sync::Arc;

use embedded_typst::{EmbeddedWorld, Fonts};
use typst::foundations::IntoValue;
use typst::syntax::VirtualPath;

use crate::{CompileOptions, Config, Result};

pub(crate) type TypstWorld = EmbeddedWorld;

/// Create a native world for `input`. The project root is the first template
/// directory that contains `input`, otherwise the input's own directory.
/// Imports are looked up in the project root first, then in every other
/// existing template directory.
pub(crate) fn from_file(
    input: &Path,
    config: &Config,
    options: &CompileOptions,
) -> Result<TypstWorld> {
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

    // The bundled Typst fonts ensure every document renders without
    // host-installed fonts; system and configured fonts come after them.
    #[allow(unused_mut)]
    let mut fonts = Fonts::builder().typst_fonts();
    #[cfg(not(target_arch = "wasm32"))]
    {
        if config.use_system_fonts {
            fonts = fonts.system_fonts();
        }
        for path in &config.font_paths {
            fonts = fonts.path(path);
        }
    }

    let world = new(main_path, source, Arc::new(fonts.build()), options).with_roots(roots);
    #[cfg(not(target_arch = "wasm32"))]
    let world = world.with_packages(packages(config));
    Ok(world)
}

#[cfg(not(target_arch = "wasm32"))]
fn packages(config: &Config) -> embedded_typst::Packages {
    #[allow(unused_mut)]
    let mut packages = embedded_typst::Packages::new(
        config.package_path.clone(),
        config.package_cache_path.clone(),
    );
    #[cfg(feature = "download")]
    if config.download_packages {
        packages = packages.download(concat!("typstgen/", env!("CARGO_PKG_VERSION")));
    }
    packages
}

/// Create a filesystem-free world with the bundled fonts only. `files` keys
/// are virtual paths such as `templates/shared.typ`, and values are their
/// UTF-8 source or asset data.
#[cfg(feature = "wasm")]
pub(crate) fn from_virtual(
    source: String,
    files: impl IntoIterator<Item = (String, Vec<u8>)>,
    options: &CompileOptions,
) -> std::result::Result<TypstWorld, String> {
    let main = VirtualPath::new("main.typ").expect("valid virtual path");
    let fonts = Arc::new(Fonts::builder().typst_fonts().build());
    let world = new(main, source, fonts, options);

    for (path, contents) in files {
        let vpath =
            VirtualPath::new(&path).map_err(|error| format!("invalid path {path:?}: {error}"))?;
        world.add_file(vpath, contents);
    }

    Ok(world)
}

fn new(
    main: VirtualPath,
    #[allow(unused_mut)] mut source: String,
    fonts: Arc<Fonts>,
    options: &CompileOptions,
) -> TypstWorld {
    let inputs = options
        .inputs
        .iter()
        .map(|(key, value)| (key.as_str().into(), value.as_str().into_value()))
        .collect();
    // ZUGFeRD: the main file attaches the invoice XML, served from memory.
    #[cfg(feature = "zugferd")]
    if options.zugferd_xml.is_some() {
        source.push_str(crate::zugferd::ATTACH);
    }
    let mut world = EmbeddedWorld::new(main, source, fonts).with_inputs(inputs);
    if let Some(seconds) = options.creation_timestamp {
        world = world.with_creation_timestamp(seconds);
    }
    #[cfg(feature = "zugferd")]
    if let Some(xml) = &options.zugferd_xml {
        let path = VirtualPath::new(crate::zugferd::VIRTUAL_PATH).expect("valid virtual path");
        world.add_file(path, xml.clone());
    }
    world
}
