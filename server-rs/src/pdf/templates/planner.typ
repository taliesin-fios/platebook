// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Menu planner: consolidated ingredient list for the selected recipes.
#import "theme.typ": *
#let d = json("data.json")

#show: doc-page.with(website: d.website, footer-right: [Generated #d.date])

#header-card(
  height: 48mm,
  d.business,
  [Menu Planner — Ingredient List],
  d.date,
  aside: pill(fill: warm-accent, size: 7.5pt, bold: true)[Serves #d.serves],
  extra: if d.recipes != "" {
    // One line, clipped like the old "…" truncation.
    block(height: 1.2em, clip: true, text(size: 7.5pt, fill: header-muted, d.recipes))
  },
)

#v(8mm)
#text(size: 8.5pt, style: "italic", fill: secondary, d.summary)
#v(5mm)

#for sec in d.sections {
  section-bar(sec.name, sec.count)
  v(2mm)
  for item in sec.items {
    block(breakable: false, below: if item.sources != "" { 2.6mm } else { 3mm }, inset: (left: 2mm), grid(
      columns: (4.5mm, 1fr, auto),
      column-gutter: 2.5mm,
      checkbox(dot: item.multi),
      {
        text(size: 9pt, weight: "bold", item.name)
        if item.sources != "" {
          v(1.6mm)
          block(height: 1.2em, clip: true, text(size: 7pt, fill: footer-text, item.sources))
        }
      },
      text(size: 8.5pt, fill: secondary, item.quantity),
    ))
  }
  v(4mm)
}
