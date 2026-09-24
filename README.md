# typstgen

Compile an existing `.typ` file to PDF. No external `typst` binary, no
subprocess — Typst runs embedded as a Rust library. Template resolution is
config-driven instead of hardcoded.

**Website and documentation:** [casoon.github.io/typstgen](https://casoon.github.io/typstgen/)

```
your-document.typ  +  typstgen.toml (template_paths)  →  your-document.pdf
```

Usable three ways from one crate:

- **CLI** (`cli` feature, default): `typstgen compile input.typ`
- **Rust library** (always available): `typstgen::compile(path, &config)`
- **Wasm** (`wasm` feature): source and imports supplied as a JSON virtual filesystem

## Why

[`docgen`](https://github.com/casoon/typst-business-templates) already does
PDF generation from Typst, but grew into a client/project-management tool
along the way and only ever hardcoded its template lookup paths. typstgen is
the minimal version of the same idea: one job (`.typ` in, PDF out), with the
template directory resolved from config instead of baked in.

The architecture follows
[`renderreport`](https://github.com/casoon/renderreport): embedded Typst,
feature-gated CLI/Wasm, embedded fonts only on Wasm.

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

```rust
use typstgen::{compile, Config};

let config = Config::load(None)?;
let pdf_bytes = compile("example.typ".as_ref(), &config)?;
std::fs::write("example.pdf", pdf_bytes)?;
```

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
