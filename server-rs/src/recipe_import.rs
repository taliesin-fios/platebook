// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of the schema.org JSON-LD recipe parser in `server/routes/recipes.js`
//! (`POST /api/recipes/import/url`). Pure functions; the route does the fetch.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// An ingredient as returned by the import parsers (no ids yet).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ParsedIngredient {
    pub name: String,
    pub qty_4_serving: Option<String>,
    pub qty_6_serving: Option<String>,
    pub store_section: Option<String>,
    pub category: Option<String>,
}

/// Draft recipe returned by the import routes.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ParsedRecipe {
    pub name: Option<String>,
    pub category: Option<String>,
    pub session_fit: Option<String>,
    #[ts(type = "number | null")]
    pub prep_time_min: Option<i64>,
    #[ts(type = "number | null")]
    pub cook_time_min: Option<i64>,
    #[ts(type = "number | null")]
    pub servings_base: Option<i64>,
    #[ts(type = "number | null")]
    pub hold_quality: Option<i64>,
    #[ts(type = "number | null")]
    pub reheat_quality: Option<i64>,
    pub key_overlap_ingredients: Vec<String>,
    pub chef_notes: Option<String>,
    pub raw_instructions: Option<String>,
    pub ingredients: Vec<ParsedIngredient>,
}

static JSON_LD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<script[^>]+type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        .expect("regex")
});
static HOURS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)H").expect("regex"));
static MINUTES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)M").expect("regex"));
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]+").expect("regex"));
static QTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^([0-9\s/½¼¾⅓⅔⅛]+(?:\.[0-9]+)?(?:\s*-\s*[0-9]+)?(?:\s*(?:cup|tbsp|tsp|oz|lb|lbs|g|kg|ml|l|litre|liter|pound|ounce|tablespoon|teaspoon|clove|bunch|head|slice|piece|can|jar|package|pkg|sprig|stalk|handful|pinch|to taste)?s?\.?)?)?\s*",
    )
    .expect("regex")
});
static NAME_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[0-9\s/½¼¾⅓⅔⅛,.()\[\]]+(?:cup|tbsp|tsp|oz|lb|lbs|g|kg|ml|l|tablespoon|teaspoon|pound|ounce|clove|bunch|head|slice|piece|can|jar|package|pkg|sprig|stalk|handful|pinch)?s?\.?\s*",
    )
    .expect("regex")
});

const SECTION_KEYWORDS: &[(&str, &[&str])] = &[
    (
        "Produce",
        &[
            "vegetable",
            "tomato",
            "onion",
            "garlic",
            "pepper",
            "spinach",
            "kale",
            "potato",
            "carrot",
            "celery",
            "zucchini",
            "eggplant",
            "mushroom",
            "cucumber",
            "avocado",
            "lettuce",
            "cabbage",
            "broccoli",
            "cauliflower",
            "lemon",
            "lime",
            "orange",
            "apple",
            "pear",
            "berry",
            "peach",
            "plum",
            "herb",
            "parsley",
            "cilantro",
            "basil",
            "rosemary",
            "thyme",
        ],
    ),
    (
        "Meat & Seafood",
        &[
            "chicken",
            "beef",
            "pork",
            "lamb",
            "turkey",
            "duck",
            "sausage",
            "salmon",
            "tuna",
            "shrimp",
            "cod",
            "tilapia",
            "scallop",
            "crab",
            "lobster",
            "anchovy",
            "prawn",
            "steak",
            "ground",
            "fillet",
            "rib",
            "chop",
            "loin",
            "tenderloin",
        ],
    ),
    (
        "Dairy & Eggs",
        &[
            "milk",
            "cream",
            "butter",
            "cheese",
            "ricotta",
            "parmesan",
            "mozzarella",
            "cheddar",
            "feta",
            "brie",
            "yogurt",
            "egg",
            "crème",
            "ghee",
        ],
    ),
    (
        "Pantry & Dry Goods",
        &[
            "flour",
            "sugar",
            "rice",
            "pasta",
            "noodle",
            "oil",
            "vinegar",
            "salt",
            "pepper",
            "spice",
            "stock",
            "broth",
            "honey",
            "maple",
            "soy sauce",
            "worcestershire",
            "hot sauce",
            "mustard",
            "mayo",
            "tahini",
            "cornstarch",
            "breadcrumb",
            "panko",
            "cocoa",
            "vanilla",
            "baking",
        ],
    ),
    (
        "Canned & Jarred",
        &[
            "canned",
            "jar",
            "tomato sauce",
            "paste",
            "beans",
            "lentil",
            "chickpea",
            "coconut milk",
            "diced tomato",
        ],
    ),
    (
        "Herbs & Spices",
        &[
            "cumin",
            "paprika",
            "turmeric",
            "coriander",
            "cinnamon",
            "nutmeg",
            "oregano",
            "dried",
            "bay leaf",
            "chili flake",
            "cayenne",
            "cardamom",
            "clove",
            "fennel seed",
        ],
    ),
];

