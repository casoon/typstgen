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
        ..Config::default()
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
        ..Config::default()
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

/// A config that only sees the packages in `dir` and never downloads.
fn offline_packages(dir: &std::path::Path) -> Config {
    Config {
        package_path: Some(dir.join("data")),
        package_cache_path: Some(dir.join("cache")),
        download_packages: false,
        ..config_with(vec![])
    }
}

#[test]
fn resolves_packages_from_the_package_directories() {
    let temp = tempdir().expect("create temporary project");
    for (dir, namespace, text) in [("data", "local", "Local"), ("cache", "preview", "Cached")] {
        let package = temp.path().join(format!("{dir}/{namespace}/greet/0.1.0"));
        fs::create_dir_all(package.join("src")).expect("create package directory");
        fs::write(
            package.join("typst.toml"),
            "[package]\nname = \"greet\"\nversion = \"0.1.0\"\nentrypoint = \"src/lib.typ\"\n",
        )
        .expect("write package manifest");
        fs::write(
            package.join("src/lib.typ"),
            format!("#import \"text.typ\": word\n#let greet() = [{text} #word]"),
        )
        .expect("write package entrypoint");
        fs::write(package.join("src/text.typ"), "#let word = [package]")
            .expect("write package module");
    }

    let input = temp.path().join("document.typ");
    fs::write(
        &input,
        "#import \"@local/greet:0.1.0\" as local\n#import \"@preview/greet:0.1.0\" as cached\n#local.greet() #cached.greet()",
    )
    .expect("write input");

    let pdf = compile(&input, &offline_packages(temp.path())).expect("compile with packages");
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn reports_a_missing_package_without_downloading() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "#import \"@preview/does-not-exist:0.1.0\": *").expect("write input");

    let error = compile(&input, &offline_packages(temp.path())).expect_err("package is missing");
    let Error::Compile(message) = error else {
        panic!("expected a compile error, got {error:?}");
    };
    assert!(message.contains("package not found"), "{message}");
}

#[test]
fn compiles_citations_and_a_bibliography() {
    let temp = tempdir().expect("create temporary project");
    fs::write(
        temp.path().join("refs.bib"),
        "@book{knuth1984, author = {Donald E. Knuth}, title = {The TeXbook}, publisher = {Addison-Wesley}, year = {1984}}",
    )
    .expect("write bibliography");
    let input = temp.path().join("paper.typ");
    fs::write(
        &input,
        "#set document(date: datetime(year: 2026, month: 1, day: 1))\nSee @knuth1984.\n#bibliography(\"refs.bib\", style: \"ieee\")",
    )
    .expect("write input");

    let options = CompileOptions {
        pdf_standards: vec![PdfStandard::A2b],
        ..CompileOptions::default()
    };
    let output = compile_with_options(&input, &config_with(vec![]), &options)
        .expect("compile citations as PDF/A-2b");
    assert!(output.pdf.starts_with(b"%PDF-"));
    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
}

#[test]
fn output_reports_the_page_count() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "One\n#pagebreak()\nTwo\n#pagebreak()\nThree").expect("write input");

    let output = compile_with_warnings(&input, &config_with(vec![])).expect("compile");
    assert_eq!(output.pages, 3);
}

#[cfg(feature = "png")]
#[test]
fn renders_a_page_as_png() {
    let temp = tempdir().expect("create temporary project");
    let input = temp.path().join("document.typ");
    fs::write(&input, "#set page(width: 100pt, height: 50pt)\nHi").expect("write input");
    let config = config_with(vec![]);
    let options = CompileOptions::default();

    let png = typstgen::render_png(&input, &config, &options, 1, 2.0).expect("render page 1");
    assert!(png.starts_with(b"\x89PNG"));
    // 100pt x 50pt at 2 px/pt.
    assert_eq!(&png[16..24], &[0, 0, 0, 200, 0, 0, 0, 100]);

    let error = typstgen::render_png(&input, &config, &options, 2, 2.0).expect_err("no page 2");
    assert!(
        matches!(error, Error::PageOutOfRange { page: 2, pages: 1 }),
        "{error:?}"
    );
}

