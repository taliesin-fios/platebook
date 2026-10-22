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
static TRAILING_ZEROS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.?0+$").expect("regex"));

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

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// `n.toFixed(2)` with trailing zeros (and a bare dot) removed.
fn fixed2_trimmed(n: f64) -> String {
    let s = format!("{n:.2}");
    TRAILING_ZEROS.replace(&s, "").into_owned()
}

/// A JS number as `String(n)` prints a whole number.
fn js_int(n: f64) -> String {
    format!("{}", n as i64)
}

/// Round to the nearest 1/8 and print as "1 1/2 cups".
fn format_qty_simple(value: f64, unit: &str) -> String {
    let rounded = js_round(value * 8.0) / 8.0;
    let whole = rounded.floor();
    let frac = js_round((rounded - whole) * 8.0) as i64;
    let frac_str = match frac {
        1 => "1/8",
        2 => "1/4",
        3 => "3/8",
        4 => "1/2",
        5 => "5/8",
        6 => "3/4",
        7 => "7/8",
        _ => "",
    };
    let num = if whole > 0.0 && !frac_str.is_empty() {
        format!("{} {frac_str}", js_int(whole))
    } else if whole > 0.0 {
        js_int(whole)
    } else if !frac_str.is_empty() {
        frac_str.to_string()
    } else {
        fixed2_trimmed(rounded)
    };
    if unit.is_empty() {
        num
    } else {
        format!("{num} {unit}")
    }
}

struct Acc {
    item: GeneratedGroceryItem,
    parsed: Option<(Qty, String)>,
}

#[derive(sqlx::FromRow)]
struct Dish {
    recipe_name: String,
    recipe_id: i64,
}

/// Port of `generateGroceryList(db, sessionId, servingSize)`.
pub async fn generate_grocery_list(
    conn: &mut PgConnection,
    session_id: i64,
    serving_size: Option<i32>,
) -> Result<Vec<GeneratedGroceryItem>, sqlx::Error> {
    let dishes = sqlx::query_as::<_, Dish>(
        "SELECT r.name AS recipe_name, r.id AS recipe_id
         FROM session_dishes sd JOIN recipes r ON r.id = sd.recipe_id
         WHERE sd.session_id = $1 ORDER BY sd.sort_order, sd.id",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await?;

    // Insertion-ordered map, like the JS object it replaces.
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Acc> = HashMap::new();
    let large = serving_size.is_some_and(|s| s >= 5);

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
            let qty = if large {
                ing.qty_6_serving
            } else {
                ing.qty_4_serving
            }
            .filter(|q| !q.is_empty());
            let parsed = parse_qty_simple(qty.as_deref());
            // DI13: dedup on name + unit so "butter (cups)" and "butter (tbsp)" stay apart
            let unit = parsed.as_ref().map(|(_, u)| u.clone()).unwrap_or_default();
            let key = format!("{}|{unit}", ing.name.to_lowercase().trim());
            let shown = qty.clone().unwrap_or_else(|| "?".into());
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
                            acc.item.total_quantity = format_qty_simple(total.value, u);
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
    fn formats_eighths_like_node() {
        assert_eq!(format_qty_simple(1.5, "cups"), "1 1/2 cups");
        assert_eq!(format_qty_simple(0.25, "tsp"), "1/4 tsp");
        assert_eq!(format_qty_simple(4.0, "lb"), "4 lb");
        assert_eq!(format_qty_simple(3.0, ""), "3");
        assert_eq!(format_qty_simple(0.01, "g"), "0 g");
        assert_eq!(format_qty_simple(0.99, ""), "1");
    }
}
