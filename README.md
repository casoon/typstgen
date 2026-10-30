# typstgen

Compile an existing `.typ` file to PDF. No external `typst` binary, no
subprocess — Typst runs embedded as a Rust library. Template resolution is
config-driven instead of hardcoded.

**Website and documentation:** [casoon.github.io/typstgen](https://casoon.github.io/typstgen/)

```
your-document.typ  +  typstgen.toml (template_paths)  →  your-document.pdf
```

<p align="center">
  <img src=".github/readme/demo.svg" alt="A terminal demo: typstgen compile letter.typ --config typstgen.toml prints letter.pdf, and file letter.pdf reports a one-page PDF document, version 1.7." width="696">
</p>

Usable four ways from one crate:

- **CLI** (`cli` feature, default): `typstgen compile input.typ`
- **Rust library** (always available): `typstgen::compile(path, &config)`
- **Wasm** (`wasm` feature): source and imports supplied as a JSON virtual filesystem
- **MCP server** (`mcp` feature, default): `typstgen mcp` lets AI agents check,
  preview and compile documents — `claude mcp add typstgen -- typstgen mcp --root .`

## Why

typstgen replaces docgen, the CLI of
[typst-business-templates](https://github.com/casoon/typst-business-templates).
docgen did PDF generation from Typst, but grew into a client/project-management
tool along the way and only ever hardcoded its template lookup paths. typstgen
is the minimal version of the same idea: one job (`.typ` in, PDF out), with the
template directory resolved from config instead of baked in.

docgen is discontinued. Its templates live on as the Typst package
`business-templates` in the same repository and compile with typstgen; client
and project management, numbering and `--encrypt` are not carried over.

The architecture follows
[`renderreport`](https://github.com/casoon/renderreport): embedded Typst,
feature-gated CLI/Wasm, embedded fonts only on Wasm. The Typst world (fonts,
template roots, packages) lives in
[`embedded-typst`](https://github.com/casoon/embedded-typst), shared with
renderreport.

## Quick start

```bash
cargo install typstgen
typstgen compile letter.typ          # writes letter.pdf next to it
typstgen compile invoice.typ --input customer=ACME --pdf-standard a-2b \
  --creation-timestamp "$(date +%s)"  # sys.inputs, PDF/A, fixed date
```

Imports are looked up next to the document, then in the template directories
from `typstgen.toml` (optional; without a config, `./templates` and
`./.typstgen/templates` are used if they exist):

```toml
# typstgen.toml — paths are relative to the working directory.
# Every existing directory is searched per import, in order.
template_paths = ["templates"]
```

Package imports (`#import "@preview/…"`, `@local/…`) resolve from the same
directories as the typst CLI; missing `@preview` packages are downloaded once
into its cache. Set `download_packages = false` for offline builds.

ZUGFeRD / Factur-X e-invoices (EN 16931): `--zugferd invoice.json --company
company.json --pdf-standard a-3b` embeds the invoice data as `factur-x.xml`,
built from the same invoice JSON the `business-templates` invoice template
renders.

Archiving: `--manifest archive.json` records each build (options, SHA-256 of
the PDF and of every file read); `typstgen verify` proves later that nothing
changed and that every PDF rebuilds byte for byte.

```rust
use typstgen::{compile, Config};

let config = Config::load(None)?;
let pdf_bytes = compile("example.typ".as_ref(), &config)?;
std::fs::write("example.pdf", pdf_bytes)?;
```

## Invoice preparation

`prepare-invoice --config invoice-config.json invoice.json` builds canonical invoice values,
shared with the Rust library and MCP. User-owned configuration selects named VAT taxes or
additive sales-tax components, required fields, notes and presentation; missing totals are
calculated with exact decimal arithmetic. VAT values can feed the CII adapter; sales-tax
invoices return PDF/component data and are excluded from CII export.

`e-invoice --format xrechnung-3.0|peppol-bis-3.0 --company company.json invoice.json` writes the
same VAT invoice as UBL 2.1 XML (also as Rust function and MCP tool). Missing fields are reported
by JSON path; schema and Schematron validation are reported as `not_run`, and writing the file is
not a transmission. See [e-invoices](docs/guides/e-invoices.mdx).

See [document profiles and invoice configuration](docs/guides/document-profiles.md) for CLI,
Rust and MCP contracts. National applicability and transmission are separate concerns.

## Wasm

Build with `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm`.
Generate JS bindings with `wasm-bindgen`, then pass the source and importable
text files as JSON; the returned bytes are a PDF:

```js
import init, { compile } from './pkg/typstgen.js';

await init();
const pdf = compile(JSON.stringify({
  source: '#import "templates/shared.typ": title\n#title',
  files: { 'templates/shared.typ': '#let title = [Hello]' },
}))
```

## License

MIT
