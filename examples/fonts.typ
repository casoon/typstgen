// Uses only the fonts that typstgen bundles, so it renders without any
// installed fonts (use_system_fonts = false in typstgen.toml).
#set page(paper: "a5", margin: 18mm)
#set text(size: 10.5pt)

= Bundled fonts

== Libertinus Serif
#text(font: "Libertinus Serif")[
  The quick brown fox jumps over the lazy dog. *Bold*, _italic_, *_both_*.
]

== New Computer Modern
#text(font: "New Computer Modern")[
  The quick brown fox jumps over the lazy dog. *Bold*, _italic_, *_both_*.
]

== New Computer Modern Math
$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $

== DejaVu Sans Mono
```rust
let pdf = typstgen::compile(input, &config)?;
```
