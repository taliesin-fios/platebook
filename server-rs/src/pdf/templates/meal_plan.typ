// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

// Client meal plan: one card per dish. Only what the chef entered (dish
// names, servings, allergies on file): no reheating, freezing, "use by" or
// storage advice, since no recipe records any.
#import "theme.typ": *
#let d = json("data.json")

#show: doc-page.with(website: d.website, footer-right: d.date)

#header-card(
  height: 50mm,
  d.business,
  [#d.client — Weekly Meals],
  d.date,
  aside: pill(fill: warm-accent, size: 7.5pt, bold: true)[Serves #d.serves],
)

#v(6mm)

// Meal cards
#for (i, dish) in d.dishes.enumerate() {
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
      fill: (x, _) => if x == 0 { soft-green },
      [],
      block(inset: (left: 4.5mm, right: 5mm, top: 4mm, bottom: 3.5mm), {
        grid(
          columns: (1fr, auto),
          column-gutter: 4mm,
          align: horizon,
          text(size: 11pt, weight: "bold", fill: primary, dish.name),
          pill[Serves #d.serves],
        )
      }),
    ),
  )
}

#v(10mm)

// Allergens
#if d.allergens.len() > 0 {
  block(width: 100%, breakable: false, fill: allergen-fill, radius: 3mm, inset: (x: 7mm, top: 5mm, bottom: 5mm), below: 6mm, {
    set text(fill: allergen-text)
    text(size: 8.5pt, weight: "bold")[Allergen Awareness]
    v(2.5mm)
    set text(size: 7.5pt)
    // Only say what the server checked: matches are named, and "nothing
    // matched" is said only when every allergen on file has keywords.
    let contains = d.at("contains", default: ())
    let unchecked = d.at("unchecked", default: d.allergens)
    [Allergies on file: #d.allergens.join(", ").]
    for c in contains {
      linebreak()
      strong[Contains #c.allergen: #c.dishes.join(", ").]
    }
    if contains.len() == 0 and unchecked.len() == 0 {
      [ No ingredient in these dishes matched them.]
    }
    if unchecked.len() > 0 {
      [ Not checked against the ingredients: #unchecked.join(", ").]
    }
    linebreak()
    [Please confirm with your chef before eating.]
  })
}

#v(2mm)
#text(size: 8.5pt, style: "italic", fill: secondary)[Prepared with care by your chef]
