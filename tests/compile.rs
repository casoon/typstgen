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
