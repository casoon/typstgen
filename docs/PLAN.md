# typstgen — Entwicklungsplan

## Ziel

Ein Tool, das eine bestehende `.typ`-Datei entgegennimmt und daraus ein
fertiges PDF erzeugt — ohne externes `typst`-Binary, ohne Subprocess.
Der Template-Pfad für die `#import`-Auflösung wird über eine Config-Datei
bestimmt, nicht hartcodiert. Nutzbar als:

- CLI (`typstgen compile input.typ`)
- Rust-Lib (`typstgen::compile(path, &config)`)
- Wasm (Browser/Cloudflare-Worker-tauglich)

Bewusst kein Ersatz für `docgen` als Ganzes — kein Client-/Projekt-Management,
keine Datenbank, kein interaktiver Modus. Nur: `.typ` rein, PDF raus.

## Ausgangslage: zwei bestehende eigene Projekte

Beide liegen lokal unter `~/GitHub/` und lösen Teile desselben Problems
bereits — typstgen soll auf ihren Erfahrungen aufbauen, nicht bei null neu
erfinden.

### `typst-business-templates` (= `docgen`, `~/GitHub/typst-business-templates`)

Business-Dokumente (Rechnung, Angebot, Konzept, ...) aus JSON + Typst-Templates.
Genau das Tool, das `casoon-documents` aktuell nutzt (`generate.sh` → `docgen compile`).

**Was funktioniert und übernommen werden sollte:**

- Aktuelle Version (CLI-Crate `docgen` 0.7.2, `~/GitHub/typst-business-templates/cli`)
  kompiliert bereits *embedded* — `src/world.rs` implementiert `typst::World`
  direkt, kein Shell-out zu einem externen `typst`-Prozess mehr. (Die lokal
  installierte Binary `~/bin/docgen` ist mit 0.5.2 veraltet und prüft noch auf
  ein externes `typst` — das ist der alte Stand, nicht der aktuelle Quellcode.)
- Templates + Fonts werden via `include_dir` zur Compile-Zeit ins Binary
  eingebettet (`cli/src/local_templates.rs`) — ein Binary, keine Laufzeit-Dateien.

**Was bewusst NICHT übernommen wird — mit Begründung aus dem eigenen Repo:**

- `docs/SIMPLIFICATION-PLAN.md` (docgen-Projekt selbst): SQLite-Client-/Projekt-
  Verwaltung, Nummernkreise und interaktiver Modus waren ein Fehler
  ("docgen = PDF-Generator, nicht mehr, nicht weniger" — Analogie zu `pandoc`,
  `imagemagick`, `ffmpeg`). Zitat: "80% der User nutzen es nie", "Dual Source
  of Truth", Migrations-/Merge-Probleme mit der SQLite-Datei. → typstgen hat
  diese Features von Anfang an nicht.
- `docs/REVIEW-2026-01.md`: `main.rs` mit 1.384 Zeilen, 0% Testabdeckung,
  totes Code aus einem verworfenen Package-System. → typstgen fängt modular
  an (`config.rs`, `error.rs`, `wasm.rs` getrennt) und braucht ab Phase 1
  echte Tests, nicht nachträglich.
- `docs/ROADMAP.md` Abschnitt "Template Resolution Strategy": eine 3-stufige
  Auflösung war geplant (`./templates` → `~/.config/docgen/templates` →
  System-Pfad), aber nie als echte Config-Datei umgesetzt — nur die
  Projekt-lokale Stufe existiert real (`.docgen/templates/`, hartcodiert).
  → Das ist genau die Lücke, die typstgen schließt: `typstgen.toml` mit
  `template_paths` als expliziter, dokumentierter Config-Wert
  (`src/config.rs`), nicht implizit im Code vergraben.

### `renderreport` (`~/GitHub/renderreport`, auf crates.io veröffentlicht)

Datengetriebene Report-Engine (JSON-Components → Typst → PDF). Anderes
Paradigma als typstgen (Components statt fertiger `.typ`-Datei), aber die
**Engineering-Bauteile passen fast 1:1**:

- `Cargo.toml`: ein Crate, `[lib] crate-type = ["rlib", "cdylib"]`,
  `[[bin]]` mit `required-features = ["cli"]`, Features `cli` (clap) und
  `wasm` (wasm-bindgen) — direkt für typstgens `Cargo.toml` übernommen.
