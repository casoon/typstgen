// Seminar paper and thesis layout (German and English).
//
// #import "@local/thesis:0.1.0": thesis
// #show: thesis.with(title: [...], author: "...", bibliography: bibliography("refs.bib"))

#let labels = (
  de: (
    supervisor: "Betreuung",
    submitted: "Eingereicht am",
    matriculation: "Matrikelnummer",
    abstract: "Zusammenfassung",
    figures: "Abbildungsverzeichnis",
    tables: "Tabellenverzeichnis",
    appendix: "Anhang",
    bibliography: "Literaturverzeichnis",
    declaration: "Eigenständigkeitserklärung",
    declaration-text: [Hiermit versichere ich, dass ich die vorliegende Arbeit selbstständig verfasst und keine anderen als die angegebenen Quellen und Hilfsmittel benutzt habe. Alle Stellen, die wörtlich oder sinngemäß aus anderen Werken übernommen wurden, sind als solche kenntlich gemacht.],
    place-date: "Ort, Datum",
    signature: "Unterschrift",
  ),
  en: (
    supervisor: "Supervisor",
    submitted: "Submitted on",
    matriculation: "Student ID",
    abstract: "Abstract",
    figures: "List of Figures",
    tables: "List of Tables",
    appendix: "Appendix",
    bibliography: "References",
    declaration: "Declaration of Authorship",
    declaration-text: [I hereby declare that I have written this thesis independently and have not used any sources or aids other than those indicated. All passages taken literally or in substance from other works are marked as such.],
    place-date: "Place, date",
    signature: "Signature",
  ),
)

/// The document layout. Apply with `#show: thesis.with(..)`.
///
/// - kind: the type of work, e.g. [Seminararbeit] or [Bachelor's Thesis]
/// - bibliography: the result of `bibliography("refs.bib", ..)`, or none
/// - appendix: content after the bibliography, headings lettered A, A.1, …
/// - declaration: auto (standard text), none, or your own content; universities
///   usually prescribe their own wording
/// - outlines: also list figures and tables (only kinds that occur are listed)
/// - chapter-pagebreak: start each chapter on a new page (thesis) or not
///   (seminar paper)
#let thesis(
  title: [Title],
  subtitle: none,
  kind: none,
  author: "Author",
  matriculation: none,
  institution: none,
  department: none,
  supervisors: (),
  date: datetime.today(),
  lang: "de",
  abstract: none,
  bibliography: none,
  appendix: none,
  declaration: auto,
  outlines: true,
  chapter-pagebreak: true,
  font: "Libertinus Serif",
  body,
) = {
  let l = labels.at(lang, default: labels.en)
  let date-text = if type(date) == datetime {
    if lang == "de" { date.display("[day].[month].[year]") } else { date.display("[month repr:long] [day], [year]") }
  } else { date }

  set document(
    title: title,
    author: author,
    date: if type(date) == datetime { date } else { auto },
  )
  set text(font: font, size: 11pt, lang: lang)
  set par(justify: true, leading: 0.7em, spacing: 1.2em)
  set page(paper: "a4", margin: (inside: 3cm, outside: 2.5cm, y: 2.5cm))
  set heading(numbering: "1.1")
  show heading.where(level: 1): it => {
    if chapter-pagebreak {
      pagebreak(weak: true)
      v(1.5cm)
    } else {
      v(0.8cm, weak: true)
    }
    set text(size: if chapter-pagebreak { 18pt } else { 15pt })
    it
    v(if chapter-pagebreak { 0.6cm } else { 0.3cm })
  }
  set math.equation(numbering: "(1)")
  set figure(gap: 0.8em)
  show figure.caption: set text(size: 9.5pt)
  show figure.where(kind: table): set figure.caption(position: top)
  set table(align: left, stroke: (x, y) => if y == 0 { (bottom: 0.6pt) } else { none }, inset: 6pt)
  show link: set text(fill: rgb("#1d4e89"))

  // Title page.
  page(margin: (x: 2.5cm, y: 3cm), numbering: none, {
    set align(center)
    if institution != none { text(size: 14pt, institution); linebreak() }
    if department != none { text(size: 12pt, department) }
    v(1fr)
    if kind != none { text(size: 12pt, upper(kind)); v(0.8cm) }
    text(size: 22pt, weight: "bold", title)
    if subtitle != none { v(0.4cm); text(size: 14pt, subtitle) }
    v(1fr)
    set align(left)
    grid(
      columns: (auto, 1fr),
      column-gutter: 1em,
      row-gutter: 0.6em,
      ..(
        (text(weight: "bold", author), []),
        if matriculation != none { (l.matriculation + ":", [#matriculation]) },
        if supervisors.len() > 0 { (l.supervisor + ":", supervisors.join(linebreak())) },
        (l.submitted + ":", [#date-text]),
      ).filter(row => row != none).flatten(),
    )
  })

  // Front matter: roman page numbers.
  set page(numbering: "I", footer: context align(center, counter(page).display("I")))
  counter(page).update(1)

  if abstract != none {
    heading(numbering: none, outlined: false, l.abstract)
    abstract
    pagebreak()
  }

  outline(depth: 3)

  if outlines {
    context {
      for (kind, title) in ((image, l.figures), (table, l.tables)) {
        if query(figure.where(kind: kind)).len() > 0 {
          v(1cm)
          outline(title: title, target: figure.where(kind: kind))
        }
      }
    }
  }

  // Main matter: arabic page numbers, chapter title in the header.
  set page(
    numbering: "1",
    header: context {
      let page-number = here().page()
      if query(heading.where(level: 1)).any(it => it.location().page() == page-number) { return }
      let previous = query(heading.where(level: 1).before(here()))
      if previous.len() > 0 {
        set text(size: 9pt)
        emph(previous.last().body)
        v(-0.6em)
        line(length: 100%, stroke: 0.4pt)
      }
    },
    footer: context align(center, counter(page).display("1")),
  )
  counter(page).update(1)

  body

  if bibliography != none {
    set heading(numbering: none)
    set std.bibliography(title: l.bibliography)
    bibliography
  }

  if appendix != none {
    set heading(numbering: "A.1", supplement: l.appendix)
    counter(heading).update(0)
    appendix
  }

  if declaration != none {
    pagebreak(weak: true)
    heading(numbering: none, outlined: false, l.declaration)
    if declaration == auto { l.declaration-text } else { declaration }
    v(2.5cm)
    grid(
      columns: (1fr, 1fr),
      column-gutter: 2cm,
      row-gutter: 0.4em,
      line(length: 100%, stroke: 0.5pt), line(length: 100%, stroke: 0.5pt),
      text(size: 9pt, l.place-date), text(size: 9pt, l.signature),
    )
  }
}
