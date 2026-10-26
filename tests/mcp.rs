//! Drives `typstgen mcp` over stdio like an MCP client.
#![cfg(feature = "mcp")]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tempfile::tempdir;

#[test]
fn mcp_server_checks_previews_and_compiles() {
    let temp = tempdir().expect("create workspace");
    let root = temp.path().join("workspace");
    fs::create_dir_all(root.join("templates")).expect("create templates");
    fs::write(
        root.join("templates/title.typ"),
        "// Title block\n#let title = [Report]",
    )
    .expect("write template");
    fs::write(
        root.join("ok.typ"),
        "#import \"title.typ\": title\n#set document(title: \"T\")\n= #title #sys.inputs.who",
    )
    .expect("write document");
    fs::write(root.join("bad.typ"), "#missing()").expect("write broken document");
    fs::write(
        root.join("typstgen.toml"),
        "template_paths = [\"templates\"]\n",
    )
    .expect("write config");

    let call = |id: u64, name: &str, arguments: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
               "params": {"name": name, "arguments": arguments}})
    };
    #[allow(unused_mut)]
    let mut messages = vec![
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "test", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        call(2, "check", json!({"path": "bad.typ"})),
        call(
            3,
            "preview",
            json!({"path": "ok.typ", "inputs": {"who": "A"}}),
        ),
        call(
            4,
            "compile",
            json!({"path": "ok.typ", "inputs": {"who": "A"},
            "pdf_standards": ["a-2b"], "creation_timestamp": 1_700_000_000}),
        ),
        call(
            5,
            "compile",
            json!({"path": "ok.typ", "output": "../outside.pdf"}),
        ),
        call(6, "list_templates", json!({})),
    ];
    #[cfg(feature = "zugferd")]
    {
        let fixtures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zugferd");
        for name in ["invoice.json", "company.json"] {
            fs::copy(fixtures.join(name), root.join(name)).expect("copy ZUGFeRD fixture");
        }
        messages.push(call(
            7,
            "compile",
            json!({"path": "ok.typ", "inputs": {"who": "A"}, "output": "invoice.pdf",
            "pdf_standards": ["a-3b"], "creation_timestamp": 1_700_000_000,
            "zugferd": {"invoice": "invoice.json", "company": "company.json"}}),
        ));
    }
    #[cfg(feature = "zugferd")]
    {
        fs::write(root.join("profiles.json"), json!({"version":"1","profiles":[
            {"id":"de-reference","version":"1","status":"released","country":"DE","tax_case":"reference",
            "languages":["en"],"format_locales":["en-GB"],"currencies":["EUR"],"layouts":["din"],"outputs":["pdf-preview"],
            "required":["facts.confirmed"],"rules":[],"sources":[]}
        ]}).to_string()).unwrap();
        messages.push(call(
            8,
            "list_document_profiles",
            json!({"catalog":"profiles.json"}),
        ));
        messages.push(call(9,"resolve_document_profile",json!({"catalog":"profiles.json","selection":{
            "profile_id":"de-reference","profile_version":"1","language":"en","format_locale":"en-GB",
            "currency":"EUR","layout":"din","output_profile":"pdf-preview"}})));
        messages.push(call(
            10,
            "validate_document",
            json!({"catalog":"profiles.json","request":{}}),
        ));
        messages.push(call(
            11,
            "list_document_profiles",
            json!({"catalog":"../outside.json"}),
        ));
    }
    let expected = messages.len() - 1;

    let mut child = Command::new(env!("CARGO_BIN_EXE_typstgen"))
        .args(["mcp", "--root"])
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start typstgen mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    for message in &messages {
        writeln!(stdin, "{message}").expect("send message");
    }

    let mut results = std::collections::HashMap::new();
    for line in BufReader::new(child.stdout.take().expect("stdout")).lines() {
        let response: Value = serde_json::from_str(&line.expect("read line")).expect("json");
        if let Some(id) = response["id"].as_u64() {
            results.insert(id, response["result"].clone());
        }
        if results.len() == expected {
            break;
        }
    }
    drop(stdin);
    child.wait().expect("server exits after stdin closes");

    let text = |id: u64| {
        results[&id]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    };
    let is_error = |id: u64| results[&id]["isError"].as_bool().unwrap_or(false);

    assert!(is_error(2));
    assert!(
        text(2).contains("bad.typ:1:2: error: unknown variable: missing"),
        "{}",
        text(2)
    );

    assert!(!is_error(3));
    assert_eq!(results[&3]["content"][0]["type"], "image");
    assert_eq!(results[&3]["content"][0]["mimeType"], "image/png");

    assert!(!is_error(4), "{}", text(4));
    assert!(text(4).starts_with("wrote ok.pdf"), "{}", text(4));
    assert!(fs::read(root.join("ok.pdf"))
        .expect("pdf")
        .starts_with(b"%PDF-"));

    assert!(is_error(5));
    assert!(text(5).contains("outside the workspace"));
    assert!(!temp.path().join("outside.pdf").exists());

    assert_eq!(text(6), "title.typ — Title block");

    #[cfg(feature = "zugferd")]
    {
        assert_eq!(
            serde_json::from_str::<Value>(&text(8)).unwrap()["profiles"][0]["country"],
            "DE"
        );
        let selected: Value = serde_json::from_str(&text(9)).unwrap();
        assert_eq!(selected["status"], "resolved");
        assert_eq!(selected["profile"]["country"], "DE");
        assert_eq!(
            serde_json::from_str::<Value>(&text(10)).unwrap()["status"],
            "blocked"
        );
        assert!(is_error(11));
        assert!(!is_error(7), "{}", text(7));
        let pdf = fs::read(root.join("invoice.pdf")).expect("ZUGFeRD pdf");
        assert!(String::from_utf8_lossy(&pdf).contains("<fx:DocumentFileName>factur-x.xml"));
    }
}
