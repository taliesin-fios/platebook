// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Ingredient quantities, shared by the session grocery list (`routes/grocery.rs`)
//! and the Planner (`routes/planner.rs`) so both print amounts the same way:
//! units merge on a normal form ("cups" = "cup", "Tablespoons" = "tbsp"), amounts
//! round to the nearest 1/8 ("1 1/2 cups", never "1.5 cups"), and a known unit is
//! printed in one spelling, singular or plural by the amount ("1 cup", "2 cups").

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

static TRAILING_ZEROS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.?0+$").expect("regex"));

static UNIT_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("tsp", "tsp"),
        ("teaspoon", "tsp"),
        ("teaspoons", "tsp"),
        ("ts", "tsp"),
        ("tbsp", "tbsp"),
        ("tablespoon", "tbsp"),
        ("tablespoons", "tbsp"),
        ("tbs", "tbsp"),
        ("tb", "tbsp"),
        ("cup", "cup"),
        ("cups", "cup"),
        ("oz", "oz"),
        ("ounce", "oz"),
        ("ounces", "oz"),
        ("fl oz", "oz"),
        ("lb", "lb"),
        ("lbs", "lb"),
        ("pound", "lb"),
        ("pounds", "lb"),
        ("g", "g"),
        ("gram", "g"),
        ("grams", "g"),
        ("gr", "g"),
        ("kg", "kg"),
        ("kilogram", "kg"),
        ("kilograms", "kg"),
        ("ml", "ml"),
        ("milliliter", "ml"),
        ("milliliters", "ml"),
        ("l", "l"),
        ("liter", "l"),
        ("liters", "l"),
        ("litre", "l"),
        ("litres", "l"),
        ("clove", "cloves"),
        ("cloves", "cloves"),
        ("can", "can"),
        ("cans", "can"),
        ("bunch", "bunch"),
        ("bunches", "bunch"),
        ("sprig", "sprig"),
        ("sprigs", "sprig"),
        ("slice", "slice"),
        ("slices", "slice"),
        ("piece", "piece"),
        ("pieces", "piece"),
        ("stalk", "stalk"),
        ("stalks", "stalk"),
        ("head", "head"),
        ("heads", "head"),
        ("pinch", "pinch"),
        ("pinches", "pinch"),
        ("dash", "dash"),
        ("dashes", "dash"),
        ("quart", "qt"),
        ("quarts", "qt"),
        ("qt", "qt"),
        ("pint", "pt"),
        ("pints", "pt"),
        ("pt", "pt"),
        ("sheet", "sheet"),
        ("sheets", "sheet"),
        ("strip", "strip"),
        ("strips", "strip"),
        ("knob", "knob"),
        ("knobs", "knob"),
        ("link", "link"),
        ("links", "link"),
    ]
    .into_iter()
    .collect()
});

/// "Tablespoons" → "tbsp", "cups" → "cup". Unknown units come back lowercased.
pub fn normalize_unit(raw: &str) -> String {
    let lower = raw.to_lowercase().trim().to_string();
    UNIT_MAP
        .get(lower.as_str())
        .map(|u| u.to_string())
        .unwrap_or(lower)
}

pub(crate) fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Numeric value + unit back to "1 1/2 cups" (nearest 1/8).
pub fn format_qty(value: f64, raw_unit: &str) -> String {
    if raw_unit.is_empty() && value == 0.0 {
        return String::new();
    }
    if raw_unit == "to taste" || raw_unit == "as needed" {
        return raw_unit.to_string();
    }
    let rounded = js_round(value * 8.0) / 8.0;
    if rounded == 0.0 {
        return raw_unit.to_string();
    }
    let whole = rounded.floor();
    let frac = js_round((rounded - whole) * 8.0) / 8.0;
    const FRAC_MAP: &[(f64, &str)] = &[
        (0.875, "7/8"),
        (0.75, "3/4"),
        (0.625, "5/8"),
        (0.5, "1/2"),
        (0.375, "3/8"),
        (0.25, "1/4"),
        (0.125, "1/8"),
    ];
    let frac_str = FRAC_MAP
        .iter()
        .find(|(v, _)| (frac - v).abs() < 0.01)
        .map(|(_, s)| *s)
        .unwrap_or("");
    if (frac - 1.0).abs() < 0.01 {
        return format_qty(whole + 1.0, raw_unit);
    }
    let whole_str = format!("{}", whole as i64);
    let num = if whole > 0.0 && !frac_str.is_empty() {
        format!("{whole_str} {frac_str}")
    } else if whole > 0.0 {
        whole_str
    } else if !frac_str.is_empty() {
        frac_str.to_string()
    } else {
        TRAILING_ZEROS
            .replace(&format!("{rounded:.2}"), "")
            .into_owned()
    };
    if raw_unit.is_empty() {
        num
    } else {
        format!("{num} {raw_unit}")
    }
}

