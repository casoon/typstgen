---
title: Overview
description: What typstgen does, where it stops, and how this documentation is organised.
order: 0
---

typstgen compiles an existing `.typ` file to PDF. The Typst engine runs inside typstgen as a Rust
library, so no `typst` binary has to be installed and no subprocess is started. Where imported
templates live is set in a config file, `typstgen.toml`, instead of being hardcoded.

One crate, three entry points:

- **CLI**: `typstgen compile input.typ` (default `cli` feature).
- **Rust library**: `typstgen::compile(path, &config)` returns the PDF bytes.
- **Wasm**: the `wasm` feature exports `compile`, which takes the source and its imports as a
  JSON virtual filesystem.

## Where it stops

| typstgen does | typstgen does not |
| --- | --- |
| Compile one `.typ` file to one PDF | Watch files or compile folders in bulk |
| Resolve imports against configured template directories | Download Typst packages (`@preview/…`) |
| Bundle Typst's fallback fonts, optionally add system and custom fonts | Manage clients, numbering or data (that was `docgen`) |
| Report Typst errors with their hints | Print Typst warnings |

## How the docs are organised

- **Getting started**: [install](getting-started/installation/) the CLI or the crate and compile a
  [first document](getting-started/quickstart/).
- **Guides**: how [templates and imports](guides/templates/) are resolved and which
  [fonts](guides/fonts/) are available.
- **Reference**: [CLI options](reference/cli/), [`typstgen.toml`](reference/configuration/) and the
  [Rust and Wasm API](reference/library/). Item-level documentation lives on
  [docs.rs](https://docs.rs/typstgen).

Every PDF in the [showcase](../showcase/) was compiled by typstgen from the `.typ` file shown next to
it.