pub fn guess_section(name: &str) -> &'static str {
    let n = name.to_lowercase();
    SECTION_KEYWORDS
        .iter()
        .find(|(_, kws)| kws.iter().any(|kw| n.contains(kw)))
        .map(|(s, _)| *s)
        .unwrap_or("Other")
}

fn is_recipe(item: &Value) -> bool {
    match item.get("@type") {
        Some(Value::String(t)) => t == "Recipe",
        Some(Value::Array(ts)) => ts.iter().any(|t| t.as_str() == Some("Recipe")),
        _ => false,
    }
}

/// The first schema.org `Recipe` object in the page's JSON-LD blocks.
pub fn find_recipe(html: &str) -> Option<Value> {
    for cap in JSON_LD.captures_iter(html) {
        let Ok(data) = serde_json::from_str::<Value>(cap[1].trim()) else {
            continue; // bad JSON-LD block — skip
        };
        let candidates: Vec<Value> = match data {
            Value::Array(items) => items,
            Value::Object(ref obj) if obj.get("@graph").is_some_and(crate::js::truthy) => {
                match obj.get("@graph") {
                    Some(Value::Array(g)) => g.clone(),
                    Some(other) => vec![other.clone()],
                    None => vec![],
                }
            }
            other => vec![other],
        };
        if let Some(r) = candidates.into_iter().find(is_recipe) {
            return Some(r);
        }
    }
    None
}

/// ISO 8601 duration → minutes (PT1H30M → 90); `None` for 0 or missing.
pub fn parse_duration(d: Option<&Value>) -> Option<i64> {
    let d = d.and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let num = |re: &Regex| {
        re.captures(d)
            .and_then(|c| c[1].parse::<i64>().ok())
            .unwrap_or(0)
    };
    let total = num(&HOURS) * 60 + num(&MINUTES);
    (total != 0).then_some(total)
}

/// `recipeYield` → servings, capped at 20, default 4.
pub fn parse_yield(y: Option<&Value>) -> i64 {
    let Some(y) = y.filter(|v| crate::js::truthy(v)) else {
        return 4;
    };
    let first = match y {
        Value::Array(a) => a.first().cloned().unwrap_or(Value::Null),
        other => other.clone(),
    };
    DIGITS
        .find(&crate::js::string(&first))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .map(|n| n.min(20))
        .unwrap_or(4)
}

/// One `recipeIngredient` line → name, quantity and a guessed store section.
pub fn parse_ingredient_line(line: &str) -> ParsedIngredient {
    let text = line.trim();
    let qty = QTY
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();
    let name = NAME_PREFIX.replace(text, "").trim().to_string();
    let name = if name.is_empty() {
        text.to_string()
    } else {
        name
    };
    ParsedIngredient {
        store_section: Some(guess_section(&name).to_string()),
        name,
        qty_4_serving: (!qty.is_empty()).then_some(qty),
        qty_6_serving: None,
        category: None,
    }
}