#[test]
fn example_thesis_compiles_as_pdf_a() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let config = Config {
        template_paths: vec![examples.join("templates")],
        package_path: Some(examples.join("packages")),
        download_packages: false,
        ..config_with(vec![])
    };
    let options = CompileOptions {
        pdf_standards: vec![PdfStandard::A2b],
        creation_timestamp: Some(1_759_276_800),
        ..CompileOptions::default()
    };

    let output = compile_with_options(&examples.join("thesis.typ"), &config, &options)
        .expect("compile the example thesis");
    assert_eq!(output.pages, 7);
    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
}

#[cfg(feature = "zugferd")]
mod zugferd {
    use super::*;
    use typstgen::zugferd::UblFormat;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/zugferd")
            .join(name);
        fs::read_to_string(path).expect("read fixture")
    }

    #[test]
    fn writes_cii_for_the_en16931_profile() {
        let xml =
            typstgen::zugferd::xml_from_docgen(&fixture("invoice.json"), &fixture("company.json"))
                .expect("valid invoice");
        for expected in [
            "<ram:ID>urn:cen.eu:en16931:2017</ram:ID>",
            "<ram:BilledQuantity unitCode=\"DAY\">2.5</ram:BilledQuantity>",
            "<ram:BilledQuantity unitCode=\"H87\">1</ram:BilledQuantity>",
            "<ram:ID schemeID=\"VA\">DE123456789</ram:ID>",
            "<ram:IBANID>DE89370400440532013000</ram:IBANID>",
            "<ram:CalculatedAmount>456.00</ram:CalculatedAmount>",
            "<ram:CalculatedAmount>2.79</ram:CalculatedAmount>",
            "<udt:DateTimeString format=\"102\">20260930</udt:DateTimeString>",
            "<ram:GrandTotalAmount>2898.69</ram:GrandTotalAmount>",
        ] {
            assert!(xml.contains(expected), "missing {expected} in\n{xml}");
        }
        // Billing period from "09/2026".
        assert!(xml.contains("20260901") && xml.contains("20260930"));
    }

    #[test]
    fn reports_every_problem_in_the_data() {
        let invoice = fixture("invoice.json")
            .replace("\"unit\": \"Stk\"", "\"unit\": \"Kisten\"")
            .replace("\"amount\": \"2898.69\"", "\"amount\": \"3000\"");
        let company = fixture("company.json").replace("\"vat_id\": \"DE123456789\",", "");
        let Err(Error::Zugferd(message)) = typstgen::zugferd::xml_from_docgen(&invoice, &company)
        else {
            panic!("expected ZUGFeRD problems");
        };
        assert!(message.contains("vat_id or company.tax_id"), "{message}");
        assert!(message.contains("unit \"Kisten\""), "{message}");
        assert!(message.contains("totals.total is 3000.00"), "{message}");
    }

    #[test]
    fn maps_kleinunternehmer_to_exempt() {
        let exempt = "{ \"code\": \"Kleinunternehmer\", \"percentage\": \"0\" }";
        let invoice = fixture("invoice.json")
            .replace("{ \"code\": \"Standard\", \"percentage\": \"19\" }", exempt)
            .replace("{ \"code\": \"Reduced\", \"percentage\": \"7\" }", exempt)
            .replace("\"amount\": \"458.79\"", "\"amount\": \"0\"")
            .replace("\"amount\": \"2898.69\"", "\"amount\": \"2439.90\"");
        let Err(Error::Zugferd(message)) =
            typstgen::zugferd::xml_from_docgen(&invoice, &fixture("company.json"))
        else {
            panic!("expected a missing exemption reason");
        };
        assert!(message.contains("exempt VAT needs"), "{message}");

        let invoice = invoice.replace(
            "\"Kleinunternehmer\",",
            "\"Kleinunternehmer\", \"exemption_reason\": \"§ 19 UStG\",",
        );
        let xml = typstgen::zugferd::xml_from_docgen(&invoice, &fixture("company.json"))
            .expect("valid exempt invoice");
        assert!(
            xml.contains("<ram:CategoryCode>E</ram:CategoryCode>"),
            "{xml}"
        );
        assert!(xml.contains("§ 19 UStG"), "{xml}");
    }

    fn ubl(format: UblFormat, invoice: &str, company: &str) -> Result<String, String> {
        typstgen::zugferd::ubl_xml_from_docgen(format, invoice, company)
            .map_err(|error| error.to_string())
    }

    /// The text of every `<tag …>text</tag>` in `xml`, in document order.
    fn values(xml: &str, tag: &str) -> Vec<String> {
        xml.match_indices(&format!("<{tag}"))
            .filter_map(|(start, _)| {
                let rest = &xml[start + tag.len() + 1..];
                let open_end = rest.find('>')?;
                (rest.starts_with('>') || rest.starts_with(' ')).then_some(())?;
                let text = &rest[open_end + 1..];
                Some(text[..text.find("</")?].to_owned())
            })
            .collect()
    }

    #[test]
    fn writes_xrechnung_ubl_in_schema_order() {
        let xml = ubl(
            UblFormat::XRechnung30,
            &fixture("invoice.json"),
            &fixture("company.json"),
        )
        .expect("valid XRechnung");
        for expected in [
            "<cbc:CustomizationID>urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0</cbc:CustomizationID>",
            "<cbc:ProfileID>urn:fdc:peppol.eu:2017:poacc:billing:01:1.0</cbc:ProfileID>",
            "<cbc:DueDate>2026-10-15</cbc:DueDate>",
            "<cbc:BuyerReference>991-01234-56</cbc:BuyerReference>",
            "<cbc:EndpointID schemeID=\"9930\">DE123456789</cbc:EndpointID>",
            "<cbc:EndpointID schemeID=\"0204\">991-01234-56</cbc:EndpointID>",
            "<cbc:Telephone>+49 30 1234567</cbc:Telephone>",
            "<cbc:PaymentMeansCode>58</cbc:PaymentMeansCode>",
            "<cbc:InvoicedQuantity unitCode=\"DAY\">2.5</cbc:InvoicedQuantity>",
            "<cbc:PayableAmount currencyID=\"EUR\">2898.69</cbc:PayableAmount>",
        ] {
            assert!(xml.contains(expected), "missing {expected} in\n{xml}");
        }
        let order = [
            "<cbc:IssueDate>",
            "<cbc:DueDate>",
            "<cbc:InvoiceTypeCode>",
            "<cbc:DocumentCurrencyCode>",
            "<cbc:BuyerReference>",
            "<cac:InvoicePeriod>",
            "<cac:AccountingSupplierParty>",
            "<cac:AccountingCustomerParty>",
            "<cac:Delivery>",
            "<cac:PaymentMeans>",
            "<cac:PaymentTerms>",
            "<cac:TaxTotal>",
            "<cac:LegalMonetaryTotal>",
            "<cac:InvoiceLine>",
        ]
        .map(|tag| xml.find(tag).unwrap_or_else(|| panic!("missing {tag}")));
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{xml}");
        let line = &xml[xml.find("<cac:InvoiceLine>").unwrap()..];
        assert!(line.find("<cbc:Description>") < line.find("<cbc:Name>"));
    }

    #[test]
    fn ubl_and_cii_carry_the_same_invoice() {
        let (invoice, company) = (fixture("invoice.json"), fixture("company.json"));
        let cii = typstgen::zugferd::xml_from_docgen(&invoice, &company).expect("valid CII");
        for format in UblFormat::ALL {
            let ubl = ubl(format, &invoice, &company).expect("valid UBL");
            let pairs = [
                ("ram:InvoiceCurrencyCode", "cbc:DocumentCurrencyCode"),
                ("ram:TaxBasisTotalAmount", "cbc:TaxExclusiveAmount"),
                ("ram:TaxTotalAmount", "cbc:TaxAmount"),
                ("ram:GrandTotalAmount", "cbc:TaxInclusiveAmount"),
                ("ram:DuePayableAmount", "cbc:PayableAmount"),
                ("ram:IBANID", "cbc:ID"),
            ];
            for (cii_tag, ubl_tag) in pairs {
                let expected = &values(&cii, cii_tag)[0];
                assert!(
                    values(&ubl, ubl_tag).contains(expected),
                    "{cii_tag} {expected} missing as {ubl_tag} in {}",
                    format.id()
                );
            }
            // Tax groups: same categories, rates, bases and amounts.
            let header = &cii[cii.find("<ram:ApplicableHeaderTradeSettlement>").unwrap()..];
            let total =
                &ubl[ubl.find("<cac:TaxTotal>").unwrap()..ubl.find("</cac:TaxTotal>").unwrap()];
            assert_eq!(
                values(header, "ram:CategoryCode"),
                values(total, "cbc:ID")
                    .into_iter()
                    .filter(|id| id != "VAT")
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                values(header, "ram:RateApplicablePercent"),
                values(total, "cbc:Percent")
            );
            assert_eq!(
                values(header, "ram:BasisAmount"),
                values(total, "cbc:TaxableAmount")
            );
            assert_eq!(
                values(header, "ram:CalculatedAmount"),
                values(total, "cbc:TaxAmount")[1..]
            );
            // Parties and dates.
            for name in ["Northwind Studio", "Example Ltd", "DE123456789"] {
                assert!(cii.contains(name) && ubl.contains(name), "{name}");
            }
            for (cii_date, ubl_date) in [
                ("20261001", "2026-10-01"),
                ("20261015", "2026-10-15"),
                ("20260930", "2026-09-30"),
            ] {
                assert!(
                    cii.contains(cii_date) && ubl.contains(ubl_date),
                    "{ubl_date}"
                );
            }
        }
    }

    #[test]
    fn ubl_formats_report_missing_fields_with_paths() {
        let invoice = fixture("invoice.json")
            .replace("\"buyer_reference\": \"991-01234-56\",", "")
            .replace(
                "\"electronic_address\": { \"scheme_id\": \"0204\", \"value\": \"991-01234-56\" }",
                "\"electronic_address\": { \"scheme_id\": \"Leitweg\", \"value\": \"991-01234-56\" }",
            );
        let company = fixture("company.json")
            .replace("\"phone\": \"+49 30 1234567\",", "")
            .replace(
                "\"electronic_address\": { \"scheme_id\": \"9930\", \"value\": \"DE123456789\" },",
                "",
            );
        let message = ubl(UblFormat::XRechnung30, &invoice, &company).expect_err("invalid");
        // The EN 16931 model rejects the malformed scheme before adapter checks run.
        assert!(
            message.contains("recipient.electronic_address.scheme_id: expected an EAS code"),
            "{message}"
        );

        let invoice = invoice.replace("\"Leitweg\"", "\"0204\"");
        let message = ubl(UblFormat::XRechnung30, &invoice, &company).expect_err("invalid");
        for path in [
            "company.electronic_address",
            "metadata.buyer_reference",
            "company.contact.phone",
        ] {
            assert!(
                message.lines().any(|line| line.starts_with(path)),
                "{path}: {message}"
            );
        }
        assert!(
            !message.contains("recipient.electronic_address"),
            "{message}"
        );

        // Peppol accepts an order reference instead of a buyer reference and
        // needs no seller telephone, but every endpoint.
        let message = ubl(UblFormat::PeppolBis30, &invoice, &company).expect_err("invalid");
        assert!(
            message.contains("metadata.buyer_reference or metadata.order_reference"),
            "{message}"
        );
        assert!(!message.contains("company.contact.phone"), "{message}");
        let invoice = invoice.replace(
            "\"customer_number\"",
            "\"order_reference\": \"PO-7\", \"customer_number\"",
        );
        let company = fixture("company.json").replace("\"phone\": \"+49 30 1234567\",", "");
        let xml = ubl(UblFormat::PeppolBis30, &invoice, &company).expect("valid Peppol");
        assert!(
            xml.contains("<cac:OrderReference>\n    <cbc:ID>PO-7</cbc:ID>"),
            "{xml}"
        );
        assert!(xml.contains("billing:3.0</cbc:CustomizationID>"), "{xml}");

        let error = "xrechnung-2.3"
            .parse::<UblFormat>()
            .expect_err("unknown format")
            .to_string();
        assert!(error.contains("xrechnung-3.0, peppol-bis-3.0"), "{error}");
    }

    #[test]
    fn intra_community_and_export_use_their_own_categories() {
        let supply = |code: &str| {
            fixture("invoice.json")
                .replace(
                    "{ \"code\": \"Standard\", \"percentage\": \"19\" }",
                    &format!("{{ \"code\": \"{code}\", \"percentage\": \"0\" }}"),
                )
                .replace(
                    "{ \"code\": \"Reduced\", \"percentage\": \"7\" }",
                    &format!("{{ \"code\": \"{code}\", \"percentage\": \"0\" }}"),
                )
                .replace(
                    "\"city\": \"Hamburg\"",
                    "\"city\": \"Wien\", \"country_code\": \"AT\"",
                )
                .replace(
                    "\"company\": \"Example Ltd\",",
                    "\"company\": \"Example Ltd\", \"vat_id\": \"ATU12345678\",",
                )
                .replace(
                    "\"performance_period\": \"09/2026\"",
                    "\"performance_period\": \"09/2026\", \"delivery_country_code\": \"AT\", \"delivery_city\": \"Wien\", \"delivery_postal_code\": \"1010\"",
                )
                .replace("\"amount\": \"458.79\"", "\"amount\": \"0\"")
                .replace("\"amount\": \"2898.69\"", "\"amount\": \"2439.90\"")
        };
        let company = fixture("company.json");
        for (code, category, exemption) in [
            ("IntraCommunity", "K", "VATEX-EU-IC"),
            ("Export", "G", "VATEX-EU-G"),
        ] {
            let invoice = supply(code);
            for format in UblFormat::ALL {
                let xml = ubl(format, &invoice, &company).expect("valid supply");
                assert!(
                    xml.contains(&format!("<cbc:ID>{category}</cbc:ID>")),
                    "{xml}"
                );
                assert!(
                    xml.contains(&format!(
                        "<cbc:TaxExemptionReasonCode>{exemption}</cbc:TaxExemptionReasonCode>"
                    )),
                    "{xml}"
                );
                assert!(
                    xml.contains("<cbc:IdentificationCode>AT</cbc:IdentificationCode>"),
                    "{xml}"
                );
                let delivery = xml.find("<cac:Delivery>").unwrap();
                assert!(delivery < xml.find("<cac:DeliveryLocation>").unwrap());
            }
            let cii = typstgen::zugferd::xml_from_docgen(&invoice, &company).expect("valid supply");
            assert!(cii.contains("<ram:CityName>Wien</ram:CityName>"), "{cii}");
            assert!(
                cii.contains("<ram:PostcodeCode>1010</ram:PostcodeCode>"),
                "{cii}"
            );
            assert!(cii.contains(&format!("<ram:CategoryCode>{category}</ram:CategoryCode>")));
            assert!(cii.contains(&format!(
                "<ram:ExemptionReasonCode>{exemption}</ram:ExemptionReasonCode>"
            )));
            let ship_to = cii.find("<ram:ShipToTradeParty>").unwrap();
            assert!(cii.contains("<ram:CountryID>AT</ram:CountryID>"));
            assert!(ship_to < cii.find("<ram:ApplicableHeaderTradeSettlement>").unwrap());
        }
        // XRechnung (BR-DE-10/11) wants city and post code once a delivery country is given.
        let no_city = supply("Export").replace(", \"delivery_city\": \"Wien\"", "");
        let error = ubl(UblFormat::XRechnung30, &no_city, &company).unwrap_err();
        assert!(error.contains("metadata.delivery_city"), "{error}");
        assert!(ubl(UblFormat::PeppolBis30, &no_city, &company).is_ok());
        // BR-IC-02 / BR-IC-12: the buyer's VAT ID and the delivery country are required.
        let missing_country =
            supply("IntraCommunity").replace(", \"delivery_country_code\": \"AT\"", "");
        let error = typstgen::zugferd::xml_from_docgen(&missing_country, &company)
            .unwrap_err()
            .to_string();
        assert!(error.contains("metadata.delivery_country_code"), "{error}");
        let missing_vat_id = supply("IntraCommunity").replace("\"vat_id\": \"ATU12345678\",", "");
        let error = typstgen::zugferd::xml_from_docgen(&missing_vat_id, &company)
            .unwrap_err()
            .to_string();
        assert!(error.contains("recipient.vat_id"), "{error}");
        // An export needs neither.
        assert!(typstgen::zugferd::xml_from_docgen(
            &supply("Export").replace("\"vat_id\": \"ATU12345678\",", ""),
            &company
        )
        .is_ok());
    }

    #[test]
    fn ubl_formats_handle_zero_rated_and_mixed_tax() {
        let reverse = "{ \"code\": \"ReverseCharge\", \"percentage\": \"0\" }";
        let exempt = "{ \"code\": \"Exempt\", \"percentage\": \"0\", \"exemption_reason\": \"§ 4 Nr. 21 UStG\" }";
        let invoice = fixture("invoice.json")
            .replace("{ \"code\": \"Reduced\", \"percentage\": \"7\" }", exempt)
            .replace("\"amount\": \"458.79\"", "\"amount\": \"456.00\"")
            .replace("\"amount\": \"2898.69\"", "\"amount\": \"2895.90\"");
        let reverse_charge = fixture("invoice.json")
            .replace(
                "{ \"code\": \"Standard\", \"percentage\": \"19\" }",
                reverse,
            )
            .replace("{ \"code\": \"Reduced\", \"percentage\": \"7\" }", reverse)
            .replace(
                "\"city\": \"Hamburg\"",
                "\"city\": \"Wien\", \"country_code\": \"AT\"",
            )
            .replace(
                "\"company\": \"Example Ltd\",",
                "\"company\": \"Example Ltd\", \"vat_id\": \"ATU12345678\",",
            )
            .replace("\"amount\": \"458.79\"", "\"amount\": \"0\"")
            .replace("\"amount\": \"2898.69\"", "\"amount\": \"2439.90\"");
        for format in UblFormat::ALL {
            let xml = ubl(format, &invoice, &fixture("company.json")).expect("mixed S and E");
            let total =
                &xml[xml.find("<cac:TaxTotal>").unwrap()..xml.find("</cac:TaxTotal>").unwrap()];
            assert_eq!(values(total, "cbc:Percent"), ["0", "19"], "{xml}");
            assert!(
                total.contains("<cbc:TaxExemptionReason>§ 4 Nr. 21 UStG</cbc:TaxExemptionReason>")
            );
            assert!(
                xml.contains("<cbc:TaxAmount currencyID=\"EUR\">0.00</cbc:TaxAmount>"),
                "{xml}"
            );

            let xml =
                ubl(format, &reverse_charge, &fixture("company.json")).expect("reverse charge");
            assert!(
                xml.contains(
                    "<cbc:TaxExemptionReasonCode>VATEX-EU-AE</cbc:TaxExemptionReasonCode>"
                ),
                "{xml}"
            );
            assert!(
                xml.contains("<cbc:CompanyID>ATU12345678</cbc:CompanyID>"),
                "{xml}"
            );
            // Exemption reasons belong to the document-level tax breakdown only.
            let lines = &xml[xml.find("<cac:InvoiceLine>").unwrap()..];
            assert!(!lines.contains("TaxExemptionReason"), "{xml}");
            assert!(
                xml.contains("<cbc:PayableAmount currencyID=\"EUR\">2439.90</cbc:PayableAmount>")
            );
        }
    }

    #[test]
    fn embeds_the_invoice_into_a_pdf_a3() {
        let temp = tempdir().expect("create temporary project");
        let input = temp.path().join("invoice.typ");
        fs::write(
            &input,
            "#set document(title: \"Rechnung\")\n= Rechnung RE-2026-001",
        )
        .expect("write input");
        let xml =
            typstgen::zugferd::xml_from_docgen(&fixture("invoice.json"), &fixture("company.json"))
                .expect("valid invoice");
        let options = CompileOptions {
            pdf_standards: vec![PdfStandard::A3b],
            creation_timestamp: Some(1_790_000_000),
            zugferd_xml: Some(xml.into_bytes()),
            ..CompileOptions::default()
        };
        let pdf = compile_with_options(&input, &config_with(vec![]), &options)
            .expect("compile ZUGFeRD invoice")
            .pdf;
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/F(factur-x.xml)"), "attachment name");
        assert!(text.contains("/AFRelationship/Alternative"));
        assert!(text.contains("<fx:ConformanceLevel>EN 16931</fx:ConformanceLevel>"));
        assert!(text.contains("Factur-X PDFA Extension Schema"));
        // The update points back to Typst's own cross-reference table.
        assert!(text.trim_end().ends_with("%%EOF") && text.matches("/Prev ").count() == 1);

        let plain = CompileOptions {
            pdf_standards: vec![PdfStandard::A2b],
            ..options
        };
        let error =
            compile_with_options(&input, &config_with(vec![]), &plain).expect_err("needs PDF/A-3");
        assert!(error.to_string().contains("PDF/A-3"), "{error}");
    }
}

