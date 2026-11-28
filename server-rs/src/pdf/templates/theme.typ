// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Shared palette and pieces for the session / planner PDFs
// (the "hybrid_soft_badges_playful_cards" theme of the old jsPDF output).

#let page-bg       = rgb("#FCFBF7")
#let header-green  = rgb("#6F8F63")
#let header-muted  = rgb(195, 220, 195)
#let primary       = rgb("#264227")
#let secondary     = rgb("#58606A")
#let soft-green    = rgb("#EAF3E3")
#let table-header  = rgb("#DCE8D6")
#let warm-accent   = rgb("#FFF1E6")
#let allergen-fill = rgb("#FFF7D8")
#let allergen-text = rgb("#7A5418")
#let footer-text   = rgb("#6E7685")
#let row-alt       = rgb("#F6F4EE")
#let card-border   = rgb("#EADFD1")

// Letter page, warm background, website + date footer, Platebook credit.
#let doc-page(website: "", footer-right: "", body) = {
  set document(title: none, keywords: ("Platebook",))
  set text(font: "Liberation Sans", size: 9pt, fill: primary)
  set page(
    paper: "us-letter",
    fill: page-bg,
    margin: (x: 14mm, top: 9mm, bottom: 18mm),
    footer: context {
      set text(size: 7.5pt, fill: footer-text)
      grid(columns: (1fr, auto, 1fr), website, [Made with Platebook], align(right, footer-right))
    },
    footer-descent: 40%,
  )
  set par(leading: 0.45em, spacing: 0pt)
  body
}

// Rounded pill/badge.
#let pill(body, fill: table-header, color: primary, size: 7pt, bold: false) = box(
  fill: fill,
  radius: 3mm,
  inset: (x: 3.5mm, y: 1.4mm),
  text(size: size, fill: color, weight: if bold { "bold" } else { "regular" }, body),
)

// Green header card: business name, subtitle, date, and a right-hand column.
#let header-card(height: 42mm, business, subtitle, date, aside: none, extra: none) = block(
  width: 100%,
  height: height,
  fill: header-green,
  radius: 5mm,
  inset: (left: 9mm, right: 7mm, top: 6.5mm, bottom: 5mm),
  {
    set text(fill: white)
    grid(
      columns: (1fr, auto),
      column-gutter: 4mm,
      {
        text(size: 19pt, weight: "bold", business)
        v(3.2mm)
        text(size: 10pt, subtitle)
        v(3mm)
        text(size: 8pt, fill: header-muted, date)
        if extra != none {
          v(2.5mm)
          extra
        }
      },
      align(right, aside),
    )
  },
)

// Store-section bar with its item count.
#let section-bar(name, count) = block(
  width: 100%,
  fill: table-header,
  radius: 2mm,
  inset: (x: 5mm, y: 2mm),
  sticky: true,
  grid(
    columns: (1fr, auto),
    text(size: 8.5pt, weight: "bold", name),
    text(size: 7.5pt, fill: secondary, count),
  ),
)

// Empty checkbox (optionally with a green dot inside).
#let checkbox(dot: false) = box(
  width: 4.5mm,
  height: 4.5mm,
  stroke: 0.3mm + card-border,
  radius: 0.8mm,
  if dot { align(center + horizon, circle(radius: 1.2mm, fill: header-green)) },
)
