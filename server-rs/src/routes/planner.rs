// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/planner.js`.
//!
//! POST /api/planner/ingredients — consolidated, scaled shopping list for a set
//! of this account's recipes, grouped by store section.
//! GET  /api/planner/pdf         — the same list as a PDF

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{NaiveDate, Utc};
use regex::Regex;
use serde::Serialize;
use sqlx::PgConnection;
use ts_rs::TS;

use crate::accounts;
use crate::db::Tenant;
use crate::error::{AppError, DbResult};
use crate::js;
use crate::pdf::{self, Template, docs};
use crate::routes::grocery::STORE_SECTION_ORDER;
use crate::routes::{JsonBody, id_list};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ingredients", post(ingredients))
        .route("/pdf", get(planner_pdf))
}

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

fn normalize_unit(raw: &str) -> String {
    let lower = raw.to_lowercase().trim().to_string();
    UNIT_MAP
        .get(lower.as_str())
        .map(|u| u.to_string())
        .unwrap_or(lower)
}

#[derive(Debug, Clone, PartialEq)]
struct Parsed {
    value: f64,
    raw_unit: String,
    norm_unit: String,
    unparseable: bool,
}

static TO_TASTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(to taste|as needed|optional|handful|a few|some)$").expect("regex")
});
static MIXED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\s+(\d+)/(\d+)\s*(.*)$").expect("regex"));
static FRAC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)/(\d+)\s*(.*)$").expect("regex"));
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+\.?\d*)\s*(.*)$").expect("regex"));
static TRAILING_ZEROS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.?0+$").expect("regex"));

fn parse_qty_string(s: &str) -> Parsed {
    let num_unit = |value: f64, raw: &str| {
        let raw_unit = raw.trim().to_string();
        Parsed {
            value,
            norm_unit: normalize_unit(&raw_unit),
            raw_unit,
            unparseable: false,
        }
    };
    if s.is_empty() {
        return num_unit(0.0, "");
    }
    let s = s.trim();
    if TO_TASTE.is_match(s) {
        return Parsed {
            value: 0.0,
            raw_unit: s.to_lowercase(),
            norm_unit: "to taste".into(),
            unparseable: true,
        };
    }
    let int = |v: &str| v.parse::<f64>().unwrap_or(f64::NAN);
    if let Some(c) = MIXED.captures(s) {
        return num_unit(int(&c[1]) + int(&c[2]) / int(&c[3]), &c[4]);
    }
    if let Some(c) = FRAC.captures(s) {
        return num_unit(int(&c[1]) / int(&c[2]), &c[3]);
    }
    if let Some(c) = NUM.captures(s) {
        return num_unit(js::parse_float_str(&c[1]).unwrap_or(f64::NAN), &c[2]);
    }
    // Cannot parse at all: treat as 1 of the full string
    Parsed {
        value: 1.0,
        raw_unit: s.to_string(),
        norm_unit: s.to_lowercase(),
        unparseable: true,
    }
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Numeric value + unit back to "1 1/2 cups" (nearest 1/8).
fn format_qty(value: f64, raw_unit: &str) -> String {
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

fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.replace(['(', ')'], "").trim().to_string()
}

/// One consolidated ingredient.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PlannerItem {
    pub name: String,
    pub quantity: String,
    pub amount: Option<f64>,
    pub unit: String,
    pub store_section: String,
    pub source_dishes: Vec<String>,
    pub multi_source: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PlannerSection {
    pub section: String,
    pub items: Vec<PlannerItem>,
}

/// POST /api/planner/ingredients.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PlannerResponse {
    pub sections: Vec<PlannerSection>,
    pub total_items: usize,
    pub recipe_count: usize,
    /// Absent when no recipes were sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recipe_names: Option<Vec<String>>,
}

#[derive(sqlx::FromRow)]
struct Row {
    name: String,
    qty_4_serving: Option<String>,
    store_section: Option<String>,
    recipe_name: String,
    recipe_base_servings: i32,
}

struct Entry {
    display_name: String,
    value: f64,
    raw_unit: String,
    unparseable: bool,
    store_section: String,
    source_dishes: Vec<String>,
}

/// `a.localeCompare(b)` approximation: case-insensitive first, then exact.
fn locale_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| b.cmp(a))
}

