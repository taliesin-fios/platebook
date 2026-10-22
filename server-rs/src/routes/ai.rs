// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/ai.js`.
//!
//! POST /api/ai/suggest-menu — Claude picks dishes for a client (30 / 15 min per account)
//! GET  /api/ai/status       — is an API key configured?
//! POST /api/ai/test         — ping the API (same limit)

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde_json::{Value, json};

use crate::anthropic::{self, ClaudeError};
use crate::auth::Account;
use crate::db::Tenant;
use crate::error::{AppError, err_msg};
use crate::js;
use crate::models::{Client, RecipeRow};
use crate::rate_limit::AccountLimits;
use crate::routes::{JsonBody, body_id};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/suggest-menu", post(suggest_menu))
        .route("/status", get(status))
        .route("/test", post(test))
}

async fn check_ai_limit(state: &AppState, account_id: i64) -> Result<(), AppError> {
    state
        .account_limits
        .ai
        .check(&state.pool, &AccountLimits::key(account_id))
        .await
        .map_err(|l| l.with_code())
}

/// `${value}` for a nullable column, as a JS template literal prints it.
fn show<T: ToString>(v: &Option<T>) -> String {
    v.as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "null".into())
}

fn list(v: &Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items.iter().map(js::string).collect(),
        _ => vec![],
    }
}

fn or_text(items: Vec<String>, fallback: &str) -> String {
    if items.is_empty() {
        fallback.to_string()
    } else {
        items.join(", ")
    }
}

#[derive(sqlx::FromRow, Default)]
struct Biz {
    business_name: Option<String>,
    location: Option<String>,
    default_session_window: Option<Decimal>,
}

