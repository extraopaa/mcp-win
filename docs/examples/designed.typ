#set page(paper: "a4", margin: (x: 20mm, y: 18mm),
  footer: context align(right, text(size: 9pt, fill: rgb("667085"))[MCP Win · #counter(page).display("1")]))
#set text(font: "Lato", size: 11pt, fill: rgb("243047"), lang: "fr")
#set par(leading: 0.75em)
#let purple = rgb("5846c7")
#block(width: 100%, fill: purple, inset: 18pt, radius: 8pt)[
  #text(size: 10pt, fill: white)[RAPPORT · OCTOBRE 2026]
  #v(6pt)
  #text(size: 30pt, weight: "bold", fill: white)[MCP Win]
  #parbreak()
  #text(size: 14pt, fill: white)[Documents sur mesure]
]
#v(16pt)
#grid(columns: (1fr, 1fr), gutter: 18pt,
  [#block(text(size: 18pt, weight: "bold", fill: purple)[Une identité claire])
   Typographie, couleurs et espaces donnent un rythme lisible à vos documents. L’IA contrôle chaque détail de la composition.],
  block(fill: rgb("f1effb"), inset: 14pt, radius: 6pt)[
    #text(size: 24pt, weight: "bold", fill: purple)[100 % local]
    #parbreak()
    PDF, tableaux et OCR sans service payant.
  ])
#v(16pt)
#text(size: 18pt, weight: "bold", fill: purple)[Indicateurs]
#v(8pt)
#table(columns: (2fr, 1fr, 1fr), inset: 10pt,
  stroke: 0.5pt + rgb("d8dce6"),
  fill: (x, y) => if y == 0 { purple } else if calc.odd(y) { rgb("f7f8fc") },
  table.header([#text(fill: white, weight: "bold")[Fonction]], [#text(fill: white)[État]], [#text(fill: white)[Accès]]),
  [Mise en page], [Disponible], [MCP],
  [Reconnaissance OCR], [Locale], [MCP],
  [Vérification visuelle], [PNG], [MCP])
#v(18pt)
#line(length: 100%, stroke: 1pt + purple)
#text(size: 9pt, fill: rgb("667085"))[Exemple de composition : grille, tableau, couleurs, styles et pagination.]
