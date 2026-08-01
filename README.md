# typstgen

> Status: early scaffold. `compile()` is a stub — see [docs/PLAN.md](docs/PLAN.md).

Compile an existing `.typ` file to PDF. No external `typst` binary, no
subprocess — Typst runs embedded as a Rust library. Template resolution is
config-driven instead of hardcoded.

```
your-document.typ  +  typstgen.toml (template_paths)  →  your-document.pdf
```

Usable three ways from one crate:

- **CLI** (`cli` feature, default): `typstgen compile input.typ`
- **Rust library** (always available): `typstgen::compile(path, &config)`
- **Wasm** (`wasm` feature): same compile logic, callable from JavaScript

## Why

[`docgen`](https://github.com/casoon/typst-business-templates) already does
PDF generation from Typst, but grew into a client/project-management tool
along the way and only ever hardcoded its template lookup paths. typstgen is
the minimal version of the same idea: one job (`.typ` in, PDF out), with the
template directory resolved from config instead of baked in.

See [docs/PLAN.md](docs/PLAN.md) for the full rationale, lessons carried over
from `docgen`, and the architecture reused from
[`renderreport`](https://github.com/casoon/renderreport) (embedded Typst,
feature-gated CLI/Wasm, embedded-fonts-only on Wasm).

## Quick start (once Phase 1 lands)

```bash
cargo install typstgen
typstgen compile documents/concepts/2026/example.typ
```

```toml
# typstgen.toml
template_paths = ["./templates", "./.typstgen/templates"]
```

```rust
use typstgen::{compile, Config};

let config = Config::load(None)?;
let pdf_bytes = compile("example.typ".as_ref(), &config)?;
std::fs::write("example.pdf", pdf_bytes)?;
```

## License

MIT
