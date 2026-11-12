// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Allergen detection from ingredient names, on the server.
//!
//! One source of truth: `ALLERGEN_KEYWORDS` / `ALLERGEN_EXCLUSIONS` in
//! `client/src/constants.ts` own the list (the client also uses it to check a
//! recipe while it is being edited). This copy is what the server checks with,
//! and the `matches_the_client_constants` test fails if the two drift, so edit
//! the client file first and then this one. Matching is whole-word and
//! case-insensitive, so "egg" doesn't match "eggplant" and "butter" doesn't
//! match "peanut butter".
//!
//! Session saves use it to refuse a dish that conflicts with the client's
//! allergies unless the chef acknowledged it, and the meal plan PDF uses it
//! so it only says what was actually checked.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgConnection;

/// The allergens the app knows, each with the keywords that flag it.
pub const KEYWORDS: &[(&str, &[&str])] = &[
    (
        "Dairy",
        &[
            "milk",
            "cream",
            "butter",
            "cheese",
            "ricotta",
            "parmesan",
            "gruyere",
            "mozzarella",
            "yogurt",
            "ghee",
            "crème fraîche",
            "brie",
            "feta",
            "burrata",
            "mascarpone",
            "half-and-half",
            "buttermilk",
        ],
    ),
    (
        "Gluten",
        &[
            "flour",
            "bread",
            "breadcrumbs",
            "cracker",
            "crackers",
            "pasta",
            "noodles",
            "panko",
            "wheat",
            "barley",
            "malt",
            "rye",
            "beer",
            "ale",
            "lager",
            "soy sauce",
            "semolina",
            "couscous",
            "orzo",
            "farro",
            "spelt",
            "bulgur",
            "cake",
            "pastry",
        ],
    ),
    (
        "Eggs",
        &[
            "egg",
            "eggs",
            "egg yolk",
            "egg white",
            "whole egg",
            "mayonnaise",
            "aioli",
        ],
    ),
    (
        "Tree Nuts",
        &[
            "almond",
            "walnut",
            "pecan",
            "cashew",
            "pistachio",
            "hazelnut",
            "pine nut",
            "macadamia",
            "chestnut",
        ],
    ),
    ("Peanuts", &["peanut", "peanut butter", "groundnut"]),
    (
        "Fish",
        &[
            "anchovy",
            "salmon",
            "tuna",
            "cod",
            "halibut",
            "sea bass",
            "tilapia",
            "fish sauce",
            "worcestershire",
        ],
    ),
    (
        "Shellfish",
        &[
            "shrimp", "crab", "lobster", "scallop", "clam", "mussel", "oyster", "prawn",
        ],
    ),
    (
        "Soy",
        &["soy sauce", "tofu", "edamame", "miso", "tamari", "tempeh"],
    ),
    ("Sesame", &["sesame", "tahini", "sesame oil"]),
];

/// Text that looks like an allergen keyword but isn't that allergen.
const EXCLUSIONS: &[(&str, &[&str])] = &[
    (
        "Dairy",
        &[
            r"peanut\s+butter",
            r"almond\s+butter",
            r"cashew\s+butter",
            r"sunflower\s+butter",
            r"nut\s+butter",
            r"cocoa\s+butter",
            r"apple\s+butter",
            r"shea\s+butter",
            r"body\s+butter",
        ],
    ),
    ("Eggs", &["eggplant", "eggnog"]),
];

struct Matcher {
    allergen: &'static str,
    keywords: Vec<Regex>,
    exclusions: Vec<Regex>,
}

static MATCHERS: LazyLock<Vec<Matcher>> = LazyLock::new(|| {
    KEYWORDS
        .iter()
        .map(|(allergen, words)| Matcher {
            allergen,
            keywords: words
                .iter()
                .map(|w| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(w))).expect("regex"))
                .collect(),
            exclusions: EXCLUSIONS
                .iter()
                .filter(|(a, _)| a == allergen)
                .flat_map(|(_, res)| res.iter())
                .map(|r| Regex::new(&format!("(?i){r}")).expect("regex"))
                .collect(),
        })
        .collect()
});

/// The known allergens found in these texts (ingredient names), in list order.
pub fn detect<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<&'static str> {
    let texts: Vec<&str> = texts.into_iter().collect();
    MATCHERS
        .iter()
        .filter(|m| {
            texts.iter().any(|t| {
                !m.exclusions.iter().any(|ex| ex.is_match(t))
                    && m.keywords.iter().any(|kw| kw.is_match(t))
            })
        })
        .map(|m| m.allergen)
        .collect()
}

/// The app's name for an allergen the client listed ("tree nuts" → "Tree Nuts"),
/// or `None` for one it has no keywords for (those can't be checked).
pub fn known(name: &str) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .map(|(a, _)| *a)
        .find(|a| a.eq_ignore_ascii_case(name.trim()))
}

/// A stored list column (`clients.allergens`) as strings. Older rows can hold
/// the JSON array as text, or a bare string.
pub fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(parsed @ Value::Array(_)) => string_list(Some(&parsed)),
            _ if s.trim().is_empty() => vec![],
            _ => vec![s.clone()],
        },
        _ => vec![],
    }
}

/// A session dish that contains some of the client's allergens.
#[derive(Debug, Clone, Serialize)]
pub struct DishConflict {
    pub id: i64,
    pub name: String,
    pub allergens: Vec<&'static str>,
}

