// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/recipes.js`.
//!
//! GET    /api/recipes              — list (category, session_fit, search, active)
//! GET    /api/recipes/:id          — one recipe with ingredients
//! POST   /api/recipes              — create (+ ingredients)
//! PUT    /api/recipes/:id          — update (COALESCE semantics, + replace ingredients)
//! DELETE /api/recipes/:id          — soft delete, blocked while used by a session
//! POST   /api/recipes/import/url   — schema.org JSON-LD import (20 / 15 min per account)
//! POST   /api/recipes/import/text  — Claude-parsed import (same limit)

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgConnection, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::anthropic;
use crate::auth::Account;
use crate::db::Tenant;
use crate::error::{AppError, DbResult};
use crate::js;
use crate::models::{Ingredient, Recipe, RecipeRow, SUCCESS, Success};
use crate::rate_limit::AccountLimits;
use crate::recipe_import::{self, ParsedRecipe, confidence_level};
use crate::routes::{JsonBody, json_or_empty, parse_id};
use crate::safe_fetch::{self, FetchError};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/import/url", post(import_url))
        .route("/import/text", post(import_text))
}

/// POST /api/recipes.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecipeWithIngredients {
    #[serde(flatten)]
    #[ts(flatten)]
    pub recipe: RecipeRow,
    pub ingredients: Vec<Ingredient>,
}

/// GET /api/recipes/:id and PUT /api/recipes/:id.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecipeDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub recipe: RecipeRow,
    pub total_time: i32,
    pub ingredients: Vec<Ingredient>,
}

fn not_found() -> AppError {
    AppError::not_found_msg("Recipe not found")
}

