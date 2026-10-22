// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Invoice: the layout of the InvoiceBuilder preview card.
#let d = json("data.json")

#let navy       = rgb("#5E7F57")
#let green-dark = rgb("#3A5C32")
#let warm-white = rgb("#FBF8F1")
#let gray-50    = rgb("#F9FAFB")
#let gray-100   = rgb("#F3F4F6")
#let gray-200   = rgb("#E5E7EB")
#let gray-400   = rgb("#9CA3AF")
#let gray-500   = rgb("#6B7280")
#let gray-600   = rgb("#4B5563")
#let white70    = rgb(255, 255, 255, 179)
#let white60    = rgb(255, 255, 255, 153)
#let white80    = rgb(255, 255, 255, 204)

#set document(title: none, keywords: ("Platebook",))
#set page(paper: "us-letter", margin: 12mm, fill: white)
#set text(font: "Liberation Sans", size: 10pt, fill: navy)
#set par(leading: 0.5em, spacing: 0pt)

#let label(body) = text(size: 8pt, weight: "bold", fill: gray-400, tracking: 0.06em, upper(body))
#let lines(..items) = items.pos().filter(x => x != "").join(linebreak())

#block(width: 100%, radius: 4mm, stroke: 0.3mm + gray-100, clip: true, {
  // Header band
  block(
    width: 100%,
    fill: gradient.linear(rgb("#1B2B4B"), rgb("#2D6A4F"), angle: 45deg),
    inset: (x: 8mm, top: 8mm, bottom: 7mm),
    below: 0pt,
    {
      set text(fill: white)
      grid(
        columns: (auto, 1fr),
        column-gutter: 6mm,
        if d.logo != none {
          box(
            width: 26mm, height: 26mm, radius: 3mm, clip: true,
            stroke: 0.5mm + rgb(255, 255, 255, 51), fill: rgb(255, 255, 255, 26),
            image(d.logo, width: 100%, height: 100%, fit: "contain"),
          )
        },
        align(right, {
          if d.business != "" { text(size: 15pt, weight: "bold", d.business) }
          if d.chef_name != "" { v(1mm); text(size: 8.5pt, fill: white80, d.chef_name) }
          set text(size: 8.5pt, fill: white70)
          for line in (d.chef_email, d.chef_phone).filter(x => x != "") { v(0.8mm); line }
        }),
      )
      v(7mm)
      grid(
        columns: (1fr, auto),
        align: (left + bottom, right + bottom),
        {
          text(size: 8pt, weight: "bold", fill: white60, tracking: 0.15em)[INVOICE]
          v(1.5mm)
          text(size: 18pt, weight: "bold", d.number)
        },
        {
          set text(size: 8.5pt, fill: white70)
          if d.issued != "" [Issued: #text(fill: white, weight: "bold", d.issued)]
          if d.issued != "" and d.due != "" { v(1mm) }
          if d.due != "" [Due: #text(fill: white, weight: "bold", d.due)]
        },
      )
    },
  )

  // Body
  block(width: 100%, inset: (x: 8mm, top: 7mm, bottom: 5mm), {
    // Bill to
    if (d.client_name, d.client_email, d.client_phone, d.client_address).any(x => x != "") {
      label[Bill To]
      v(1.5mm)
      if d.client_name != "" { text(size: 10pt, weight: "bold", d.client_name); v(1mm) }
      set text(size: 8.5pt, fill: gray-500)
      for line in (d.client_email, d.client_phone, d.client_address).filter(x => x != "") {
        line
        v(1mm)
      }
      v(6mm)
    }

    // Line items
    let cols = (1fr, 14mm, 24mm, 24mm)
    block(stroke: (bottom: 0.3mm + gray-100), inset: (bottom: 2mm), width: 100%, grid(
      columns: cols,
      column-gutter: 2mm,
      label[Description], align(center, label[Qty]), align(right, label[Price]), align(right, label[Amount]),
    ))
    for li in d.items {
      block(stroke: (bottom: 0.3mm + gray-50), inset: (y: 3mm), width: 100%, below: 0pt, grid(
        columns: cols,
        column-gutter: 2mm,
        text(fill: navy, li.description),
        align(center, text(fill: gray-500, li.quantity)),
        align(right, text(fill: gray-500, li.price)),
        align(right, text(weight: "bold", li.amount)),
      ))
    }
    v(6mm)

    // Totals
    {
      set text(fill: gray-500)
      grid(columns: (1fr, auto), [Subtotal], d.subtotal)
      if d.tax != none {
        v(2mm)
        grid(columns: (1fr, auto), d.tax.label, d.tax.amount)
      }
    }
    v(2.5mm)
    block(width: 100%, stroke: (top: 0.3mm + gray-200), inset: (top: 3mm), grid(
      columns: (1fr, auto),
      text(size: 11.5pt, weight: "bold")[Total Due],
      text(size: 11.5pt, weight: "bold", fill: green-dark, d.total),
    ))

    // Note
    if d.message != "" {
      v(6mm)
      block(width: 100%, fill: warm-white, radius: 3mm, inset: (x: 5mm, y: 4mm), {
        text(size: 8pt, weight: "bold", fill: gray-400)[Note]
        v(1.5mm)
        set par(leading: 0.7em)
        text(size: 8.5pt, fill: gray-600, d.message)
      })
    }

    // Footer
    if d.business != "" {
      v(6mm)
      block(width: 100%, stroke: (top: 0.3mm + gray-100), inset: (top: 3mm),
        align(center, text(size: 8.5pt, fill: gray-400, d.business)))
    }
  })
})

#v(3mm)
#align(center, text(size: 7pt, fill: gray-400)[Made with Platebook])