- `src/wasm.rs`: Kommentar dort bringt den entscheidenden Punkt auf den
  Punkt — "No filesystem/system fonts are available in that environment,
  so the engine is configured to rely solely on the embedded fallback
  fonts." `fontdb`s System-Font-Scan funktioniert nicht auf
  `wasm32-unknown-unknown`. → typstgens World-Implementierung muss von
  Anfang an zwei Modi kennen: `use_system_fonts` (nativ) vs. immer
  eingebettete Fonts (Wasm), nicht als nachträglicher Sonderfall.
- `src/engine/config.rs` (`EngineConfig` mit `pack_paths: Vec<PathBuf>`)
  ist strukturell genau `typstgen::Config::template_paths` — dieselbe Idee,
  hier von "Template-Pack-Pfaden" auf "Template-Import-Pfade" verengt.
- `.github/workflows/ci.yml`: Formatierung, Clippy mit `-D warnings`,
  Feature-Matrix — als Vorlage für `typstgen`s CI übernommen (siehe
  `.github/workflows/ci.yml` in diesem Repo).

## Geteiltes Crate: Kandidat für später, nicht Phase 1

`typst-business-templates/src/world.rs` (234 Zeilen) und
`renderreport/src/engine/world.rs` (257 Zeilen) sind strukturell fast
identisch: `typst::World`-Trait-Implementierung mit virtuellem Dateisystem,
eingebettetem `FontBook`, `sys.inputs`-Injection. Eine dritte, wieder leicht
andere Implementierung in typstgen wäre die dritte Kopie desselben Codes.

**Vorschlag:** Sobald typstgen v0.1 steht und funktioniert, die World-/Font-
Logik aus beiden bestehenden Projekten in ein gemeinsames Crate extrahieren
(Arbeitstitel `embedded-typst`, auf crates.io frei) und `renderreport` +
`typstgen` beide darauf umstellen. Bewusst **nicht** als Voraussetzung für
Phase 1 — sonst blockiert ein Refactor von `renderreport` den Start von
typstgen. Erst extrahieren, wenn zwei echte Nutzer (renderreport, typstgen)
den gemeinsamen Code klar erkennen lassen, nicht vorher spekulativ bauen.

## Phase 1 — Kernfunktion (erledigt)

`src/lib.rs::compile()` kompiliert nun eingebettet über `typst::compile()` und
`typst_pdf::pdf()`:

1. `src/world.rs`: `typst::World`-Implementierung, angelehnt an
   `typst-business-templates/src/world.rs` — virtuelles Dateisystem, das
   `#import`-Pfade relativ zu `Config::resolve_template_path()` auflöst.
2. Font-Loading: eingebettete Typst-Fallback-Fonts sind immer verfügbar;
   System-Fonts werden nur nativ und nur bei `Config::use_system_fonts`
   geladen.
3. `compile()` ist mit `typst::compile()` → `typst_pdf::pdf()` → `Vec<u8>`
   verdrahtet.
4. Integrationstests validieren sowohl die Library als auch
   `typstgen compile` mit einem konfigurierten Template-Import und einem
   echten PDF-Header. Die vorhandenen CASOON-Dokumente verwenden zusätzlich
   docgens `/data`- und `/locale`-Injection, die bewusst außerhalb dieses
   Crate-Scopes liegt.

## Phase 2 — Wasm (erledigt)

`src/wasm.rs::compile()` nimmt ein JSON-Objekt mit Quelltext und referenzierten
Template-Dateien als virtuellem Dateisystem entgegen. Auf `wasm32` werden nur
die eingebetteten Fallback-Fonts verwendet.

## Phase 3 — Konsolidierung (optional)

Extraktion von `embedded-typst` (siehe oben), sobald beide Projekte stabil
sind und der doppelte Code sichtbar unangenehm wird.

## Bewusst außerhalb des Scopes

- Kein Datenbank-/Client-/Projekt-Management (siehe Lessons-Learned oben).
- Kein JSON→Template-Daten-Merging wie bei `docgen compile invoice.json` —
  typstgen kompiliert eine fertige `.typ`-Datei, Daten-Injection ist Sache
  des Aufrufers (`sys.inputs`, falls später gebraucht).
- Keine Paket-Registry/`@local`-Package-Auflösung — nur Config-gesteuerte
  lokale Pfade.
