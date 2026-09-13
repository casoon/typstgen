#!/usr/bin/env bash
# Rebuilds the website fixtures in this folder with the typstgen CLI from this
# checkout: the PDFs next to their .typ sources, the help texts and the error
# output. Run after changing an example or the compiler, then commit the result.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --manifest-path ../Cargo.toml
bin=../target/release/typstgen

for doc in hello letter fonts; do
  "$bin" compile "$doc.typ" --config typstgen.toml
done

"$bin" --help > help.txt
"$bin" compile --help > compile-help.txt

if "$bin" compile missing-import.typ --config typstgen.toml 2> missing-import.txt; then
  echo "error: missing-import.typ compiled, expected a failure" >&2
  exit 1
fi