async fn list(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Json<Vec<Recipe>>> {
    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT * FROM recipes WHERE account_id = ");
    qb.push_bind(t.account.id);
    if q.get("active").map(String::as_str) != Some("false") {
        qb.push(" AND is_active");
    }
    if let Some(c) = q.get("category").filter(|c| !c.is_empty() && *c != "All") {
        qb.push(" AND category = ").push_bind(c.clone());
    }
    if let Some(f) = q
        .get("session_fit")
        .filter(|f| !f.is_empty() && *f != "All")
    {
        qb.push(" AND session_fit = ").push_bind(f.clone());
    }
    if let Some(s) = q.get("search").filter(|s| !s.is_empty()) {
        // SQLite LIKE was case-insensitive for ASCII
        qb.push(" AND name ILIKE ").push_bind(format!("%{s}%"));
    }
    qb.push(r#" ORDER BY name COLLATE "C" ASC, id"#);
    let rows = qb
        .build_query_as::<RecipeRow>()
        .fetch_all(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(rows.into_iter().map(Recipe::from).collect()))
}

async fn ingredients_of(conn: &mut PgConnection, id: i64) -> Result<Vec<Ingredient>, sqlx::Error> {
    sqlx::query_as::<_, Ingredient>(
        "SELECT * FROM ingredients WHERE recipe_id = $1 ORDER BY id ASC",
    )
    .bind(id)
    .fetch_all(conn)
    .await
}

async fn owned_recipe(t: &mut Tenant, raw_id: &str) -> Result<RecipeRow, AppError> {
    let id = parse_id(raw_id).ok_or_else(not_found)?;
    sqlx::query_as::<_, RecipeRow>("SELECT * FROM recipes WHERE id = $1 AND account_id = $2")
        .bind(id)
        .bind(t.account.id)
        .fetch_optional(&mut *t.tx)
        .await?
        .ok_or_else(not_found)
}

async fn detail(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<RecipeDetail>> {
    let recipe = owned_recipe(&mut t, &raw_id).await?;
    let ingredients = ingredients_of(&mut t.tx, recipe.id).await?;
    t.tx.commit().await?;
    Ok(Json(RecipeDetail {
        total_time: recipe.total_time(),
        recipe,
        ingredients,
    }))
}

/// Inserts the named ingredients (blank names skipped).
async fn insert_ingredients(
    conn: &mut PgConnection,
    recipe_id: i64,
    list: &[Value],
) -> Result<(), sqlx::Error> {
    for ing in list {
        let name = ing
            .get("name")
            .filter(|v| js::truthy(v))
            .map(|v| js::string(v).trim().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let text = |k: &str| match ing.get(k) {
            None | Some(Value::Null) => None,
            Some(v) => Some(js::string(v)),
        };
        sqlx::query(
            "INSERT INTO ingredients (recipe_id, name, qty_4_serving, qty_6_serving, store_section, category)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(recipe_id)
        .bind(name)
        .bind(text("qty_4_serving"))
        .bind(text("qty_6_serving"))
        .bind(text("store_section"))
        .bind(text("category"))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn ingredient_list(body: &JsonBody) -> Option<Vec<Value>> {
    match body.get("ingredients") {
        Some(Value::Array(a)) => Some(a.clone()),
        _ => None,
    }
}

async fn create(
    mut t: Tenant,
    body: JsonBody,
) -> DbResult<(StatusCode, Json<RecipeWithIngredients>)> {
    if !body.truthy("name") {
        return Err(AppError::validation("Name is required").into());
    }
    let recipe = sqlx::query_as::<_, RecipeRow>(
        "INSERT INTO recipes (
           account_id, name, category, session_fit, prep_time_min, cook_time_min, servings_base,
           hold_quality, reheat_quality, key_overlap_ingredients, chef_notes,
           raw_instructions, import_source
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING *",
    )
    .bind(t.account.id)
    .bind(body.opt_text("name"))
    .bind(
        body.text_or_none("category")
            .unwrap_or_else(|| "Other".into()),
    )
    .bind(
        body.text_or_none("session_fit")
            .unwrap_or_else(|| "Session-Friendly".into()),
    )
    .bind(body.opt_i64("prep_time_min").map(|n| n as i32))
    .bind(body.opt_i64("cook_time_min").map(|n| n as i32))
    .bind(body.i64_or("servings_base", 4) as i32)
    .bind(body.opt_i64("hold_quality").map(|n| n as i32))
    .bind(body.opt_i64("reheat_quality").map(|n| n as i32))
    .bind(json_or_empty(body.get("key_overlap_ingredients")))
    .bind(body.opt_text("chef_notes"))
    .bind(body.opt_text("raw_instructions"))
    .bind(
        body.text_or_none("import_source")
            .unwrap_or_else(|| "manual".into()),
    )
    .fetch_one(&mut *t.tx)
    .await?;
    if let Some(list) = ingredient_list(&body) {
        insert_ingredients(&mut t.tx, recipe.id, &list).await?;
    }
    let ingredients = ingredients_of(&mut t.tx, recipe.id).await?;
    t.tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(RecipeWithIngredients {
            recipe,
            ingredients,
        }),
    ))
}

async fn update(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<RecipeDetail>> {
    let existing = owned_recipe(&mut t, &raw_id).await?;
    let id = existing.id;
    let overlap = body
        .get("key_overlap_ingredients")
        .filter(|v| js::truthy(v))
        .cloned();
    let recipe = sqlx::query_as::<_, RecipeRow>(
        "UPDATE recipes SET
           name = COALESCE($1, name),
           category = COALESCE($2, category),
           session_fit = COALESCE($3, session_fit),
           prep_time_min = COALESCE($4, prep_time_min),
           cook_time_min = COALESCE($5, cook_time_min),
           servings_base = COALESCE($6, servings_base),
           hold_quality = COALESCE($7, hold_quality),
           reheat_quality = COALESCE($8, reheat_quality),
           key_overlap_ingredients = COALESCE($9, key_overlap_ingredients),
           chef_notes = COALESCE($10, chef_notes),
           raw_instructions = COALESCE($11, raw_instructions)
         WHERE id = $12 AND account_id = $13 RETURNING *",
    )
    .bind(body.opt_text("name"))
    .bind(body.opt_text("category"))
    .bind(body.opt_text("session_fit"))
    .bind(body.opt_i64("prep_time_min").map(|n| n as i32))
    .bind(body.opt_i64("cook_time_min").map(|n| n as i32))
    .bind(body.opt_i64("servings_base").map(|n| n as i32))
    .bind(body.opt_i64("hold_quality").map(|n| n as i32))
    .bind(body.opt_i64("reheat_quality").map(|n| n as i32))
    .bind(overlap)
    .bind(body.opt_text("chef_notes"))
    .bind(body.opt_text("raw_instructions"))
    .bind(id)
    .bind(t.account.id)
    .fetch_one(&mut *t.tx)
    .await?;

    if body.truthy("ingredients") {
        sqlx::query("DELETE FROM ingredients WHERE recipe_id = $1")
            .bind(id)
            .execute(&mut *t.tx)
            .await?;
        if let Some(list) = ingredient_list(&body) {
            insert_ingredients(&mut t.tx, id, &list).await?;
        }
    }
    let ingredients = ingredients_of(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(RecipeDetail {
        total_time: recipe.total_time(),
        recipe,
        ingredients,
    }))
}

async fn remove(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<Success>> {
    let recipe = owned_recipe(&mut t, &raw_id).await?;
    // Block deletion while any session uses this recipe
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_dishes WHERE recipe_id = $1")
        .bind(recipe.id)
        .fetch_one(&mut *t.tx)
        .await?;
    if count > 0 {
        let plural = if count == 1 { "" } else { "s" };
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "RECIPE_IN_USE",
            format!(
                "This recipe is used in {count} session{plural} and cannot be deleted. Remove it from those sessions first."
            ),
        )
        .with("session_count", count)
        .into());
    }
    sqlx::query("UPDATE recipes SET is_active = false WHERE id = $1")
        .bind(recipe.id)
        .execute(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(SUCCESS))
}

/// POST /api/recipes/import/url and /import/text.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RecipeImportResponse {
    #[ts(as = "ParsedRecipe")]
    pub parsed: Value,
    #[ts(type = "number")]
    pub confidence: i64,
    #[ts(type = "'high' | 'medium' | 'low'")]
    pub confidence_level: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'json_ld'")]
    pub source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_url: Option<String>,
}

async fn check_import_limit(state: &AppState, account_id: i64) -> Result<(), AppError> {
    state
        .account_limits
        .import
        .check(&state.limits_pool, &AccountLimits::key(account_id))
        .await
        .map_err(|l| l.with_code())
}

async fn import_url(
    State(state): State<AppState>,
    account: Account,
    body: JsonBody,
) -> Result<Response, AppError> {
    check_import_limit(&state, account.id).await?;
    let url = body.opt_text("url").unwrap_or_default();
    if url.is_empty() || !url.starts_with("http") {
        return Err(AppError::validation("A valid URL is required"));
    }
    if safe_fetch::is_url_blocked(&url) {
        return Err(AppError::validation("That URL is not allowed"));
    }

    let html = match safe_fetch::fetch_html(&safe_fetch::client(), &url).await {
        Ok(h) => h,
        Err(FetchError::Disallowed) => {
            return Err(AppError::validation(
                "That URL redirects to a disallowed location",
            ));
        }
        Err(FetchError::Status(code)) => {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "FETCH_ERROR",
                format!("Could not fetch that URL (HTTP {code})"),
            ));
        }
        Err(FetchError::Timeout) => {
            return Err(AppError::new(
                StatusCode::REQUEST_TIMEOUT,
                "TIMEOUT",
                "Request timed out — the site took too long to respond",
            ));
        }
        Err(FetchError::TooLarge) => {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "FETCH_ERROR",
                "That page is too large to import",
            ));
        }
        Err(FetchError::Other(e)) => {
            tracing::error!(error = %e, "[import/url]");
            return Err(AppError::internal(e));
        }
    };

    let Some(recipe) = recipe_import::find_recipe(&html) else {
        return Err(AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "NO_RECIPE_FOUND",
            "No structured recipe data found on this page. The site may not support recipe import. Try copying the recipe text and using the Paste Text tab instead.",
        ));
    };
    let (parsed, confidence, level) = recipe_import::map_recipe(&recipe);
    Ok(Json(RecipeImportResponse {
        parsed: serde_json::to_value(parsed).map_err(AppError::internal)?,
        confidence,
        confidence_level: level,
        source: Some("json_ld"),
        source_url: Some(url),
    })
    .into_response())
}

