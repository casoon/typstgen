use std::fs;
use std::process::Command;

use tempfile::tempdir;
use typstgen::{
    compile, compile_with_options, compile_with_warnings, CompileOptions, Config, Error, PageRange,
    PdfStandard,
};

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

#[cfg(feature = "wasm")]
#[test]
fn wasm_accepts_inputs_and_pdf_options() {
    let request = serde_json::json!({
        "source": "#assert.eq(sys.inputs.customer, \"ACME\")\n#assert.eq(datetime.today(), datetime(year: 2020, month: 1, day: 1))\n= Wasm",
        "inputs": { "customer": "ACME" },
        "pdf_standards": ["a-2b"],
        "creation_timestamp": 1_577_836_800,
        "pages": "1",
        "pdf_tags": false,
    });
    let pdf = typstgen::wasm::compile(&request.to_string()).expect("compile with options");

    assert!(contains(&pdf, b"pdfaid:part"));
    assert!(contains(&pdf, b"D:20200101000000Z"));
}

#[test]
fn errors_name_file_line_and_column() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "= Title\n#missing").expect("write input");

    let Err(Error::Compile(message)) = compile(&input, &config_with(vec![])) else {
        panic!("expected a compile error");
    };
    assert!(
        message.contains("document.typ:2:2: error: unknown variable: missing"),
        "{message}"
    );
}

#[test]
fn warnings_are_returned() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "#set text(font: \"No Such Font\")\nHello").expect("write input");

    let output = compile_with_warnings(&input, &config_with(vec![])).expect("compile");
    assert!(output.pdf.starts_with(b"%PDF-"));
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.contains("document.typ:1:") && warning.contains("warning:")),
        "{:?}",
        output.warnings
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn three_pages(temp: &std::path::Path) -> std::path::PathBuf {
    let input = temp.join("document.typ");
    fs::write(
        &input,
        "#set text(lang: \"en\")\nA\n#pagebreak()\nB\n#pagebreak()\nC",
    )
    .expect("write input");
    input
}

#[test]
fn inputs_are_visible_as_sys_inputs() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#assert.eq(sys.inputs, (customer: \"ACME\", number: \"42\"))\n= Invoice",
    )
    .expect("write input");

    let options = CompileOptions {
        inputs: [("customer", "ACME"), ("number", "42")]
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .into(),
        ..Default::default()
    };
    compile_with_options(&input, &config_with(vec![]), &options)
        .expect("sys.inputs assertion holds");
}

#[test]
fn creation_timestamp_sets_metadata_and_today() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#assert.eq(datetime.today(), datetime(year: 2020, month: 1, day: 1))\n= Fixed date",
    )
    .expect("write input");

    let options = CompileOptions {
        creation_timestamp: Some(1_577_836_800), // 2020-01-01T00:00:00Z
        ..Default::default()
    };
    let output = compile_with_options(&input, &config_with(vec![]), &options).expect("compile");

    assert!(contains(&output.pdf, b"D:20200101000000Z"));
    let again = compile_with_options(&input, &config_with(vec![]), &options).expect("compile");
    assert_eq!(
        output.pdf, again.pdf,
        "fixed timestamp gives identical PDFs"
    );
}

#[test]
fn pdf_standard_is_written_and_enforced() {
    let temp = tempdir().expect("create temporary project");
    let input = three_pages(temp.path());
    let config = config_with(vec![]);

    let without_date = CompileOptions {
        pdf_standards: vec![PdfStandard::A2b],
        ..Default::default()
    };
    let Err(Error::Compile(message)) = compile_with_options(&input, &config, &without_date) else {
        panic!("expected PDF/A without a date to fail");
    };
    assert!(message.contains("missing document date"), "{message}");

    let options = CompileOptions {
        creation_timestamp: Some(1_577_836_800),
        ..without_date
    };
    let output = compile_with_options(&input, &config, &options).expect("compile PDF/A-2b");
    assert!(contains(&output.pdf, b"pdfaid:part"));

    let conflicting = CompileOptions {
        pdf_standards: vec![PdfStandard::A2b, PdfStandard::A3b],
        ..Default::default()
    };
    let Err(Error::Compile(message)) = compile_with_options(&input, &config, &conflicting) else {
        panic!("expected conflicting standards to fail");
    };
    assert!(message.contains("at most one PDF/A standard"), "{message}");
}