/// The session's dishes that contain one of its client's allergens, checked
/// against each recipe's name and ingredient names. Reads through the
/// caller's (tenant) transaction, so RLS scopes it.
pub async fn session_conflicts(
    conn: &mut PgConnection,
    session_id: i64,
) -> Result<Vec<DishConflict>, sqlx::Error> {
    let client_allergens: Option<Option<Value>> = sqlx::query_scalar(
        "SELECT c.allergens FROM sessions s JOIN clients c ON c.id = s.client_id WHERE s.id = $1",
    )
    .bind(session_id)
    .fetch_optional(&mut *conn)
    .await?;
    let wanted: Vec<&'static str> = string_list(client_allergens.flatten().as_ref())
        .iter()
        .filter_map(|a| known(a))
        .collect();
    if wanted.is_empty() {
        return Ok(vec![]);
    }
    let dishes: Vec<(i64, String, Vec<String>)> = sqlx::query_as(
        "SELECT r.id, r.name,
                COALESCE(array_agg(i.name) FILTER (WHERE i.name IS NOT NULL), '{}') AS ingredients
         FROM session_dishes sd
         JOIN recipes r ON r.id = sd.recipe_id
         LEFT JOIN ingredients i ON i.recipe_id = r.id
         WHERE sd.session_id = $1
         GROUP BY r.id, r.name
         ORDER BY MIN(sd.sort_order), r.id",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(dishes
        .into_iter()
        .filter_map(|(id, name, ingredients)| {
            let found: Vec<&'static str> = detect(
                std::iter::once(name.as_str()).chain(ingredients.iter().map(String::as_str)),
            )
            .into_iter()
            .filter(|a| wanted.contains(a))
            .collect();
            (!found.is_empty()).then_some(DishConflict {
                id,
                name,
                allergens: found,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_whole_words_only() {
        assert_eq!(detect(["whole milk"]), ["Dairy"]);
        assert_eq!(detect(["Egg yolk"]), ["Eggs"]);
        assert_eq!(detect(["eggplant", "eggnog"]), Vec::<&str>::new());
        assert_eq!(detect(["peanut butter"]), ["Peanuts"]);
        assert_eq!(detect(["cocoa butter"]), Vec::<&str>::new());
        assert_eq!(detect(["butter"]), ["Dairy"]);
        assert_eq!(detect(["lasagna noodles", "ricotta"]), ["Dairy", "Gluten"]);
        assert_eq!(
            detect(["shrimp", "peanut", "wheat flour"]),
            ["Gluten", "Peanuts", "Shellfish"]
        );
        assert_eq!(detect(["MOZZARELLA, shredded"]), ["Dairy"]);
        assert_eq!(detect(Vec::<&str>::new()), Vec::<&str>::new());
    }

    #[test]
    fn known_names_ignore_case() {
        assert_eq!(known("tree nuts"), Some("Tree Nuts"));
        assert_eq!(known(" Dairy "), Some("Dairy"));
        assert_eq!(known("Mushrooms"), None);
    }

    #[test]
    fn reads_stored_lists() {
        assert_eq!(string_list(Some(&json!(["Dairy", 3, null]))), ["Dairy"]);
        assert_eq!(string_list(Some(&json!("[\"Soy\"]"))), ["Soy"]);
        assert_eq!(string_list(Some(&json!("Tree Nuts"))), ["Tree Nuts"]);
        assert_eq!(string_list(Some(&json!(""))), Vec::<String>::new());
        assert_eq!(string_list(None), Vec::<String>::new());
    }

    /// The quoted strings of the first `[...]` on each `name: [...]` line.
    fn parse_lists(block: &str, quote: char) -> Vec<(String, Vec<String>)> {
        block
            .lines()
            .filter_map(|line| {
                let (key, rest) = line.split_once(":")?;
                let rest = rest.trim_start().strip_prefix("[")?;
                let key = key.trim().trim_matches(quote).to_string();
                let items = rest.split_once("]")?.0;
                let items = items
                    .split(',')
                    .map(|i| i.trim().trim_matches(quote).to_string())
                    .filter(|i| !i.is_empty())
                    .collect();
                Some((key, items))
            })
            .collect()
    }

    /// The text of `export const <name>` up to its closing `};`.
    fn ts_block<'a>(src: &'a str, name: &str) -> &'a str {
        let start = src
            .find(&format!("const {name}"))
            .expect("constant in constants.ts");
        let body = &src[start..];
        &body[..body.find("\n};").expect("end of block")]
    }

    #[test]
    fn matches_the_client_constants() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/src/constants.ts");
        let src = std::fs::read_to_string(path).expect("client/src/constants.ts");

        let client = parse_lists(ts_block(&src, "ALLERGEN_KEYWORDS"), '\'');
        let server: Vec<(String, Vec<String>)> = KEYWORDS
            .iter()
            .map(|(a, w)| (a.to_string(), w.iter().map(|s| s.to_string()).collect()))
            .collect();
        assert_eq!(
            server, client,
            "keywords differ from client/src/constants.ts"
        );

        // Exclusions are written `/regex/i` there and `regex` here.
        let client_ex: Vec<(String, Vec<String>)> = ts_block(&src, "ALLERGEN_EXCLUSIONS")
            .lines()
            .filter_map(|line| {
                let (key, rest) = line.split_once(":")?;
                let rest = rest.trim_start().strip_prefix("[")?;
                let items = rest.split_once("]")?.0;
                let items = items
                    .split(", ")
                    .map(|i| {
                        i.trim()
                            .trim_start_matches('/')
                            .trim_end_matches("/i")
                            .to_string()
                    })
                    .collect();
                Some((key.trim().to_string(), items))
            })
            .collect();
        let server_ex: Vec<(String, Vec<String>)> = EXCLUSIONS
            .iter()
            .map(|(a, r)| (a.to_string(), r.iter().map(|s| s.to_string()).collect()))
            .collect();
        assert_eq!(
            server_ex, client_ex,
            "exclusions differ from client/src/constants.ts"
        );
    }
}