const RECIPE_SYSTEM_PROMPT: &str = r#"You are a recipe parser for a professional personal chef's weekly meal prep business. Extract structured recipe data from text that may be a blog recipe, a cookbook transcription, a restaurant-style recipe, or a rough conversational description from memory.
Return ONLY a valid JSON object — no explanation, no markdown, no extra text.
Required JSON structure:
{
  "name": "string or null",
  "category": "Couples | Families | Plant-Based | Other",
  "session_fit": "Session-Friendly | Long-Cook",
  "prep_time_min": "integer or null",
  "cook_time_min": "integer or null",
  "servings_base": "integer (default 4)",
  "hold_quality": "integer 1-5",
  "reheat_quality": "integer 1-5",
  "key_overlap_ingredients": ["string[]"],
  "chef_notes": "string or null",
  "raw_instructions": "string or null",
  "ingredients": [
    {
      "name": "string",
      "qty_4_serving": "string or null",
      "qty_6_serving": "string or null",
      "store_section": "string",
      "category": "string"
    }
  ]
}
Rules:
- session_fit: use Long-Cook if total active+passive time exceeds 90 minutes
- hold_quality estimates by type: braised meats/stews/curries/bean dishes=5, meatballs in sauce/baked pasta=4-5, roasted vegetables/grain bowls=3-4, fresh salads/raw dishes=1-2
- reheat_quality: same logic as hold_quality
- key_overlap_ingredients: only core pantry/aromatic staples (garlic, onion, lemon, tomato, olive oil, stock, cream, herbs, yogurt, etc.). Never include specialty or single-use ingredients
- store_section: exactly one of: Produce, Meat & Seafood, Dairy & Eggs, Pantry & Dry Goods, Canned & Jarred, Herbs & Spices, Other
- qty_6_serving: multiply qty_4_serving by 1.5 and round to clean measurement
- If text is not a recipe, return: {"error": "No recipe found"}
- Never invent ingredients not mentioned or clearly implied by the dish name"#;