/// `{ parsed, confidence, confidence_level }` for a schema.org Recipe.
pub fn map_recipe(r: &Value) -> (ParsedRecipe, i64, &'static str) {
    let ingredients: Vec<ParsedIngredient> = match r.get("recipeIngredient") {
        Some(Value::Array(lines)) => lines
            .iter()
            .filter_map(Value::as_str)
            .filter(|l| !l.is_empty())
            .map(parse_ingredient_line)
            .filter(|i| !i.name.is_empty())
            .collect(),
        _ => vec![],
    };
    let prep = parse_duration(r.get("prepTime"));
    let cook = parse_duration(r.get("cookTime"));
    let total = prep.unwrap_or(0) + cook.unwrap_or(0);

    let instructions = match r.get("recipeInstructions") {
        Some(Value::Array(steps)) => steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let text = match step {
                    Value::String(s) => s.clone(),
                    other => other
                        .get("text")
                        .filter(|v| crate::js::truthy(v))
                        .or_else(|| other.get("name").filter(|v| crate::js::truthy(v)))
                        .map(crate::js::string)
                        .unwrap_or_default(),
                };
                format!("{}. {text}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };

    let name = r
        .get("name")
        .filter(|v| crate::js::truthy(v))
        .map(crate::js::string);
    let lower = name.clone().unwrap_or_default().to_lowercase();
    let low_hold = ["salad", "raw", "sashimi", "ceviche", "carpaccio"]
        .iter()
        .any(|k| lower.contains(k));
    let hold = if low_hold {
        2
    } else if total > 60 {
        5
    } else {
        4
    };
    let reheat = if low_hold { 1 } else { hold };
    let notes = r
        .get("description")
        .and_then(Value::as_str)
        .filter(|d| !d.is_empty())
        .map(|d| d.chars().take(300).collect::<String>());

    let mut score = 100;
    if name.is_none() {
        score -= 25;
    }
    if ingredients.len() < 2 {
        score -= 30;
    }
    if prep.is_none() && cook.is_none() {
        score -= 15;
    }
    if ingredients.len() < 4 {
        score -= 10;
    }

    let parsed = ParsedRecipe {
        name,
        category: Some("Other".into()),
        session_fit: Some(
            if total > 90 {
                "Long-Cook"
            } else {
                "Session-Friendly"
            }
            .into(),
        ),
        prep_time_min: prep,
        cook_time_min: cook,
        servings_base: Some(parse_yield(r.get("recipeYield"))),
        hold_quality: Some(hold),
        reheat_quality: Some(reheat),
        key_overlap_ingredients: vec![],
        chef_notes: notes,
        raw_instructions: Some(instructions),
        ingredients,
    };
    (parsed, score, confidence_level(score))
}

pub fn confidence_level(score: i64) -> &'static str {
    if score >= 80 {
        "high"
    } else if score >= 50 {
        "medium"
    } else {
        "low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_recipe_in_graph_and_maps_it() {
        let html = r#"<html><script type="application/ld+json">{bad json</script>
          <script type='application/ld+json' data-x>{"@graph":[{"@type":"WebPage"},
          {"@type":["Recipe"],"name":"Braised Short Ribs","prepTime":"PT20M","cookTime":"PT2H30M",
           "recipeYield":["6 servings"],"description":"Rich.",
           "recipeIngredient":["2 lb beef short ribs","1 cup red wine","3 cloves garlic","Salt"],
           "recipeInstructions":[{"@type":"HowToStep","text":"Sear."},"Braise."]}]}</script>"#;
        let r = find_recipe(html).expect("recipe");
        let (p, score, level) = map_recipe(&r);
        assert_eq!(p.name.as_deref(), Some("Braised Short Ribs"));
        assert_eq!(p.prep_time_min, Some(20));
        assert_eq!(p.cook_time_min, Some(150));
        assert_eq!(p.session_fit.as_deref(), Some("Long-Cook"));
        assert_eq!(p.servings_base, Some(6));
        assert_eq!(p.hold_quality, Some(5));
        assert_eq!(p.raw_instructions.as_deref(), Some("1. Sear.\n2. Braise."));
        assert_eq!(p.ingredients.len(), 4);
        assert_eq!(p.ingredients[0].qty_4_serving.as_deref(), Some("2 lb"));
        assert_eq!(p.ingredients[0].name, "beef short ribs");
        assert_eq!(
            p.ingredients[0].store_section.as_deref(),
            Some("Meat & Seafood")
        );
        assert_eq!(p.ingredients[3].qty_4_serving, None);
        assert_eq!((score, level), (100, "high"));
    }

    #[test]
    fn small_helpers() {
        assert_eq!(parse_duration(Some(&json!("PT1H30M"))), Some(90));
        assert_eq!(parse_duration(Some(&json!("PT0M"))), None);
        assert_eq!(parse_yield(Some(&json!("48 cookies"))), 20);
        assert_eq!(parse_yield(None), 4);
        assert_eq!(guess_section("Fresh basil"), "Produce");
        assert_eq!(guess_section("tortillas"), "Other");
        assert!(find_recipe("<p>no data</p>").is_none());
    }
}
