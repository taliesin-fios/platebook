// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/grocery.js`.
//!
//! GET  /api/sessions/:id/grocery           — stored list, or one generated on the fly
//! POST /api/sessions/:id/grocery/generate  — regenerate and store
//! GET  /api/sessions/:id/grocery/pdf       — the list as a PDF
//! PUT  /api/grocery/:item_id               — toggle (or set) is_checked

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use sqlx::PgConnection;
use ts_rs::TS;

use crate::accounts;
use crate::db::Tenant;
use crate::error::{AppError, DbResult};
use crate::models::{GroceryItemRow, Ingredient};
use crate::pdf::{self, Template, docs};
use crate::quantity::{display_unit, format_qty, normalize_name, normalize_unit};
use crate::routes::{JsonBody, parse_id};
use crate::state::AppState;

/// Mounted under /api/sessions.
pub fn session_router() -> Router<AppState> {
    Router::new()
        .route("/{id}/grocery", get(list))
        .route("/{id}/grocery/generate", post(generate))
        .route("/{id}/grocery/pdf", get(grocery_pdf))
}

/// Mounted under /api/grocery.
pub fn item_router() -> Router<AppState> {
    Router::new().route("/{item_id}", put(toggle))
}

pub const STORE_SECTION_ORDER: &[&str] = &[
    "Produce",
    "Meat & Seafood",
    "Dairy & Eggs",
    "Pantry & Dry Goods",
    "Canned & Jarred",
    "Herbs & Spices",
    "Other",
];

/// A list item built from the session's recipes (not stored yet).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct GeneratedGroceryItem {
    pub ingredient_name: String,
    pub total_quantity: String,
    pub store_section: String,
    pub source_dishes: Vec<String>,
    /// Placeholder row for a dish with no ingredient data.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub no_data: Option<bool>,
}

/// GET /api/sessions/:id/grocery and POST .../grocery/generate.
#[derive(Debug, Serialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum GroceryListResponse {
    Stored {
        items: Vec<GroceryItemRow>,
        /// The route parameter, echoed back as a string.
        session_id: String,
    },
    Generated {
        items: Vec<GeneratedGroceryItem>,
        session_id: String,
        #[ts(type = "true")]
        generated: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Qty {
    value: f64,
}

static TO_TASTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(to taste|as needed|optional|a few|handful|pinch)$").expect("regex")
});
static MIXED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)\s+(\d+)/(\d+)\s*(.*)").expect("regex"));
static FRAC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)/(\d+)\s*(.*)").expect("regex"));
static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+\.?\d*)\s*(.*)").expect("regex"));
/// "2-3 cloves", "1.5–2 cups": a range of plain numbers.
static RANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d+(?:\.\d+)?)\s*[-–]\s*(\d+(?:\.\d+)?)\s*(.*)$").expect("regex")
});

/// DI12/DI13: "1 1/2 cups" → (1.5, "cups"). `None` for non-numeric amounts.
fn parse_qty_simple(s: Option<&str>) -> Option<(Qty, String)> {
    let s = s?;
    if s.is_empty() || s == "—" {
        return None;
    }
    let s = s.trim();
    if TO_TASTE.is_match(s) {
        return None;
    }
    let unit = |u: &str| u.trim().to_lowercase();
    if let Some(c) = MIXED.captures(s) {
        let denom: f64 = c[3].parse().ok()?;
        if denom == 0.0 {
            return None;
        }
        let whole: f64 = c[1].parse().ok()?;
        let num: f64 = c[2].parse().ok()?;
        return Some((
            Qty {
                value: whole + num / denom,
            },
            unit(&c[4]),
        ));
    }
    if let Some(c) = FRAC.captures(s) {
        let denom: f64 = c[2].parse().ok()?;
        if denom == 0.0 {
            return None;
        }
        let num: f64 = c[1].parse().ok()?;
        return Some((Qty { value: num / denom }, unit(&c[3])));
    }
    if let Some(c) = NUM.captures(s) {
        let value = crate::js::parse_float_str(&c[1])?;
        return Some((Qty { value }, unit(&c[2])));
    }
    None
}

