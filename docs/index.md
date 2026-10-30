---
title: Overview
description: What typstgen does, where it stops, and how this documentation is organised.
order: 0
---

typstgen compiles an existing `.typ` file to PDF. The Typst engine runs inside typstgen as a Rust
library, so no `typst` binary has to be installed and no subprocess is started. Where imported
templates live is set in a config file, `typstgen.toml`, instead of being hardcoded.

One crate, four entry points:

- **CLI**: `typstgen compile input.typ` (default `cli` feature).
- **MCP server**: `typstgen mcp` lets AI agents check, preview and compile documents
  ([AI agents](guides/ai-agents/)).
- **Rust library**: `typstgen::compile(path, &config)` returns the PDF bytes.
- **Wasm**: the `wasm` feature exports `compile`, which takes the source and its imports as a
  JSON virtual filesystem.

Around the compiler, typstgen covers what business documents need:

- **E-invoices**: embed ZUGFeRD / Factur-X data in a PDF/A-3b, or write XRechnung 3.0 and Peppol
  BIS Billing 3.0 as UBL XML, from the same invoice JSON
  ([E-invoices](guides/e-invoices/)).
- **Invoice values**: `prepare-invoice` calculates line amounts, tax groups and totals from your own
  tax configuration ([Document profiles](guides/document-profiles/)).
- **Archives**: a manifest records each build, and `typstgen verify` later proves the PDFs are
  unchanged and rebuild byte for byte ([Archiving](guides/archiving/)).

## Where it stops

| typstgen does | typstgen does not |
| --- | --- |
| Compile one `.typ` file to one PDF | Watch files or compile folders in bulk |
| Resolve imports against configured template directories and Typst packages (`@preview/…`, `@local/…`) | Publish or update packages |
| Bundle Typst's fallback fonts, optionally add system and custom fonts | Manage clients, numbering or data (its discontinued predecessor `docgen` did; typstgen replaces only its PDF generation) |
| Report Typst errors and warnings with file, line and column | Export SVG or HTML (PNG only as MCP page preview / `render_png`) |
| Pass `sys.inputs`, enforce PDF/A or PDF/UA, export page ranges | Set the PDF identifier or `/Creator` |

## How the docs are organised

- **Getting started**: [install](getting-started/installation/) the CLI or the crate and compile a
  [first document](getting-started/quickstart/).
- **Guides**, for compiling: [templates and imports](guides/templates/), [fonts](guides/fonts/),
  [inputs and PDF options](guides/inputs-and-pdf/),
  [packages and bibliographies](guides/packages-and-bibliography/) and
  [AI agents over MCP](guides/ai-agents/).
- **Guides**, for invoices and records: [e-invoices](guides/e-invoices/),
  [document profiles and invoice configuration](guides/document-profiles/),
  [archiving](guides/archiving/), [signing](guides/signing/) and [checks in CI](guides/ci/).
- **Reference**: [CLI commands](reference/cli/), [`typstgen.toml`](reference/configuration/) and the
  [Rust and Wasm API](reference/library/). Item-level documentation lives on
  [docs.rs](https://docs.rs/typstgen).

Every PDF in the [showcase](../showcase/) was compiled by typstgen from the `.typ` file shown next to
it.
