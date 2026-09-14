---
title: typstgen.toml
description: The config file that sets template directories and fonts, where typstgen looks for it, and its defaults.
order: 2
---

## Lookup

1. The file passed with `--config` (CLI) or `Config::load(Some(path))` (library).
2. Otherwise `./typstgen.toml` in the working directory, if it exists.
3. Otherwise the defaults below.

## Keys

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `template_paths` | array of paths | `["./templates", "./.typstgen/templates"]` | Template directories in order of preference. The first one that exists is used for imports. |
| `font_paths` | array of paths | `[]` | Extra font directories or font files. Native builds only. |
| `use_system_fonts` | boolean | `true` | Add the fonts installed on the machine. Native builds only. |

```toml
template_paths = ["templates", "."]
font_paths = ["fonts"]
use_system_fonts = false
```

- Paths are relative to the working directory, not to the config file.
- Keys you leave out keep their default. Unknown keys are ignored.
- A value of the wrong type stops typstgen with the TOML error and exit code `1`:

  ```text
  error: invalid config: TOML parse error at line 1, column 18
    |
  1 | template_paths = 3
    |                  ^
  invalid type: integer `3`, expected a sequence
  ```

How the template directory is chosen and imports are resolved is described in
[Templates and imports](../../guides/templates/); fonts in [Fonts](../../guides/fonts/).