struct Acc {
    item: GeneratedGroceryItem,
    /// Running total and the unit as the first recipe wrote it.
    parsed: Option<(Qty, String)>,
}

/// "1.5 Cups" → "1 1/2 cups": every numeric amount on the list in one form.
fn shown_total(value: f64, raw_unit: &str) -> String {
    format_qty(
        value,
        &display_unit(&normalize_unit(raw_unit), raw_unit, value),
    )
}

#[derive(sqlx::FromRow)]
struct Dish {
    recipe_name: String,
    recipe_id: i64,
    servings_base: Option<i32>,
}

/// Servings when neither the session nor the recipe says.
const DEFAULT_SERVINGS: i32 = 4;

/// One ingredient's amount for a session, from [`scaled_qty`].
#[derive(Debug, PartialEq)]
pub struct ScaledQty {
    /// For the "Dish: amount" source line: the recipe's own wording when no
    /// scaling was needed, else the scaled amount.
    pub source: String,
    /// The list's total for this one dish ("1 1/2 tbsp"), before merging.
    pub total: String,
    /// The scaled numeric amount and its unit, when it is a number.
    pub parsed: Option<(f64, String)>,
}

/// One ingredient's amount for `servings`: the recipe's base amount
/// (`qty_4_serving`, written for `base` servings; the column name predates
/// per-recipe bases) times `servings / base`. Falls back to the old 6-serving
/// column, read as `1.5 × base`, only when the base amount is empty.
/// A range ("2-3 cloves") scales at both ends. Any other amount that isn't a
/// number ("a handful", "1 bunch or so") comes back as written, marked when
/// the session's size differs from the recipe's, since it couldn't be scaled.
pub fn scaled_qty(
    qty_base: Option<&str>,
    qty_6: Option<&str>,
    base: i32,
    servings: i32,
) -> Option<ScaledQty> {
    let base = if base > 0 { base } else { DEFAULT_SERVINGS };
    let (qty, written_for) = match qty_base.filter(|q| !q.trim().is_empty()) {
        Some(q) => (q, f64::from(base)),
        None => (
            qty_6.filter(|q| !q.trim().is_empty())?,
            f64::from(base) * 1.5,
        ),
    };
    let factor = f64::from(servings) / written_for;
    let unscaled = (factor - 1.0).abs() < 1e-9;
    // A range scales at both ends ("2-3 cloves" for half → "1-1 1/2 cloves").
    // It isn't summed with other amounts, so it carries no parsed value.
    if let Some(c) = RANGE.captures(qty.trim()) {
        let shown = match (c[1].parse::<f64>(), c[2].parse::<f64>()) {
            (Ok(lo), Ok(hi)) if !unscaled => format!(
                "{}-{}",
                format_qty(lo * factor, ""),
                shown_total(hi * factor, c[3].trim())
            ),
            _ => qty.to_string(),
        };
        return Some(ScaledQty {
            source: shown.clone(),
            total: shown,
            parsed: None,
        });
    }
    Some(match parse_qty_simple(Some(qty)) {
        Some((q, unit)) => {
            let value = q.value * factor;
            let total = shown_total(value, &unit);
            ScaledQty {
                source: if unscaled {
                    qty.to_string()
                } else {
                    total.clone()
                },
                total,
                parsed: Some((value, unit)),
            }
        }
        None => {
            let shown = if unscaled || TO_TASTE.is_match(qty.trim()) {
                qty.to_string()
            } else {
                format!("{} (as written for {written_for})", qty.trim())
            };
            ScaledQty {
                source: shown.clone(),
                total: shown,
                parsed: None,
            }
        }
    })
}

