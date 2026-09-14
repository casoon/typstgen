// A small letter template. Documents import it by file name; typstgen finds
// it in the first existing entry of `template_paths`.
#let letterhead(sender: none, recipient: none, date: none, subject: none, body) = {
  set page(paper: "a4", margin: (x: 25mm, top: 20mm, bottom: 25mm))
  set text(size: 11pt)
  set par(justify: true)

  align(right, text(size: 9pt, fill: luma(90), sender))
  v(18mm)
  recipient
  v(12mm)
  align(right, date)
  v(6mm)
  text(weight: "bold", subject)
  v(2mm)
  body
}
