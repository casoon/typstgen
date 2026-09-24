---
title: CLI
description: The typstgen command, its one subcommand, and how it reports success and failure.
order: 1
---

The help texts below are captured from the CLI by `examples/regenerate.sh`.

## typstgen

```text
Compile .typ files to PDF with an embedded Typst engine

Usage: typstgen <COMMAND>

Commands:
  compile  Compile a single .typ file to PDF
  help     Print this message or the help of the given subcommand(s)

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
