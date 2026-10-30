---
title: CLI
description: The typstgen command, its subcommands, and how they report success and failure.
order: 1
---

The help texts below are captured from the CLI by `examples/regenerate.sh`.

## typstgen

```text
Compile .typ files to PDF with an embedded Typst engine

Usage: typstgen <COMMAND>

Commands:
  compile                   Compile a single .typ file to PDF
  list-document-profiles    List the capabilities declared by a template package catalog
  resolve-document-profile  Resolve an explicit profile selection JSON without changing its country rules
  validate-document         Check invoice data, declared facts and profile rules; does not transmit invoices
  prepare-invoice           Build invoice values from a user-owned tax and presentation configuration
  e-invoice                 Write a pure-XML e-invoice (XRechnung 3.0 or Peppol BIS Billing 3.0, UBL 2.1) from a business-templates invoice JSON; does not send it
  verify                    Check the PDFs recorded in a manifest: unchanged, same sources, identical rebuild
  mcp                       Serve the Model Context Protocol on stdin/stdout for AI agents
  help                      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## typstgen compile

```text
Compile a single .typ file to PDF

Usage: typstgen compile [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Path to the .typ file

Options:
  -o, --output <OUTPUT>
          Output PDF path (defaults to input with .pdf extension)
  -c, --config <CONFIG>
          Path to typstgen.toml (defaults to ./typstgen.toml if present)
      --input <KEY=VALUE>
          Add a string key-value pair visible through `sys.inputs`
      --pdf-standard <STANDARD>
          PDF standards to enforce, comma-separated (e.g. a-2b, ua-1, 1.7)
      --creation-timestamp <UNIX_SECONDS>
          Creation date as a Unix timestamp; also used by datetime.today() [env: SOURCE_DATE_EPOCH=]
      --pages <PAGES>
          Pages to export, comma-separated (e.g. 1-3,5,8-); implies an untagged PDF
      --no-pdf-tags
          Write an untagged PDF (smaller, but without document structure)
  -h, --help
          Print help
```

| Option | Behaviour |
| --- | --- |
| `<INPUT>` | The `.typ` file to compile. Must exist. |
| `-o`, `--output` | Where to write the PDF. The folder must already exist; typstgen does not create it. |
| `-c`, `--config` | Config file to use. Without it, `./typstgen.toml` is used if present, otherwise the [defaults](../configuration/). |
| `--input KEY=VALUE` | A string for `sys.inputs`. Repeatable. |
| `--pdf-standard` | PDF version, PDF/A or PDF/UA-1 to enforce, comma-separated. PDF/A needs a creation date. |
| `--creation-timestamp` | Unix seconds for the PDF creation date and `datetime.today()`. Falls back to `SOURCE_DATE_EPOCH`. |
| `--pages` | Pages to export, for example `1-3,5,8-`. Produces an untagged PDF. |
| `--no-pdf-tags` | Write an untagged PDF. |
| `--manifest` | Record the build in a manifest (e.g. `archive.json`) for `typstgen verify`. Not with `--pages`. See [Archiving](../../guides/archiving/). |
| `--zugferd`, `--company` | Embed a ZUGFeRD e-invoice built from a business-templates invoice JSON and company.json; needs a PDF/A-3 standard. See [E-invoices](../../guides/e-invoices/). |

The last five options are explained in [Inputs and PDF options](../../guides/inputs-and-pdf/).

## Output and exit codes

| Result | stdout | stderr | Exit code |
| --- | --- | --- | --- |
| PDF written | Path of the PDF | Typst warnings, if any | `0` |
| Any error | – | `error: …` | `1` |

Errors include a missing input file, a missing or invalid config file, a Typst compile error, and
a PDF that cannot be written. Typst errors are printed with file, line and column, followed by their
hints and any warnings:

```text
error: typst compilation failed:
missing-import.typ:2:9: error: file not found (searched at /footer.typ)
```

This exact output is the `missing-import` example in the [showcase](../../../showcase/missing-import/).
On success, warnings go to stderr in the same `path:line:column: warning: …` form.

## typstgen prepare-invoice

```text
Build invoice values from a user-owned tax and presentation configuration

Usage: typstgen prepare-invoice --config <CONFIG> <REQUEST>

Arguments:
  <REQUEST>  

Options:
      --config <CONFIG>  
  -h, --help             Print help
```

Reads an invoice request and a user-owned configuration (named VAT taxes or additive sales-tax
components, required fields, notes, presentation) and prints the canonical invoice values as JSON:
line amounts, tax groups and totals, calculated with exact decimal arithmetic. Exits with code `1` when the report's `status` is
`blocked`. Described in
[Document profiles](../../guides/document-profiles/#user-owned-invoice-configuration).

## typstgen e-invoice

```text
Write a pure-XML e-invoice (XRechnung 3.0 or Peppol BIS Billing 3.0, UBL 2.1) from a business-templates invoice JSON; does not send it

Usage: typstgen e-invoice [OPTIONS] --company <COMPANY_JSON> --format <FORMAT> <INVOICE>

Arguments:
  <INVOICE>  The business-templates invoice JSON

Options:
      --company <COMPANY_JSON>  company.json with the seller's data
      --format <FORMAT>         Target format: xrechnung-3.0 or peppol-bis-3.0
  -o, --output <OUTPUT>         Output XML path (defaults to the invoice path with .xml extension)
  -h, --help                    Print help
```

Writes the invoice as UBL 2.1 XML. Missing or invalid fields are listed by JSON path and the command
exits with code `1`. Schema and Schematron validation are reported as `not_run`; writing the file
does not send it. Described in [E-invoices](../../guides/e-invoices/#xrechnung-and-peppol-bis-billing).

## Document profiles

`list-document-profiles`, `resolve-document-profile` and `validate-document` read a profile catalog
supplied by a template package and print a JSON report; they exit with code `1` when its `status`
is `blocked`. Described in
[Document profiles](../../guides/document-profiles/).

```text
List the capabilities declared by a template package catalog

Usage: typstgen list-document-profiles [OPTIONS] --catalog <CATALOG>

Options:
      --catalog <CATALOG>  
      --include-drafts     
  -h, --help               Print help
```

```text
Resolve an explicit profile selection JSON without changing its country rules

Usage: typstgen resolve-document-profile --catalog <CATALOG> <SELECTION>

Arguments:
  <SELECTION>  

Options:
      --catalog <CATALOG>  
  -h, --help               Print help
```

```text
Check invoice data, declared facts and profile rules; does not transmit invoices

Usage: typstgen validate-document --catalog <CATALOG> <REQUEST>

Arguments:
  <REQUEST>  

Options:
      --catalog <CATALOG>  
  -h, --help               Print help
```

## typstgen mcp

```text
Serve the Model Context Protocol on stdin/stdout for AI agents

Usage: typstgen mcp [OPTIONS]

Options:
      --root <ROOT>      Workspace directory; tools only read and write inside it (default: current directory)
  -c, --config <CONFIG>  Path to typstgen.toml (defaults to ./typstgen.toml in the workspace)
  -h, --help             Print help
```

Runs until the client closes stdin. The tools are described in [AI agents](../../guides/ai-agents/).

## typstgen verify

```text
Check the PDFs recorded in a manifest: unchanged, same sources, identical rebuild

Usage: typstgen verify [OPTIONS] [MANIFEST]

Arguments:
  [MANIFEST]  The manifest written by `compile --manifest` [default: archive.json]

Options:
      --rebuild-only  Do not read the stored PDFs; only rebuild and compare with the recorded SHA-256 (for CI, where the archived PDFs are not checked out)
```

`--rebuild-only` skips reading the stored PDFs and only compares the rebuild with the recorded
SHA-256, for CI checkouts that do not contain the archived PDFs.

Prints one line per document (`ok` or `FAILED` with the reasons, then any `note:` lines) and exits
with code `1` if any document fails. A stored PDF that was signed after recording passes with a
note, see [Signing](../../guides/signing/). Described in [Archiving](../../guides/archiving/).
