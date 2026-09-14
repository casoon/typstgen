// The template is not next to this file: typstgen resolves the import
// against `template_paths` from typstgen.toml.
#import "letterhead.typ": letterhead

#show: letterhead.with(
  sender: [Northwind Studio · 1 Harbour Road · Example Town],
  recipient: [
    Ada Example \
    Example Ltd \
    22 Market Street \
    Example City
  ],
  date: [14 September 2026],
  subject: [Your quote request],
)

Dear Ms Example,

thank you for your enquiry. As discussed, we will set up the document
pipeline in two steps: first the templates, then the automated PDF build.

#table(
  columns: (1fr, auto),
  align: (left, right),
  table.header([*Item*], [*Price*]),
  [Letter and invoice templates], [€ 1,200],
  [PDF build in CI], [€ 600],
  [*Total*], [*€ 1,800*],
)

The offer is valid for 30 days. We look forward to hearing from you.

Kind regards \
Northwind Studio
