// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Client meal plan: one card per dish with reheat guidance.
#import "theme.typ": *
#let d = json("data.json")

#show: doc-page.with(website: d.website, footer-right: d.date)

#header-card(
  height: 50mm,
  d.business,
  [#d.client — Weekly Meals],
  d.date,
  aside: {
    pill(fill: warm-accent, size: 7.5pt, bold: true)[Serves #d.serves]
    v(2.5mm)
    box(fill: freeze-fill, radius: 3mm, inset: (x: 2.5mm, y: 1.6mm), {
      box(baseline: 0.6mm, snowflake(2.2mm, freezer-blue))
      h(1.6mm)
      text(size: 6.5pt, weight: "bold", fill: freezer-blue)[\= Freezer Friendly]
    })
  },
)

#v(5mm)

// Reheat colour legend
#block(width: 100%, fill: soft-green, radius: 2.5mm, inset: (x: 5mm, y: 3mm), {
  set text(size: 6.5pt, fill: secondary)
  text(weight: "bold")[Reheat guide:]
  h(5mm)
  for (cat, label) in (("stovetop", "Stovetop / simmer"), ("oven", "Oven / bake"), ("cold", "Serve cold")) {
    box(width: 4mm, height: 4mm, radius: 1mm, fill: reheat-accent.at(cat), baseline: 0.8mm)
    h(1.5mm)
    label
    h(6mm)
  }
})

#v(6mm)

// Meal cards
#for (i, dish) in d.dishes.enumerate() {
  let accent = reheat-accent.at(dish.category)
  block(
    width: 100%,
    breakable: false,
    radius: 3.5mm,
    stroke: 0.3mm + card-border,
    fill: if calc.even(i) { white } else { row-alt },
    clip: true,
    below: 4mm,
    grid(
      columns: (3.5mm, 1fr),
      fill: (x, _) => if x == 0 { accent },
      [],
      block(inset: (left: 4.5mm, right: 5mm, top: 4mm, bottom: 3mm), {
        grid(
          columns: (1fr, auto),
          column-gutter: 4mm,
          text(size: 11pt, weight: "bold", fill: primary, dish.name),
          move(dy: -1.5mm, text(size: 6pt, fill: accent, reheat-label.at(dish.category))),
        )
        v(2.2mm)
        text(size: 8.5pt, fill: secondary, dish.reheat)
        v(4mm)
        pill[Serves #d.serves]
        if d.use_by != "" {
          h(3mm)
          pill[Use by #d.use_by]
        }
        if dish.freeze {
          h(3mm)
          box(fill: freeze-fill, radius: 3mm, inset: (x: 2.2mm, y: 1.2mm), {
            box(baseline: 0.5mm, snowflake(1.8mm, freezer-blue))
            h(0.6mm)
            box(baseline: 0.6mm, checkmark(3.2mm, freezer-blue))
            h(1.2mm)
            text(size: 7pt, weight: "bold", fill: freezer-blue)[FREEZE]
          })
        }
      }),
    ),
  )
}

#v(14mm)

// Storage notes
#block(width: 100%, breakable: false, fill: warm-accent, radius: 3mm, inset: (x: 7mm, top: 5mm, bottom: 5mm), below: 5mm, {
  text(size: 8.5pt, weight: "bold")[Storage & Reheating]
  v(2.5mm)
  set text(size: 7.5pt, fill: secondary)
  set par(leading: 0.9em)
  [• Keep all containers refrigerated at or below 41°F.\
  • Yogurt sauces and cold components are in separate containers — serve cold.\
  • Do not reheat cucumber, yogurt, or fresh herbs — add at serving.\
  • Dishes marked FREEZE can be stored frozen for up to 3 months.]
})

// Allergens
#if d.allergens.len() > 0 {
  block(width: 100%, breakable: false, fill: allergen-fill, radius: 3mm, inset: (x: 7mm, top: 5mm, bottom: 5mm), below: 6mm, {
    set text(fill: allergen-text)
    text(size: 8.5pt, weight: "bold")[Allergen Awareness]
    v(2.5mm)
    text(size: 7.5pt)[This menu was planned to avoid: #d.allergens.join(", ").]
  })
}

#v(2mm)
#text(size: 8.5pt, style: "italic", fill: secondary)[Prepared with care by your chef]