#[cfg(feature = "archive")]
mod archive {
    use super::*;
    use typstgen::archive::{compile_and_record, verify, Job};

    fn job(dir: &std::path::Path) -> Job {
        Job {
            input: dir.join("letter.typ"),
            output: dir.join("letter.pdf"),
            config: None,
            inputs: [("to".to_owned(), "Ada".to_owned())].into(),
            pdf_standards: vec![PdfStandard::A2b],
            creation_timestamp: Some(1_790_000_000),
            pdf_tags: true,
            zugferd: None,
        }
    }

    #[test]
    fn records_and_verifies_a_build() {
        let temp = tempdir().expect("create temporary project");
        let dir = temp.path();
        fs::write(dir.join("part.typ"), "#let greeting = [Hello]").expect("write import");
        fs::write(
            dir.join("letter.typ"),
            "#import \"part.typ\": greeting\n#greeting #sys.inputs.to",
        )
        .expect("write input");
        let manifest = dir.join("archive.json");
        let config = config_with(vec![]);

        compile_and_record(&manifest, &job(dir), &config).expect("record build");
        let recorded: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest).expect("read manifest"))
                .expect("manifest is JSON");
        let entry = &recorded["documents"][dir
            .join("letter.pdf")
            .canonicalize()
            .unwrap()
            .display()
            .to_string()];
        assert_eq!(
            entry["files"].as_object().map(|files| files.len()),
            Some(2),
            "{recorded}"
        );
        assert_eq!(entry["inputs"]["to"], "Ada");

        let reports = verify(&manifest, false).expect("verify");
        assert!(
            reports.iter().all(|report| report.problems.is_empty()),
            "{reports:?}"
        );

        // A changed import and a changed PDF are both reported.
        fs::write(dir.join("part.typ"), "#let greeting = [Hi]").expect("change import");
        fs::write(dir.join("letter.pdf"), b"%PDF-tampered").expect("change pdf");
        let problems = &verify(&manifest, false).expect("verify")[0].problems;
        assert!(
            problems
                .iter()
                .any(|p| p.starts_with("changed:") && p.ends_with("part.typ")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p == "the PDF differs from the recorded one"),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p == "the rebuild differs from the recorded PDF"),
            "{problems:?}"
        );
    }

    #[test]
    fn accepts_a_signature_appended_after_recording() {
        let temp = tempdir().expect("create temporary project");
        let dir = temp.path();
        fs::write(dir.join("letter.typ"), "Hello #sys.inputs.to").expect("write input");
        let manifest = dir.join("archive.json");
        compile_and_record(&manifest, &job(dir), &config_with(vec![])).expect("record build");
        let recorded = fs::read(dir.join("letter.pdf")).expect("read pdf");

        // What a signer appends: an incremental update with a signature.
        let mut signed = recorded.clone();
        signed.extend_from_slice(
            b"\n9 0 obj\n<< /Type /Sig /ByteRange [0 10 20 30] /Contents <00> >>\nendobj\n\
              trailer\n<< /Size 10 /Prev 0 >>\nstartxref\n0\n%%EOF\n",
        );
        fs::write(dir.join("letter.pdf"), &signed).expect("write signed pdf");
        let report = &verify(&manifest, false).expect("verify")[0];
        assert!(report.problems.is_empty(), "{report:?}");
        assert!(
            report.notes.iter().any(|n| n.starts_with(
                "signed after recording: the recorded PDF is unchanged, 1 revision(s)"
            )),
            "{report:?}"
        );

        // An appended revision without a signature changes the document.
        let mut edited = recorded;
        edited.extend_from_slice(b"\n9 0 obj\n(changed)\nendobj\n%%EOF\n");
        fs::write(dir.join("letter.pdf"), &edited).expect("write edited pdf");
        let report = &verify(&manifest, false).expect("verify")[0];
        assert!(
            report
                .problems
                .iter()
                .any(|p| p.starts_with("changed after recording: 1 revision(s) without")),
            "{report:?}"
        );
    }
}