#[test]
fn pages_limit_the_export_and_tags_can_be_turned_off() {
    let temp = tempdir().expect("create temporary project");
    let input = three_pages(temp.path());
    let config = config_with(vec![]);

    let tagged =
        compile_with_options(&input, &config, &CompileOptions::default()).expect("compile");
    assert!(contains(&tagged.pdf, b"/Count 3"));
    assert!(contains(&tagged.pdf, b"StructTreeRoot"));

    let options = CompileOptions {
        pages: vec!["1".parse().unwrap(), "3-".parse().unwrap()],
        ..Default::default()
    };
    let partial = compile_with_options(&input, &config, &options).expect("compile pages 1 and 3");
    assert!(contains(&partial.pdf, b"/Count 2"));
    assert!(!contains(&partial.pdf, b"StructTreeRoot"));

    let untagged = CompileOptions {
        pdf_tags: false,
        ..Default::default()
    };
    let output = compile_with_options(&input, &config, &untagged).expect("compile untagged");
    assert!(!contains(&output.pdf, b"StructTreeRoot"));
}

#[test]
fn parses_pdf_standards_and_page_ranges() {
    assert_eq!("a-2b".parse::<PdfStandard>(), Ok(PdfStandard::A2b));
    assert_eq!(PdfStandard::Ua1.to_string(), "ua-1");
    assert!("a-9".parse::<PdfStandard>().is_err());

    let page = |n| std::num::NonZeroUsize::new(n);
    let range = |first, last| PageRange { first, last };
    assert_eq!("5".parse(), Ok(range(page(5), page(5))));
    assert_eq!("1-3".parse(), Ok(range(page(1), page(3))));
    assert_eq!("-3".parse(), Ok(range(None, page(3))));
    assert_eq!("5-".parse(), Ok(range(page(5), None)));
    for invalid in ["", "0", "3-1", "x", "1-y"] {
        assert!(invalid.parse::<PageRange>().is_err(), "{invalid:?}");
    }
}

#[test]
fn cli_passes_inputs_and_pdf_options() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#assert.eq(sys.inputs.customer, \"ACME\")\n\
         #assert.eq(datetime.today(), datetime(year: 2020, month: 1, day: 1))\n\
         #set text(lang: \"en\")\nA\n#pagebreak()\nB",
    )
    .expect("write input");

    let output = temp.path().join("document.pdf");
    let status = Command::new(env!("CARGO_BIN_EXE_typstgen"))
        .current_dir(temp.path())
        .env_remove("SOURCE_DATE_EPOCH")
        .args(["compile", "document.typ", "--input", "customer=ACME"])
        .args(["--pdf-standard", "a-2b", "--pages", "-1"])
        .args(["--creation-timestamp", "1577836800"])
        .status()
        .expect("run typstgen CLI");

    assert!(status.success());
    let pdf = fs::read(output).expect("read generated PDF");
    assert!(contains(&pdf, b"pdfaid:part"));
    assert!(contains(&pdf, b"/Count 1"));
}

#[test]
fn cli_reads_source_date_epoch() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#assert.eq(datetime.today(), datetime(year: 2020, month: 1, day: 1))\n= Epoch",
    )
    .expect("write input");

    let status = Command::new(env!("CARGO_BIN_EXE_typstgen"))
        .current_dir(temp.path())
        .env("SOURCE_DATE_EPOCH", "1577836800")
        .args(["compile", "document.typ"])
        .status()
        .expect("run typstgen CLI");
    assert!(status.success());
}
