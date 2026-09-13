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
  -o, --output <OUTPUT>  Output PDF path (defaults to input with .pdf extension)
  -c, --config <CONFIG>  Path to typstgen.toml (defaults to ./typstgen.toml if present)
  -h, --help             Print help
```

| Option | Behaviour |
| --- | --- |
| `<INPUT>` | The `.typ` file to compile. Must exist. |
| `-o`, `--output` | Where to write the PDF. The folder must already exist; typstgen does not create it. |
| `-c`, `--config` | Config file to use. Without it, `./typstgen.toml` is used if present, otherwise the [defaults](../configuration/). |

## Output and exit codes

| Result | stdout | stderr | Exit code |
| --- | --- | --- | --- |
| PDF written | Path of the PDF | – | `0` |
| Any error | – | `error: …` | `1` |

Errors include a missing input file, a missing or invalid config file, no existing template path,
a Typst compile error, and a PDF that cannot be written. Typst errors are printed with their hints:

```text
error: typst compilation failed:
file not found (searched at /footer.typ)
```

This exact output is the `missing-import` example in the [showcase](../../../showcase/missing-import/).
Typst warnings are not printed.
