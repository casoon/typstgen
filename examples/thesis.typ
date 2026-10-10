// The layout comes from the local package in packages/local/thesis/0.1.0.
// typstgen finds it through package_path in typstgen.toml.
#import "@local/thesis:0.1.0": thesis

#show: thesis.with(
  kind: [Seminararbeit],
  title: [Reproduzierbare Dokumente mit Typst],
  subtitle: [Vom Quelltext zum archivierbaren PDF],
  author: "Ada Example",
  matriculation: "123456",
  institution: [Example University],
  department: [Fachbereich Informatik],
  supervisors: ([Prof. Dr. Grace Example],),
  date: datetime(year: 2026, month: 10, day: 1),
  lang: "de",
  chapter-pagebreak: false,
  abstract: [
    Diese Beispielarbeit zeigt die Vorlage `@local/thesis`: Titelblatt, Verzeichnisse,
    nummerierte Formeln, Abbildungen und Tabellen, Zitate aus einer BibTeX-Datei und einen
    Anhang. Inhaltlich skizziert sie, warum ein textbasierter Satz mit festem Zeitstempel zu
    bitgleichen PDF-Dateien führt.
  ],
  bibliography: bibliography("refs.bib", style: "apa"),
  appendix: [
    = Konfiguration
    Die Arbeit wurde mit dieser `typstgen.toml` übersetzt:
    #figure(
      ```toml
      template_paths = ["templates"]
      package_path = "packages"
      download_packages = false
      use_system_fonts = false
      ```,
      caption: [Konfiguration des Beispielprojekts],
    )
  ],
)

= Einleitung

Dokumente, die aus Quelltext entstehen, lassen sich wie Software behandeln: versionieren,
vergleichen und automatisch bauen. Knuth beschrieb mit dem Literate Programming bereits früh,
wie Programm und Erläuterung aus einer gemeinsamen Quelle hervorgehen @knuth1984. LaTeX hat
diesen Ansatz für wissenschaftliche Texte etabliert @lamport1994. Typst greift ihn mit einer
programmierbaren Auszeichnungssprache und inkrementeller Übersetzung neu auf @maedje2022.

Die vorliegende Arbeit fragt, unter welchen Bedingungen aus derselben Quelle stets dasselbe PDF
entsteht, und warum das für die Archivierung zählt.

= Grundlagen

== Reproduzierbarkeit

Ein Build gilt als reproduzierbar, wenn er aus derselben Quelle in jeder Umgebung bitgleiche
Ergebnisse liefert @lamb2022. Für eine Übersetzung $B$, eine Quelle $s$ und zwei Umgebungen
$e_1, e_2$ bedeutet das mit einer kryptografischen Hashfunktion $h$:

$ h(B(s, e_1)) = h(B(s, e_2)) $ <reproduzierbar>

Die Bedingung in @reproduzierbar scheitert in der Praxis an Werten, die nicht aus der Quelle stammen:
dem aktuellen Datum, zufälligen Kennungen oder Schriften, die nur auf einem Rechner installiert
sind.

== PDF/A

PDF/A ist die Norm für die Langzeitarchivierung von PDF-Dokumenten @iso19005-2. Sie verlangt
unter anderem eingebettete Schriften und Metadaten und verbietet Inhalte, die von außen
nachgeladen werden. Typst prüft diese Anforderungen bereits beim Übersetzen.

= Vorgehen

@einflussfaktoren ordnet jedem Einflussfaktor die Einstellung zu, mit der typstgen ihn festlegt.

#figure(
  table(
    columns: (auto, 1fr),
    table.header([*Einflussfaktor*], [*Festgelegt durch*]),
    [Datum und Uhrzeit], [`--creation-timestamp` bzw. `SOURCE_DATE_EPOCH`],
    [Schriften], [mitgelieferte Schriften, `use_system_fonts = false`],
    [Pakete], [Version im Import, `download_packages = false`],
    [Vorlagen], [`template_paths` im Projekt],
  ),
  caption: [Einflussfaktoren und ihre Festlegung],
) <einflussfaktoren>

Übersetzt wird mit einem Aufruf, der Norm und Zeitstempel festlegt:

#figure(
  ```sh
  typstgen compile thesis.typ --config typstgen.toml \
    --pdf-standard a-2b --creation-timestamp 1759276800
  ```,
  caption: [Übersetzung als PDF/A-2b mit festem Zeitstempel],
)

= Fazit

Werden Datum, Schriften, Pakete und Vorlagen festgelegt, entsteht aus derselben Quelle dasselbe
PDF. Damit lässt sich jede archivierte Fassung später aus dem Quelltext prüfen.