async fn suggest_menu(
    State(state): State<AppState>,
    mut t: Tenant,
    body: JsonBody,
) -> Result<Response, AppError> {
    let account = t.account.id;
    check_ai_limit(&state, account).await?;
    if state.config.anthropic_api_key.is_none() {
        return Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "API_KEY_MISSING",
            "Anthropic API key not configured",
        ));
    }

    let ai_error = |e: &dyn std::fmt::Display| {
        AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "AI_ERROR", err_msg(e))
    };
    let client =
        sqlx::query_as::<_, Client>("SELECT * FROM clients WHERE id = $1 AND account_id = $2")
            .bind(body_id(body.get("client_id")))
            .bind(account)
            .fetch_optional(&mut *t.tx)
            .await
            .map_err(|e| ai_error(&e))?
            .ok_or_else(|| AppError::not_found_msg("Client not found"))?;
    let recipes = sqlx::query_as::<_, RecipeRow>(
        "SELECT * FROM recipes WHERE account_id = $1 AND is_active ORDER BY id",
    )
    .bind(account)
    .fetch_all(&mut *t.tx)
    .await
    .map_err(|e| ai_error(&e))?;
    let biz = sqlx::query_as::<_, Biz>(
        "SELECT business_name, location, default_session_window FROM settings WHERE account_id = $1",
    )
    .bind(account)
    .fetch_optional(&mut *t.tx)
    .await
    .map_err(|e| ai_error(&e))?
    .unwrap_or_default();
    // Release the connection before the (slow) API call.
    t.tx.commit().await.map_err(|e| ai_error(&e))?;

    let window = body
        .get("session_window_hours")
        .map(js::number)
        .filter(|n| *n != 0.0 && n.is_finite())
        .and_then(js::decimal)
        .or(biz.default_session_window.filter(|d| !d.is_zero()))
        .unwrap_or(Decimal::new(3, 0));
    let biz_name = biz
        .business_name
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "your chef business".into());
    let location = biz
        .location
        .filter(|l| !l.is_empty())
        .map(|l| format!(" in {l}"))
        .unwrap_or_default();

    let system = format!(
        r#"You are the menu planning assistant for {biz_name}, a professional personal chef business cooking in clients' homes{location}. Select the best combination of dishes for a single session.
Rules:
- NEVER suggest a dish that contains a client-flagged allergen
- Prioritize Session-Friendly dishes that fit within the session window
- Maximize ingredient overlap to reduce grocery cost and waste
- Aim for variety: different proteins, flavor profiles, textures
- Return ONLY this JSON: {{ "suggested_recipe_ids": [n, n, n], "reasoning": "string" }}
- Never include any text outside the JSON object"#
    );

    let recipe_list = recipes
        .iter()
        .map(|r| {
            format!(
                "{} | {} | {} | {} | {} min | {}",
                r.id,
                r.name,
                show(&r.category),
                show(&r.session_fit),
                r.total_time(),
                list(&r.key_overlap_ingredients).join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let serving = match body.get("serving_size").filter(|v| js::truthy(v)) {
        Some(v) => js::string(v),
        None => show(&client.serving_size),
    };
    let dish_count = match body.get("dish_count") {
        None => "3".to_string(),
        Some(v) => js::string(v),
    };
    let user = format!(
        "Client: {}, {} adults, {} children, serving size {serving}.
Dietary style: {}.
Allergens to EXCLUDE completely: {}.
Liked proteins: {}.
Spice tolerance: {}.
Foods to avoid: {}.
Session window: {} hours.
Suggest {dish_count} dishes from the recipe list below. They must work well together, avoid all flagged allergens, and share ingredients where possible.
Recipe list:
{recipe_list}",
        show(&client.household_type),
        show(&client.adults),
        show(&client.children),
        or_text(list(&client.dietary_styles), "None specified"),
        or_text(list(&client.allergens), "None"),
        or_text(list(&client.liked_proteins), "None specified"),
        client
            .spice_tolerance
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "No preference".into()),
        client
            .disliked_foods
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "none stated".into()),
        window.normalize(),
    );

    let text = match anthropic::message(
        &state,
        anthropic::DEFAULT_MODEL,
        1024,
        Some(&system),
        &user,
    )
    .await
    {
        Ok(Some(text)) if !text.is_empty() => text,
        Ok(_) => {
            return Err(AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PARSE_ERROR",
                "Empty AI response",
            ));
        }
        Err(ClaudeError::Timeout) => {
            return Err(AppError::new(
                StatusCode::REQUEST_TIMEOUT,
                "TIMEOUT",
                "AI took too long — try again or select dishes manually",
            ));
        }
        Err(e) => {
            tracing::error!(error = %e, "[ai] suggest-menu");
            return Err(ai_error(&e));
        }
    };
    let parse_error = |m: &str| AppError::new(StatusCode::UNPROCESSABLE_ENTITY, "PARSE_ERROR", m);
    let mut result: Value = serde_json::from_str(text.trim())
        .map_err(|_| parse_error("Failed to parse AI response"))?;
    // DI16: validate the response shape, then keep only real recipe ids
    let Some(Value::Array(ids)) = result.get("suggested_recipe_ids") else {
        return Err(parse_error("AI returned unexpected response shape"));
    };
    let valid: Vec<Value> = ids
        .iter()
        .filter(|v| {
            v.as_i64()
                .is_some_and(|id| recipes.iter().any(|r| r.id == id))
        })
        .cloned()
        .collect();
    result["suggested_recipe_ids"] = Value::Array(valid);
    Ok(Json(result).into_response())
}

async fn status(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "configured": state.config.anthropic_api_key.is_some() }))
}

async fn test(State(state): State<AppState>, account: Account) -> Result<Response, AppError> {
    check_ai_limit(&state, account.id).await?;
    if state.config.anthropic_api_key.is_none() {
        return Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "API_KEY_MISSING",
            "API key not configured",
        )
        .with("configured", false));
    }
    match anthropic::message(&state, anthropic::TEST_MODEL, 10, None, "ping").await {
        Ok(_) => Ok(Json(json!({ "configured": true, "working": true })).into_response()),
        Err(e) => Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "configured": true, "working": false, "error": e.to_string() })),
        )
            .into_response()),
    }
}
