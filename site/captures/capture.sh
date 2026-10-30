#!/usr/bin/env bash
# Regenerates the terminal capture on the start page: real runs of the typstgen CLI from this
# checkout on a temporary copy of examples/, so the committed PDFs stay untouched.
#   site/captures/capture.sh
# Rebuild the SVG after a capture:
#   cd site/captures && npx -y -p @casoon/castwright@0.5.0 -p shiki castwright build compile.terminal.yaml --format svg -o . && npx -y -p @casoon/castwright@0.5.0 -p shiki castwright build compile-narrow.terminal.yaml --format svg -o . && npx -y -p @casoon/castwright@0.5.0 -p shiki castwright build einvoice.terminal.yaml --format svg -o . && npx -y -p @casoon/castwright@0.5.0 -p shiki castwright build einvoice-narrow.terminal.yaml --format svg -o .
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cargo build --release --manifest-path "$repo/Cargo.toml"
bin="${CARGO_TARGET_DIR:-$repo/target}/release/typstgen"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -R "$repo/examples/." "$work/"
cd "$work"

grep template_paths typstgen.toml > "$here/config-line.txt"
"$bin" compile letter.typ > "$here/compile-letter.txt"
if "$bin" compile missing-import.typ 2> "$here/compile-missing.txt"; then
  echo "error: missing-import.typ compiled, expected a failure" >&2
  exit 1
else
  echo "$?" > "$here/exit-missing.txt"
fi

# The invoice scene: the test fixture as XRechnung, then the same invoice without the two fields
# XRechnung adds to EN 16931 (electronic address and Leitweg-ID).
cp "$repo/tests/fixtures/zugferd/invoice.json" "$repo/tests/fixtures/zugferd/company.json" "$work/"
python3 - "$work" <<'PY'
import json, sys
path = sys.argv[1]
invoice = json.load(open(f"{path}/invoice.json"))
del invoice["recipient"]["electronic_address"]
del invoice["metadata"]["buyer_reference"]
json.dump(invoice, open(f"{path}/incomplete.json", "w"), indent=2)
PY
"$bin" e-invoice --format xrechnung-3.0 --company company.json invoice.json 2> "$here/einvoice-ok.txt"
if "$bin" e-invoice --format xrechnung-3.0 --company company.json incomplete.json 2> "$here/einvoice-missing.txt"; then
  echo "error: incomplete.json was accepted, expected a failure" >&2
  exit 1
fi

# The first page of the committed letter.pdf as the picture beside the terminal, cropped to the
# part with text (needs pdftoppm from poppler and cwebp).
pdftoppm -png -f 1 -l 1 -singlefile -scale-to-x 840 -scale-to-y -1 "$repo/examples/letter.pdf" "$work/letter"
cwebp -quiet -q 85 -crop 70 50 700 640 "$work/letter.png" -o "$here/letter-page.webp"

# Page 1 of two more committed examples, for the quickstart on the start page.
for doc in thesis fonts; do
  pdftoppm -png -f 1 -l 1 -singlefile -scale-to-x 480 -scale-to-y -1 "$repo/examples/$doc.pdf" "$work/$doc"
  cwebp -quiet -q 85 "$work/$doc.png" -o "$here/$doc-page.webp"
done
