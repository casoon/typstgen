# typstgen

Compile `.typ` files to PDF with the Typst engine built in: no `typst` binary, no subprocess, and
templates found through a config file instead of hardcoded paths. Use it from the shell, from Rust,
in the browser (Wasm) or from an AI agent (MCP).

**Website and documentation:** [casoon.github.io/typstgen](https://casoon.github.io/typstgen/)

<p align="center">
  <img src=".github/readme/demo.svg" alt="A terminal demo: typstgen compile letter.typ --config typstgen.toml prints letter.pdf, and file letter.pdf reports a one-page PDF document, version 1.7." width="696">
</p>

## What it does

- **Compiles Typst to PDF** in one call, the same way on a laptop, a server and in CI.
- **Finds your templates** through `typstgen.toml`; Typst packages (`@preview/…`, `@local/…`)
  resolve as with the typst CLI.
- **Produces archivable PDFs**: PDF/A and PDF/UA, bundled fonts and a fixed creation date, so the
  same input gives the same bytes.
- **Writes e-invoices** from one invoice JSON: ZUGFeRD / Factur-X embedded in the PDF, or
  XRechnung 3.0 and Peppol BIS Billing 3.0 as XML.
- **Calculates invoice values** (line amounts, tax groups, totals) from your own tax configuration.
- **Proves archives**: a manifest records each build, and `typstgen verify` later shows that every
  PDF is unchanged and rebuilds byte for byte.
- **Works with AI agents**: the MCP server lets an agent check, preview and compile documents and
  read Typst's errors with file, line and column.

## How it works

```
your-document.typ  +  typstgen.toml (template_paths)  →  your-document.pdf
```

typstgen runs the Typst compiler as a Rust library. An `#import` is looked up next to the document
first, then in the template directories from `typstgen.toml`. You get the PDF, or Typst's error
with its exact position and exit code 1.

## Install

```bash
cargo install typstgen                              # CLI and MCP server
cargo add typstgen --no-default-features            # Rust library only
claude mcp add typstgen -- typstgen mcp --root .    # register the MCP server in Claude Code
```

Rust 1.92 or newer. [Installation](https://casoon.github.io/typstgen/docs/getting-started/installation/)
lists every Cargo feature and the MCP setup.

## Quick start

```toml
# typstgen.toml — paths are relative to the working directory.
template_paths = ["templates"]
```

```bash
typstgen compile letter.typ          # writes letter.pdf next to it
typstgen compile invoice.typ --input customer=ACME --pdf-standard a-2b \
  --creation-timestamp "$(date +%s)"  # sys.inputs, PDF/A, fixed date
```

Without a config, `./templates` and `./.typstgen/templates` are used if they exist. Missing
`@preview` packages are downloaded once; set `download_packages = false` for offline builds.

## Invoices

The invoice template of the Typst package
[business-templates](https://github.com/casoon/typst-business-templates) renders an invoice JSON
to PDF. typstgen reads the same file for the machine-readable versions, so the PDF and the XML
cannot disagree:

```bash
# ZUGFeRD / Factur-X: the EN 16931 data embedded in a PDF/A-3b
typstgen compile invoice.typ --pdf-standard a-3b --zugferd invoice.json --company company.json

# XRechnung 3.0 or Peppol BIS Billing 3.0 as UBL 2.1 XML
typstgen e-invoice --format xrechnung-3.0 --company company.json invoice.json

# Amounts, tax groups and totals from your own VAT or sales-tax configuration
# (request.json holds company, data and optional facts)
typstgen prepare-invoice --config invoice-config.json request.json
```

Problems in the data are reported per JSON field. typstgen does not send invoices, and schema and
Schematron validation stay a separate step. Guides:
[E-invoices](https://casoon.github.io/typstgen/docs/guides/e-invoices/),
[Document profiles and invoice configuration](https://casoon.github.io/typstgen/docs/guides/document-profiles/).

## From Rust, Wasm and AI agents

```rust
use typstgen::{compile, Config};

let config = Config::load(None)?;
let pdf_bytes = compile("example.typ".as_ref(), &config)?;
std::fs::write("example.pdf", pdf_bytes)?;
```

- **Rust:** [Rust and Wasm API](https://casoon.github.io/typstgen/docs/reference/library/),
  item-level docs on [docs.rs](https://docs.rs/typstgen).
- **Wasm:** the `wasm` feature exports `compile`, which takes the source and its imports as a
  JSON virtual filesystem and returns the PDF bytes.
- **AI agents:** `typstgen mcp` serves nine tools, from `check` and `preview` to `e_invoice`;
  see [AI agents](https://casoon.github.io/typstgen/docs/guides/ai-agents/).

## Background

typstgen replaces docgen, the CLI of
[typst-business-templates](https://github.com/casoon/typst-business-templates), for PDF
generation. docgen is discontinued; its templates live on as the Typst package
`business-templates` and compile with typstgen. The Typst world (fonts, template roots,
packages) comes from [`embedded-typst`](https://github.com/casoon/embedded-typst), shared with
[`renderreport`](https://github.com/casoon/renderreport).

## License

MIT
