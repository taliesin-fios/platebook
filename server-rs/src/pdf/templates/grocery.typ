// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Session grocery list, grouped by store section.
#import "theme.typ": *
#let d = json("data.json")

#show: doc-page.with(website: d.website, footer-right: [Prepared #d.date])

#header-card(
  d.business,
  [Shopping List — #d.client],
  d.date,
  aside: pill(fill: warm-accent, size: 7.5pt, bold: true)[Serves #d.serves],
)

#v(10mm)

#if d.notes != "" {
  block(width: 100%, fill: soft-green, radius: 3mm, inset: (x: 6mm, top: 4.5mm, bottom: 4.5mm), below: 6mm, {
    text(size: 8.5pt, weight: "bold")[Chef Notes]
    v(2.2mm)
    text(size: 8pt, fill: secondary, d.notes)
  })
}

#for sec in d.sections {
  section-bar(sec.name, sec.count)
  v(2mm)
  for item in sec.items {
    block(breakable: false, below: if item.sources != "" { 2.6mm } else { 3mm }, inset: (left: 2mm), grid(
      columns: (4.5mm, 1fr),
      column-gutter: 2.5mm,
      checkbox(),
      {
        text(size: 9pt, weight: "bold", item.name)
        h(1mm)
        text(size: 9pt, fill: secondary)[— #item.quantity]
        if item.sources != "" {
          v(1.6mm)
          text(size: 7pt, fill: footer-text, item.sources)
        }
      },
    ))
  }
  v(5mm)
}