async fn ingredients(mut t: Tenant, body: JsonBody) -> DbResult<Json<PlannerResponse>> {
    let ids = id_list(body.get("recipe_ids")).unwrap_or_default();
    let has_ids =
        matches!(body.get("recipe_ids"), Some(serde_json::Value::Array(a)) if !a.is_empty());
    if !has_ids {
        return Ok(Json(PlannerResponse {
            sections: vec![],
            total_items: 0,
            recipe_count: 0,
            recipe_names: None,
        }));
    }
    let serving_size = match body.get("serving_size") {
        None => 4.0,
        Some(v) => js::number(v),
    };
    let res = plan(&mut t.tx, t.account.id, &ids, serving_size).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

/// The consolidated list for `ids` (this account's active recipes only).
pub async fn plan(
    conn: &mut PgConnection,
    account_id: i64,
    ids: &[i64],
    serving_size: f64,
) -> Result<PlannerResponse, sqlx::Error> {
    let rows = sqlx::query_as::<_, Row>(
        "SELECT i.name, i.qty_4_serving, i.store_section, r.name AS recipe_name,
                COALESCE(r.servings_base, 4) AS recipe_base_servings
         FROM ingredients i JOIN recipes r ON r.id = i.recipe_id
         WHERE r.account_id = $1 AND i.recipe_id = ANY($2) AND r.is_active
         ORDER BY i.recipe_id, i.id",
    )
    .bind(account_id)
    .bind(ids)
    .fetch_all(&mut *conn)
    .await?;
    let recipe_names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM recipes WHERE account_id = $1 AND id = ANY($2) AND is_active ORDER BY id",
    )
    .bind(account_id)
    .bind(ids)
    .fetch_all(&mut *conn)
    .await?;

    // Key: normalizedName::normalizedUnit → accumulator (insertion-ordered)
    let mut order: Vec<String> = Vec::new();
    let mut merged: HashMap<String, Entry> = HashMap::new();
    for row in rows {
        let base = if row.recipe_base_servings != 0 {
            row.recipe_base_servings
        } else {
            4
        };
        let scale = if base > 0 {
            serving_size / f64::from(base)
        } else {
            1.0
        };
        let mut parsed = parse_qty_string(row.qty_4_serving.as_deref().unwrap_or(""));
        if !parsed.unparseable && parsed.value != 0.0 && scale != 1.0 {
            parsed.value *= scale;
        }
        let key = format!("{}::{}", normalize_name(&row.name), parsed.norm_unit);
        match merged.get_mut(&key) {
            None => {
                order.push(key.clone());
                merged.insert(
                    key,
                    Entry {
                        display_name: row.name.trim().to_string(),
                        value: parsed.value,
                        raw_unit: parsed.raw_unit,
                        unparseable: parsed.unparseable,
                        store_section: row
                            .store_section
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "Other".into()),
                        source_dishes: vec![row.recipe_name],
                    },
                );
            }
            Some(e) => {
                if !parsed.unparseable {
                    e.value += parsed.value;
                }
                if !e.source_dishes.contains(&row.recipe_name) {
                    e.source_dishes.push(row.recipe_name);
                }
            }
        }
    }

    let all: Vec<PlannerItem> = order
        .into_iter()
        .filter_map(|k| merged.remove(&k))
        .map(|e| PlannerItem {
            quantity: if e.unparseable {
                e.raw_unit.clone()
            } else {
                format_qty(e.value, &e.raw_unit)
            },
            amount: (!e.unparseable).then_some(e.value),
            unit: e.raw_unit,
            multi_source: e.source_dishes.len() > 1,
            name: e.display_name,
            store_section: e.store_section,
            source_dishes: e.source_dishes,
        })
        .collect();
    let total_items = all.len();

    let mut by_section: HashMap<&str, Vec<PlannerItem>> = HashMap::new();
    for item in all {
        let sec = STORE_SECTION_ORDER
            .iter()
            .find(|s| **s == item.store_section)
            .copied()
            .unwrap_or("Other");
        by_section.entry(sec).or_default().push(item);
    }
    let sections = STORE_SECTION_ORDER
        .iter()
        .filter_map(|s| {
            by_section.remove(s).map(|mut items| {
                items.sort_by(|a, b| locale_cmp(&a.name, &b.name));
                PlannerSection {
                    section: s.to_string(),
                    items,
                }
            })
        })
        .collect();

    Ok(PlannerResponse {
        sections,
        total_items,
        recipe_count: recipe_names.len(),
        recipe_names: Some(recipe_names),
    })
}

/// GET /api/planner/pdf?recipe_ids=1,2,3&serving_size=4&date=YYYY-MM-DD:
/// the ingredient list as a PDF. `date` is the browser's local date for the
/// "Generated" line (UTC today when absent or invalid).
async fn planner_pdf(
    State(state): State<AppState>,
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Response> {
    pdf::check_limit(&state, t.account.id).await?;
    let ids: Vec<i64> = q
        .get("recipe_ids")
        .map(|s| s.split(',').filter_map(|p| p.trim().parse().ok()).collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return Err(AppError::validation("recipe_ids is required").into());
    }
    let serving_size = q
        .get("serving_size")
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(4.0);
    let today = q
        .get("date")
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    let res = plan(&mut t.tx, t.account.id, &ids, serving_size).await?;
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    let bytes = pdf::render_blocking(
        Template::Planner,
        docs::planner(&res, serving_size, today, &settings),
        vec![],
    )
    .await?;
    Ok(pdf::response(bytes, &format!("planner-{today}.pdf")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_units() {
        let p = parse_qty_string("1 1/2 Tablespoons");
        assert_eq!(p.value, 1.5);
        assert_eq!(p.raw_unit, "Tablespoons");
        assert_eq!(p.norm_unit, "tbsp");
        assert!(parse_qty_string("to taste").unparseable);
        let odd = parse_qty_string("a splash");
        assert!(odd.unparseable);
        assert_eq!(odd.value, 1.0);
    }

    #[test]
    fn formats_like_node() {
        assert_eq!(format_qty(1.5, "cups"), "1 1/2 cups");
        assert_eq!(format_qty(0.0, ""), "");
        assert_eq!(format_qty(0.01, "g"), "g");
        assert_eq!(format_qty(2.0, ""), "2");
        assert_eq!(format_qty(0.333, "cup"), "3/8 cup");
    }

    #[test]
    fn names_normalize() {
        assert_eq!(normalize_name("  Olive  Oil (extra) "), "olive oil extra");
    }
}