const MAX_TEXT_BYTES: usize = 50_000;

async fn import_text(
    State(state): State<AppState>,
    account: Account,
    body: JsonBody,
) -> Result<Response, AppError> {
    check_import_limit(&state, account.id).await?;
    let raw = body.opt_text("raw_text").unwrap_or_default();
    if raw.encode_utf16().count() < 20 {
        return Err(AppError::validation("Text too short to parse"));
    }
    if raw.len() > MAX_TEXT_BYTES {
        return Err(AppError::validation(
            "Text too long — please trim to under 50 KB",
        ));
    }
    if state.config.anthropic_api_key.is_none() {
        return Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "API_KEY_MISSING",
            "Anthropic API key not configured",
        ));
    }

    let text = match anthropic::message(
        &state,
        anthropic::DEFAULT_MODEL,
        4096,
        Some(RECIPE_SYSTEM_PROMPT),
        &raw,
    )
    .await
    {
        Ok(text) => text.unwrap_or_default(),
        Err(e) if e.status() == Some(401) => {
            return Err(AppError::new(
                StatusCode::UNAUTHORIZED,
                "AUTH_ERROR",
                "Invalid API key",
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "[import/text]");
            return Err(AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AI_ERROR",
                crate::error::err_msg(&e),
            ));
        }
    };
    let Ok(parsed) = serde_json::from_str::<Value>(text.trim()) else {
        return Err(AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "PARSE_ERROR",
            "Failed to parse AI response",
        ));
    };
    if let Some(err) = parsed.get("error").filter(|e| js::truthy(e)) {
        return Err(AppError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "NO_RECIPE_FOUND",
            js::string(err),
        ));
    }

    let ingredient_count = parsed
        .get("ingredients")
        .and_then(Value::as_array)
        .map(Vec::len);
    let mut score = 100;
    if !parsed.get("name").is_some_and(js::truthy) {
        score -= 25;
    }
    if ingredient_count.unwrap_or(0) < 2 {
        score -= 30;
    }
    if !parsed.get("prep_time_min").is_some_and(js::truthy)
        && !parsed.get("cook_time_min").is_some_and(js::truthy)
    {
        score -= 15;
    }
    if ingredient_count.is_some_and(|n| n < 5) {
        score -= 10;
    }
    Ok(Json(RecipeImportResponse {
        parsed,
        confidence: score,
        confidence_level: confidence_level(score),
        source: None,
        source_url: None,
    })
    .into_response())
}
