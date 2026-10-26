# Changelog

All notable changes to this project are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Profiles:** Data-driven document profile listing, selection and invoice validation are shared
  by the Rust library, CLI and MCP, with field diagnostics and separate external-validation states.

- ZUGFeRD: VAT code `Kleinunternehmer` is accepted as an alias for `Exempt` (category E), as used
  by older invoice JSON files. It still needs `exemption_reason` or `exemption_reason_code`.

### Changed

- Docs: docgen is discontinued. Its templates live on as the Typst package `business-templates`
  in [typst-business-templates](https://github.com/casoon/typst-business-templates), and help texts
  call the invoice data "business-templates invoice JSON".

### Fixed

- **ZUGFeRD:** Line amounts and tax amounts round directly to cents without a second rounding
  through four fractional digits, preserving correct results at sub-cent boundaries.

- **ZUGFeRD:** Invalid calendar dates, conflicting currencies and stated tax-group values that
  disagree with the normalized invoice are rejected before CII generation.

- **Profiles:** Malformed/conflicting optional dates and unsupported invoice discounts block the
  rendering view; seller diagnostics point to the company input fields.

### Security

- **Dependencies:** Updated rustls, rustls-webpki and webpki-roots to patched versions, Tokio to
  1.53.3 and the withdrawn yoke-derive version to 0.8.4; no RustSec vulnerabilities remain in the lockfile.

## [0.5.0] - 2026-10-13

### Breaking

- `archive::Report` has a new public field `notes`; struct literals need it.

### Added

- `verify` accepts a PDF signed after recording: when the stored PDF starts with the recorded
  bytes and the appended incremental updates carry a signature, the document passes with a note.
  Revisions appended without a signature fail. `archive::Report` has a new public field `notes`.
- Guide [Signing](https://casoon.github.io/typstgen/docs/guides/signing/): sign with pyHanko (PAdES, RFC 3161 timestamp), keep
  PDF/A and ZUGFeRD valid, and when a signature matters in Germany. The archiving guide shows how
  to timestamp `archive.json` with `openssl ts`.

## [0.4.1] - 2026-10-12

### Changed

- The Typst world (fonts, template roots, packages, `datetime.today()`) now comes from the new
  [`embedded-typst`](https://crates.io/crates/embedded-typst) crate, shared with renderreport. No
  change in behaviour.

## [0.4.0] - 2026-10-11

### Breaking

- `Config` has three new public fields; struct literals need `..Config::default()`.
- `Output` has a new public field `pages`; `CompileOptions` has `zugferd_xml` (with the `zugferd`
  feature), so struct literals need `..CompileOptions::default()`.

### Added

- `typstgen mcp`: a Model Context Protocol server over stdio with the tools `check`, `preview`,
  `compile` and `list_templates`, restricted to a workspace directory (`--root`). Errors are
  returned to the agent as `path:line:column` lines so it can fix them itself. `mcp` feature, on by
  default.
- `render_png` renders one page as PNG (`png` feature) and `Error::PageOutOfRange`.
- `Output::pages`: the number of pages of the compiled document.
- ZUGFeRD / Factur-X e-invoices (EN 16931, `zugferd` feature, on by default): `--zugferd
  invoice.json --company company.json` builds the CII XML from docgen invoice data, checks it, embeds
  it as `factur-x.xml` in a PDF/A-3 and adds the Factur-X XMP schema. Library:
  `zugferd::xml_from_docgen`, `CompileOptions::zugferd_xml`, `Error::Zugferd`. MCP: `zugferd`
  parameter of `compile`.
- Archive manifests (`archive` feature, on by default): `compile --manifest archive.json` records
  the options and the SHA-256 of the PDF, of every file Typst read and of every font file loaded;
  `typstgen verify` checks that the PDFs are unchanged and rebuild byte for byte. Library:
  `archive::{compile_and_record, verify, Job, Report}`, `Error::Archive`. `verify --rebuild-only`
  skips the stored PDFs for CI checkouts.
- Guide "Checks and CI": pinned fonts and packages, a local verify script and a tag-triggered
  GitHub Actions workflow.
- Example `examples/thesis.typ`: a seminar paper built on a local package
  (`examples/packages/local/thesis/0.1.0`) with BibTeX citations, compiled as PDF/A-2b.
- Typst packages: `#import "@preview/…"` and `@local/…` resolve from the same package data and
  cache directories as the typst CLI. Missing `@preview` packages are downloaded from Typst
  Universe (`download` feature, on by default, pure-Rust TLS). New `typstgen.toml` keys
  `package_path`, `package_cache_path` and `download_packages`. Native builds only.
- Tests for citations and `#bibliography` (BibTeX) under PDF/A-2b.

## [0.3.0] - 2026-09-24

### Added

- `compile_with_options` and `CompileOptions`: values for `sys.inputs`, PDF standards (PDF
  version, PDF/A, PDF/UA-1), a fixed creation timestamp (also used by `datetime.today()`), page
  ranges and untagged output. `PdfStandard` and `PageRange` parse the names the `typst` CLI uses.
- CLI flags `--input KEY=VALUE`, `--pdf-standard`, `--creation-timestamp` (falls back to
  `SOURCE_DATE_EPOCH`), `--pages` and `--no-pdf-tags`.
- The Wasm request accepts `inputs`, `pdf_standards`, `creation_timestamp`, `pages` and
  `pdf_tags`.

## [0.2.0] - 2026-09-24

### Breaking

- `Config::resolve_template_path` and `Error::TemplatePathNotFound` are removed; use
  `Config::template_roots`, which returns every existing template directory.
- Minimum supported Rust version is now 1.92 (required by Typst 0.15).

### Added

- `compile_with_warnings` returns the PDF together with Typst's warnings; the CLI prints them to
  stderr.
- Errors and warnings name the file, line and column.

### Changed

- Typst 0.15 (was 0.13).
- Template directories are optional: documents without imports compile without any.
- Every existing entry of `template_paths` is searched per import, in order, instead of only the
  first existing one. Entries that are not directories are skipped.
- Fonts found on disk are loaded only when a document uses them, and each face of a font
  collection is registered once.

### Fixed

- `datetime.today()` returns a date instead of a date and time, and honours `offset`.

### Security

- quick-xml 0.38 (RUSTSEC-2026-0194, RUSTSEC-2026-0195) is still pulled in through Typst's
  bibliography support; no fixed version is available yet. Do not compile untrusted documents.

## [0.1.0] - 2026-08-01

### Added

- `typstgen compile`, the Rust library and the Wasm export, with template paths from
  `typstgen.toml` and Typst's bundled fonts.
