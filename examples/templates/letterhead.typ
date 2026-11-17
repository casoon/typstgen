// A small letter template. Documents import it by file name; typstgen looks
// it up in every existing entry of `template_paths`.
#let accent = rgb("#ad3216")

#let letterhead(sender: none, recipient: none, date: none, subject: none, body) = {
  set page(paper: "a4", margin: (x: 25mm, top: 20mm, bottom: 25mm))
  set text(size: 11pt)
  set par(justify: true)
  // Tables: hairlines between rows, a tinted header, no vertical rules.
  set table(
    stroke: (x: none, y: 0.5pt + luma(205)),
    inset: (x: 7pt, y: 7pt),
    fill: (_, y) => if y == 0 { luma(246) },
  )
  show table.cell.where(y: 0): set text(size: 10pt, fill: accent)

  block(width: 100%, stroke: (top: 2.5pt + accent), inset: (top: 7pt))[
    #align(right, text(size: 9pt, fill: luma(90), sender))
  ]
  v(16mm)
  recipient
  v(12mm)
  align(right, date)
  v(6mm)
  text(weight: "bold", subject)
  v(2mm)
  body
}
