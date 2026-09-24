---
title: Rust and Wasm
description: The public API of the crate and the Wasm export. Item-level documentation lives on docs.rs.
order: 3
---

The full API documentation is on [docs.rs/typstgen](https://docs.rs/typstgen).

## Rust

```sh
cargo add typstgen --no-default-features
```

```rust
use typstgen::{compile, Config};

let config = Config::load(None)?;
let pdf_bytes = compile("example.typ".as_ref(), &config)?;
std::fs::write("example.pdf", pdf_bytes)?;
```

| Item | Purpose |
| --- | --- |
| `compile(input: &Path, config: &Config) -> Result<Vec<u8>>` | Compiles a `.typ` file and returns the PDF bytes. Warnings are discarded. |
| `compile_with_warnings(input: &Path, config: &Config) -> Result<Output>` | Same, but returns an `Output` with `pdf` and the formatted `warnings`. |
| `Config` | `template_paths`, `font_paths`, `use_system_fonts`; see [typstgen.toml](../configuration/). Implements `Default`, `Serialize` and `Deserialize`. |
| `Config::load(Option<&Path>)` | Reads the given file, else `./typstgen.toml`, else the defaults. |
| `Config::template_roots()` | The configured template directories that exist, canonicalized, in order. |
| `Error`, `Result` | Error type and alias used by all of the above. |

`Config` can also be built in code, which is what the test suite does:

```rust
let config = Config {
    template_paths: vec!["templates".into()],
    font_paths: vec![],
    use_system_fonts: false,
};
```

### Errors

| Variant | When |
| --- | --- |
| `InputNotFound` | The input file does not exist. |
| `Config` | The config file cannot be read. |
| `InvalidConfig` | The config file is not valid TOML for `Config`. |
| `Compile` | Typst reported errors; the message lists them as `path:line:column: error: …` with their hints, followed by any warnings. |
| `Io` | Any other I/O error, for example reading the input. |

## Wasm

```sh
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
```

The `wasm` feature exports one function through `wasm-bindgen`. Generate the JavaScript bindings
with `wasm-bindgen` or `wasm-pack` as for any wasm-bindgen crate; typstgen does not publish an npm
package.

```js
// Bindings generated with: wasm-bindgen --target web --out-dir pkg \
//   target/wasm32-unknown-unknown/release/typstgen.wasm
import init, { compile } from './pkg/typstgen.js';

await init();
const pdf = compile(JSON.stringify({
  source: '#import "templates/shared.typ": title\n#title',
  files: { 'templates/shared.typ': '#let title = [Hello]' },
}));
```

- `source` is the main document, `files` maps virtual paths to their contents.
- File contents are strings, so only text files (Typst sources, CSV, JSON …) can be passed.
- The result is the PDF as a `Uint8Array`. Invalid JSON, an invalid path or a Typst error throws
  with the message.
- Only the [bundled fonts](../../guides/fonts/) are available, and `datetime.today()` has no
  date to return.
