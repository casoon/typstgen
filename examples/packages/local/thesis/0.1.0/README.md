# thesis

Layout for seminar papers and theses in German or English: title page, abstract, table of contents,
lists of figures and tables, numbered headings and equations, chapter titles in the header,
bibliography, lettered appendix and a declaration of authorship.

```typst
#import "@local/thesis:0.1.0": thesis

#show: thesis.with(
  kind: [Bachelorarbeit],
  title: [Titel der Arbeit],
  author: "Vorname Nachname",
  supervisors: ([Prof. Dr. …],),
  lang: "de",
  bibliography: bibliography("refs.bib", style: "apa"),
  appendix: [= Daten],
)

= Einleitung
…
```

| Parameter | Default | Meaning |
| --- | --- | --- |
| `title`, `subtitle`, `kind` | `[Title]`, none, none | Title page; `kind` is the type of work. |
| `author`, `matriculation` | `"Author"`, none | Author and student ID. |
| `institution`, `department`, `supervisors` | none, none, `()` | Title page header and supervisors. |
| `date` | today | A `datetime` (formatted per language) or content. Also the PDF date. |
| `lang` | `"de"` | `"de"` or `"en"`: labels, hyphenation, date format. |
| `abstract` | none | Abstract before the table of contents. |
| `bibliography` | none | The result of `bibliography(..)`, placed after the main text. |
| `appendix` | none | Content after the bibliography; headings are lettered. |
| `declaration` | `auto` | Standard declaration, `none`, or your own text. Check the wording your university prescribes. |
| `outlines` | `true` | Add lists of figures and tables (only kinds that occur). |
| `chapter-pagebreak` | `true` | Start chapters on a new page (thesis); `false` for a compact seminar paper. |
| `font` | `"Libertinus Serif"` | Body font. |

To use it outside this repository, copy `thesis/0.1.0/` into your package data directory under
`local/` (see the typstgen guide "Packages and bibliographies").
