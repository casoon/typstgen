# Changelog

All notable changes to this project are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
