use std::fs;
use std::process::Command;

use tempfile::tempdir;
use typstgen::{compile, Config};

#[test]
fn compiles_a_document_with_an_import_from_the_template_root() {
    let temp = tempdir().expect("create temporary project");
    let template_root = temp.path().join("templates");
    fs::create_dir_all(template_root.join("shared")).expect("create template directory");
    fs::write(
        template_root.join("shared/title.typ"),
        "#let title = [Configured template import]",
    )
    .expect("write imported template");

    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#import \"shared/title.typ\": title\n#set text(font: \"Libertinus Serif\")\n= #title",
    )
    .expect("write input");

    let config = Config {
        template_paths: vec![template_root],
        font_paths: vec![],
        use_system_fonts: false,
    };
    let pdf = compile(&input, &config).expect("compile Typst document");

    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn cli_writes_a_valid_pdf() {
    let temp = tempdir().expect("create temporary project");
    let template_root = temp.path().join("templates");
    fs::create_dir_all(&template_root).expect("create template directory");
    fs::write(
        template_root.join("shared.typ"),
        "#let title = [CLI output]",
    )
    .expect("write imported template");

    let input = temp.path().join("document.typ");
    fs::write(&input, "#import \"shared.typ\": title\n= #title").expect("write input");
    let config = temp.path().join("typstgen.toml");
    fs::write(
        &config,
        format!("template_paths = [\"{}\"]", template_root.display()),
    )
    .expect("write config");

    let output = temp.path().join("document.pdf");
    let status = Command::new(env!("CARGO_BIN_EXE_typstgen"))
        .args(["compile", input.to_str().unwrap(), "--config"])
        .arg(&config)
        .args(["--output", output.to_str().unwrap()])
        .status()
        .expect("run typstgen CLI");

    assert!(status.success());
    assert!(fs::read(output)
        .expect("read generated PDF")
        .starts_with(b"%PDF-"));
}

fn config_with(template_paths: Vec<std::path::PathBuf>) -> Config {
    Config {
        template_paths,
        font_paths: vec![],
        use_system_fonts: false,
    }
}

#[test]
fn compiles_without_any_template_directory() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "= No templates needed").expect("write input");

    let config = config_with(vec![temp.path().join("does-not-exist")]);
    let pdf = compile(&input, &config).expect("compile without template directory");

    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn searches_every_template_directory_per_import() {
    let temp = tempdir().expect("create temporary project");
    let empty = temp.path().join("empty");
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    for dir in [&empty, &first, &second] {
        fs::create_dir_all(dir).expect("create template directory");
    }
    fs::write(first.join("a.typ"), "#let a = [A]").expect("write first template");
    fs::write(second.join("b.typ"), "#let b = [B]").expect("write second template");

    let docs = temp.path().join("docs");
    fs::create_dir_all(&docs).expect("create document directory");
    let input = docs.join("document.typ");
    fs::write(
        &input,
        "#import \"a.typ\": a\n#import \"b.typ\": b\n= #a #b",
    )
    .expect("write input");

    let config = config_with(vec![empty, first, second]);
    let pdf = compile(&input, &config).expect("compile with imports from two roots");

    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn skips_template_paths_that_are_files() {
    let temp = tempdir().expect("create temporary project");
    let not_a_dir = temp.path().join("templates.typ");
    fs::write(&not_a_dir, "").expect("write file in place of a directory");
    let templates = temp.path().join("templates");
    fs::create_dir_all(&templates).expect("create template directory");
    fs::write(templates.join("t.typ"), "#let t = [T]").expect("write template");

    let config = config_with(vec![not_a_dir, templates.clone()]);
    assert_eq!(
        config.template_roots(),
        vec![templates
            .canonicalize()
            .expect("canonicalize template directory")]
    );

    let input = temp.path().join("document.typ");
    fs::write(&input, "#import \"t.typ\": t\n= #t").expect("write input");
    assert!(compile(&input, &config).is_ok());
}

#[test]
fn loads_binary_assets_from_a_template_directory() {
    let temp = tempdir().expect("create temporary project");
    let templates = temp.path().join("templates");
    fs::create_dir_all(&templates).expect("create template directory");
    fs::write(
        templates.join("logo.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#,
    )
    .expect("write image");

    let input = temp.path().join("document.typ");
    fs::write(&input, "#image(\"/logo.svg\", width: 1cm)").expect("write input");

    let pdf = compile(&input, &config_with(vec![templates])).expect("compile with image");
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn today_returns_a_date_and_honours_the_offset() {
    let temp = tempdir().expect("create temporary project");
    let utc = chrono::Utc::now().date_naive();
    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        format!(
            "#assert.eq(datetime.today().hour(), none)\n\
             #assert.eq(datetime.today(offset: 0), datetime(year: {}, month: {}, day: {}))\n\
             = Today",
            chrono::Datelike::year(&utc),
            chrono::Datelike::month(&utc),
            chrono::Datelike::day(&utc),
        ),
    )
    .expect("write input");

    compile(&input, &config_with(vec![])).expect("today() assertions hold");
}

#[cfg(feature = "wasm")]
#[test]
fn wasm_compiles_from_a_virtual_filesystem() {
    let request = serde_json::json!({
        "source": "#import \"templates/shared.typ\": title\n= #title",
        "files": { "templates/shared.typ": "#let title = [Virtual]" },
    });
    let pdf = typstgen::wasm::compile(&request.to_string()).expect("compile virtual files");

    assert!(pdf.starts_with(b"%PDF-"));
}