/// "  Olive  Oil (extra) " → "olive oil extra": the key ingredients merge on.
pub fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.replace(['(', ')'], "").trim().to_string()
}

/// Units printed as-is whatever the amount ("1 tbsp", "3 tbsp").
const ABBREVIATED: &[&str] = &["tsp", "tbsp", "oz", "lb", "g", "kg", "ml", "l", "qt", "pt"];

/// How to print a unit: `norm` is `normalize_unit(raw)`. A known unit gets one
/// spelling, plural when the amount (rounded to 1/8, as printed) is over 1. An
/// unknown unit ("large", "medium") is printed the way the recipe wrote it.
pub fn display_unit(norm: &str, raw: &str, value: f64) -> String {
    let known = UNIT_MAP.values().any(|u| *u == norm);
    if !known {
        return raw.trim().to_string();
    }
    if ABBREVIATED.contains(&norm) {
        return norm.to_string();
    }
    let plural = js_round(value * 8.0) / 8.0 > 1.0;
    // "cloves" is the one plural normal form (see UNIT_MAP).
    let singular = if norm == "cloves" { "clove" } else { norm };
    if !plural {
        return singular.to_string();
    }
    match singular {
        "bunch" | "pinch" | "dash" => format!("{singular}es"),
        s => format!("{s}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_units() {
        assert_eq!(normalize_unit("Tablespoons"), "tbsp");
        assert_eq!(normalize_unit("cups"), "cup");
        assert_eq!(normalize_unit("Large"), "large");
    }

    #[test]
    fn formats_like_node() {
        assert_eq!(format_qty(1.5, "cups"), "1 1/2 cups");
        assert_eq!(format_qty(0.0, ""), "");
        assert_eq!(format_qty(0.01, "g"), "g");
        assert_eq!(format_qty(2.0, ""), "2");
        assert_eq!(format_qty(0.333, "cup"), "3/8 cup");
        assert_eq!(format_qty(0.25, "tsp"), "1/4 tsp");
        assert_eq!(format_qty(0.99, ""), "1");
    }

    #[test]
    fn names_normalize() {
        assert_eq!(normalize_name("  Olive  Oil (extra) "), "olive oil extra");
    }

    #[test]
    fn displays_one_spelling_per_unit() {
        let d = |raw: &str, v: f64| display_unit(&normalize_unit(raw), raw, v);
        assert_eq!(d("cups", 0.5), "cup");
        assert_eq!(d("cup", 1.0), "cup");
        assert_eq!(d("Cup", 2.0), "cups");
        assert_eq!(d("cups", 1.01), "cup"); // prints as "1", so singular
        assert_eq!(d("Tablespoons", 2.0), "tbsp");
        assert_eq!(d("teaspoon", 0.5), "tsp");
        assert_eq!(d("pounds", 1.0), "lb");
        assert_eq!(d("clove", 3.0), "cloves");
        assert_eq!(d("cloves", 1.0), "clove");
        assert_eq!(d("bunch", 2.0), "bunches");
        assert_eq!(d("pinches", 1.0), "pinch");
        assert_eq!(d("Large", 2.0), "Large");
        assert_eq!(d("", 3.0), "");
    }
}