/// Port of `generateGroceryList(db, sessionId, servingSize)`: every recipe
/// scaled from its own base servings to the session's serving size.
pub async fn generate_grocery_list(
    conn: &mut PgConnection,
    session_id: i64,
    serving_size: Option<i32>,
) -> Result<Vec<GeneratedGroceryItem>, sqlx::Error> {
    let dishes = sqlx::query_as::<_, Dish>(
        "SELECT r.name AS recipe_name, r.id AS recipe_id, r.servings_base
         FROM session_dishes sd JOIN recipes r ON r.id = sd.recipe_id
         WHERE sd.session_id = $1 ORDER BY sd.sort_order, sd.id",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await?;

    // Insertion-ordered map, like the JS object it replaces.
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Acc> = HashMap::new();
    let servings = serving_size.filter(|s| *s > 0).unwrap_or(DEFAULT_SERVINGS);

    for dish in &dishes {
        let ingredients = sqlx::query_as::<_, Ingredient>(
            "SELECT * FROM ingredients WHERE recipe_id = $1 ORDER BY id",
        )
        .bind(dish.recipe_id)
        .fetch_all(&mut *conn)
        .await?;

        if ingredients.is_empty() {
            let key = format!("__nodata__{}", dish.recipe_id);
            if !map.contains_key(&key) {
                order.push(key.clone());
            }
            map.insert(
                key,
                Acc {
                    item: GeneratedGroceryItem {
                        ingredient_name: format!("[No ingredient data for {}]", dish.recipe_name),
                        total_quantity: "Add manually".into(),
                        store_section: "Other".into(),
                        source_dishes: vec![dish.recipe_name.clone()],
                        no_data: Some(true),
                    },
                    parsed: None,
                },
            );
            continue;
        }

        for ing in ingredients {
            let scaled = scaled_qty(
                ing.qty_4_serving.as_deref(),
                ing.qty_6_serving.as_deref(),
                dish.servings_base.unwrap_or(DEFAULT_SERVINGS),
                servings,
            );
            let qty = scaled.as_ref().map(|s| s.total.clone());
            let parsed = scaled
                .as_ref()
                .and_then(|s| s.parsed.clone())
                .map(|(value, unit)| (Qty { value }, unit));
            // DI13: dedup on name + unit so "butter (cups)" and "butter (tbsp)" stay apart,
            // while "cup" and "cups" (or "tbsp" and "Tablespoons") merge.
            let unit = parsed
                .as_ref()
                .map(|(_, u)| normalize_unit(u))
                .unwrap_or_default();
            let key = format!("{}|{unit}", normalize_name(&ing.name));
            let shown = scaled
                .as_ref()
                .map_or_else(|| "?".to_string(), |s| s.source.clone());
            let source = format!("{}: {shown}", dish.recipe_name);

            match map.get_mut(&key) {
                None => {
                    order.push(key.clone());
                    map.insert(
                        key,
                        Acc {
                            item: GeneratedGroceryItem {
                                ingredient_name: ing.name.clone(),
                                total_quantity: qty.clone().unwrap_or_else(|| "—".into()),
                                store_section: ing
                                    .store_section
                                    .clone()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| "Other".into()),
                                source_dishes: vec![source],
                                no_data: None,
                            },
                            parsed,
                        },
                    );
                }
                Some(acc) => {
                    acc.item.source_dishes.push(source);
                    // DI12: sum when units match; fall back to "x + y" for non-numeric
                    match (&qty, &parsed, &mut acc.parsed) {
                        (Some(_), Some((p, _)), Some((total, u))) => {
                            total.value += p.value;
                            acc.item.total_quantity = shown_total(total.value, u);
                        }
                        (Some(q), _, _) if acc.item.total_quantity != "—" => {
                            acc.item.total_quantity = format!("{} + {q}", acc.item.total_quantity);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let mut items: Vec<GeneratedGroceryItem> = order
        .into_iter()
        .filter_map(|k| map.remove(&k).map(|a| a.item))
        .collect();
    let rank = |s: &str| {
        STORE_SECTION_ORDER
            .iter()
            .position(|x| *x == s)
            .unwrap_or(99)
    };
    items.sort_by_key(|i| rank(&i.store_section));
    Ok(items)
}

fn session_not_found() -> AppError {
    AppError::not_found_msg("Session not found")
}

/// The session's serving size if the account owns it.
async fn owned_session(t: &mut Tenant, raw_id: &str) -> Result<(i64, Option<i32>), AppError> {
    let id = parse_id(raw_id).ok_or_else(session_not_found)?;
    let row: Option<(Option<i32>,)> =
        sqlx::query_as("SELECT serving_size FROM sessions WHERE id = $1 AND account_id = $2")
            .bind(id)
            .bind(t.account.id)
            .fetch_optional(&mut *t.tx)
            .await?;
    let (serving,) = row.ok_or_else(session_not_found)?;
    Ok((id, serving))
}

async fn stored(conn: &mut PgConnection, id: i64) -> Result<Vec<GroceryItemRow>, sqlx::Error> {
    sqlx::query_as::<_, GroceryItemRow>(
        "SELECT * FROM grocery_items WHERE session_id = $1 ORDER BY id ASC",
    )
    .bind(id)
    .fetch_all(conn)
    .await
}

async fn list(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<GroceryListResponse>> {
    let (id, serving) = owned_session(&mut t, &raw_id).await?;
    let items = stored(&mut t.tx, id).await?;
    let res = if !items.is_empty() {
        GroceryListResponse::Stored {
            items,
            session_id: raw_id,
        }
    } else {
        GroceryListResponse::Generated {
            items: generate_grocery_list(&mut t.tx, id, serving).await?,
            session_id: raw_id,
            generated: true,
        }
    };
    t.tx.commit().await?;
    Ok(Json(res))
}

/// The stored list (or one generated on the fly, like GET .../grocery) as a PDF.
async fn grocery_pdf(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> DbResult<Response> {
    pdf::check_limit(&state, t.account.id).await?;
    let (id, serving) = owned_session(&mut t, &raw_id).await?;
    let items = stored(&mut t.tx, id).await?;
    let lines: Vec<docs::GroceryLine> = if items.is_empty() {
        generate_grocery_list(&mut t.tx, id, serving)
            .await?
            .into_iter()
            .map(Into::into)
            .collect()
    } else {
        items.into_iter().map(Into::into).collect()
    };
    let session = crate::routes::sessions::session_with_details(&mut t.tx, id)
        .await?
        .ok_or_else(session_not_found)?;
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    let bytes = pdf::render_blocking(
        Template::Grocery,
        docs::grocery(&session, lines, &settings),
        vec![],
    )
    .await?;
    Ok(pdf::response(bytes, &docs::grocery_filename(&session)))
}

async fn generate(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> DbResult<Json<GroceryListResponse>> {
    let (id, serving) = owned_session(&mut t, &raw_id).await?;
    sqlx::query("DELETE FROM grocery_items WHERE session_id = $1")
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
    for item in generate_grocery_list(&mut t.tx, id, serving).await? {
        sqlx::query(
            "INSERT INTO grocery_items (session_id, ingredient_name, total_quantity, store_section, source_dishes)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(&item.ingredient_name)
        .bind(&item.total_quantity)
        .bind(&item.store_section)
        .bind(json!(item.source_dishes))
        .execute(&mut *t.tx)
        .await?;
    }
    let items = stored(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(GroceryListResponse::Stored {
        items,
        session_id: raw_id,
    }))
}

async fn toggle(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<GroceryItemRow>> {
    let not_found = || AppError::not_found_msg("Item not found");
    let id = parse_id(&raw_id).ok_or_else(not_found)?;
    // Grocery items are scoped through their parent session
    let item = sqlx::query_as::<_, GroceryItemRow>(
        "SELECT g.* FROM grocery_items g JOIN sessions s ON s.id = g.session_id
         WHERE g.id = $1 AND s.account_id = $2",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(not_found)?;
    let checked = match body.opt_flag("is_checked") {
        Some(v) => v,
        None => !item.is_checked.unwrap_or(false),
    };
    sqlx::query("UPDATE grocery_items SET is_checked = $1 WHERE id = $2")
        .bind(checked)
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(GroceryItemRow {
        is_checked: Some(checked),
        ..item
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_from_the_recipe_base_to_any_serving_count() {
        let total = |q, base, n| scaled_qty(Some(q), None, base, n).unwrap().total;
        assert_eq!(total("2 cups", 4, 1), "1/2 cup");
        assert_eq!(total("2 cups", 4, 2), "1 cup");
        assert_eq!(total("2 cups", 4, 3), "1 1/2 cups");
        assert_eq!(total("2 cups", 4, 5), "2 1/2 cups");
        assert_eq!(total("3 can", 6, 6), "3 cans");
        assert_eq!(total("3 can", 6, 2), "1 can");
        assert_eq!(total("1 tbsp", 2, 3), "1 1/2 tbsp");
        // No base on the recipe: 4, as the schema's default.
        assert_eq!(total("2 cups", 0, 2), "1 cup");
        // Unscaled: the source line keeps the recipe's wording.
        let same = scaled_qty(Some("1.5 Tablespoons"), None, 4, 4).unwrap();
        assert_eq!(same.source, "1.5 Tablespoons");
        assert_eq!(same.total, "1 1/2 tbsp");
        // Not a number: as written, marked when it couldn't be scaled.
        assert_eq!(total("to taste", 4, 2), "to taste");
        assert_eq!(total("2-3 cloves", 4, 2), "1-1 1/2 cloves");
        assert_eq!(total("2-3 cloves", 4, 6), "3-4 1/2 cloves");
        assert_eq!(total("2-3 cloves", 4, 4), "2-3 cloves");
        assert_eq!(
            total("some parsley", 4, 2),
            "some parsley (as written for 4)"
        );
        assert_eq!(total("some parsley", 4, 4), "some parsley");
        assert!(scaled_qty(None, None, 4, 2).is_none());
        assert!(scaled_qty(Some("  "), None, 4, 2).is_none());
    }

    #[test]
    fn old_six_serving_amount_is_only_a_fallback() {
        // Base amount wins over the old fixed 6-serving column.
        let both = scaled_qty(Some("2 cups"), Some("9 cups"), 4, 6).unwrap();
        assert_eq!(both.total, "3 cups");
        // Only the 6-serving column: read as written for 1.5 × base.
        let only6 = scaled_qty(None, Some("3 cups"), 4, 2).unwrap();
        assert_eq!(only6.total, "1 cup");
    }

    #[test]
    fn parses_quantities_like_node() {
        let p = |s| parse_qty_simple(Some(s)).map(|(q, u)| (q.value, u));
        assert_eq!(p("1 1/2 cups"), Some((1.5, "cups".into())));
        assert_eq!(p("1/2 cup"), Some((0.5, "cup".into())));
        assert_eq!(p("2.5 Cups"), Some((2.5, "cups".into())));
        assert_eq!(p("3"), Some((3.0, String::new())));
        assert_eq!(p("3lb"), Some((3.0, "lb".into())));
        assert_eq!(p("to taste"), None);
        assert_eq!(p("1/0 cup"), None);
        assert_eq!(p("—"), None);
        assert_eq!(p("pinch"), None);
    }

    #[test]
    fn shows_every_amount_in_one_form() {
        assert_eq!(shown_total(1.5, "cups"), "1 1/2 cups");
        assert_eq!(shown_total(1.5, "Tablespoons"), "1 1/2 tbsp");
        assert_eq!(shown_total(0.5, "cups"), "1/2 cup");
        assert_eq!(shown_total(2.0, "cup"), "2 cups");
        assert_eq!(shown_total(4.0, "lb"), "4 lb");
        assert_eq!(shown_total(3.0, ""), "3");
        assert_eq!(shown_total(2.0, "large"), "2 large");
    }
}
