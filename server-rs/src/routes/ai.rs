// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/ai.js`.
//!
//! POST   /api/ai/suggest-menu — the AI picks dishes for a client (30 / 15 min per account)
//! GET    /api/ai/status       — this account's AI setup (never the key itself)
//! PUT    /api/ai/settings     — choose a provider, model, base URL and key
//! DELETE /api/ai/settings     — forget them (back to the server's key, if any)
//! POST   /api/ai/test         — ping the provider (same limit)
//!
//! Each account uses its own provider and key; see `crate::llm`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use crate::db::{Tenant, Tx};
use crate::error::{AppError, err_msg};
use crate::js;
use crate::llm::{self, AiError, AiSettings, Provider, ProviderInfo, Purpose};
use crate::models::{Client, RecipeRow};
use crate::rate_limit::AccountLimits;
use crate::routes::{JsonBody, body_id};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/suggest-menu", post(suggest_menu))
        .route("/status", get(status))
        .route("/settings", put(put_settings).delete(delete_settings))
        .route("/test", post(test))
}

/// 503 `API_KEY_MISSING` for an account whose AI isn't usable yet.
pub fn setup_error(e: AiError) -> AppError {
    AppError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "API_KEY_MISSING",
        e.to_string(),
    )
}

/// The AI this account will use, read on its tenant transaction.
pub async fn account_ai(
    state: &AppState,
    tx: &mut Tx,
    account_id: i64,
) -> Result<llm::AiConfig, AppError> {
    let settings = llm::load_settings(tx, account_id).await?;
    llm::resolve(state, account_id, &settings).map_err(setup_error)
}

async fn check_ai_limit(state: &AppState, account_id: i64) -> Result<(), AppError> {
    state
        .account_limits
        .ai
        .check(&state.limits_pool, &AccountLimits::key(account_id))
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
    let ai = account_ai(&state, &mut t.tx, account).await?;

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

    let text = match llm::message(&state, &ai, Purpose::Task, 1024, Some(&system), &user).await {
        Ok(Some(text)) if !text.is_empty() => text,
        Ok(_) => {
            return Err(AppError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PARSE_ERROR",
                "Empty AI response",
            ));
        }
        Err(AiError::Timeout) => {
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
    let mut result: Value = serde_json::from_str(llm::json_text(&text))
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

/// GET /api/ai/status (and the PUT/DELETE responses).
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct AiStatus {
    /// AI features work for this account.
    pub configured: bool,
    /// `"account"` (the chef's own settings), `"server"` (the server's
    /// key) or `null` (neither).
    pub source: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    /// Last four characters of the saved key.
    pub key_hint: Option<String>,
    /// A key is saved but can't be decrypted (the server's `JWT_SECRET`
    /// changed); the chef needs to enter it again.
    pub key_unreadable: bool,
    /// The server has its own key that accounts without settings use.
    pub server_key: bool,
    /// Private and plain-HTTP base URLs are allowed (`AI_ALLOW_PRIVATE_URLS`).
    pub allow_private_urls: bool,
    pub providers: Vec<ProviderInfo>,
}

fn status_of(state: &AppState, account_id: i64, s: &AiSettings) -> AiStatus {
    let resolved = llm::resolve(state, account_id, s);
    AiStatus {
        configured: resolved.is_ok(),
        source: match &resolved {
            Ok(ai) if !ai.own => Some("server".into()),
            _ if s.ai_provider.is_some() => Some("account".into()),
            _ => None,
        },
        provider: s.ai_provider.clone(),
        model: s.ai_model.clone(),
        base_url: s.ai_base_url.clone(),
        key_hint: s.ai_key_hint.clone(),
        key_unreadable: matches!(resolved, Err(AiError::KeyUnreadable)),
        server_key: state.config.anthropic_api_key.is_some(),
        allow_private_urls: state.config.ai_allow_private_urls,
        providers: llm::provider_list(),
    }
}

async fn status(State(state): State<AppState>, mut t: Tenant) -> Result<Json<AiStatus>, AppError> {
    let settings = llm::load_settings(&mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    Ok(Json(status_of(&state, t.account.id, &settings)))
}

const MAX_MODEL_CHARS: usize = 200;
const MAX_KEY_CHARS: usize = 500;

/// PUT /api/ai/settings: `{ provider, model?, base_url?, api_key? }`.
/// `api_key` absent or empty keeps the saved key, unless the provider
/// changes: a key belongs to one provider, so a new one is then required
/// (Custom may run without one).
async fn put_settings(
    State(state): State<AppState>,
    mut t: Tenant,
    body: JsonBody,
) -> Result<Json<AiStatus>, AppError> {
    let id = t.account.id;
    crate::demo::refuse(&state, id, "Saving an AI key")?;
    let provider = body
        .trimmed_or_none("provider")
        .and_then(|p| Provider::parse(&p))
        .ok_or_else(|| AppError::validation("Choose an AI provider"))?;
    let model = body.trimmed_or_none("model");
    if model
        .as_ref()
        .is_some_and(|m| m.chars().count() > MAX_MODEL_CHARS)
    {
        return Err(AppError::validation(format!(
            "Model must be {MAX_MODEL_CHARS} characters or fewer"
        )));
    }
    if model.is_none() && provider.needs_model() {
        return Err(AppError::validation(format!(
            "Enter the model to use with {}",
            provider.label()
        )));
    }
    let base_url = match body.trimmed_or_none("base_url") {
        Some(raw) => Some(
            llm::check_base_url(&raw, state.config.ai_allow_private_urls)
                .map_err(AppError::validation)?,
        ),
        None if provider.default_base_url().is_none() => {
            return Err(AppError::validation(format!(
                "Enter the base URL for {}",
                provider.label()
            )));
        }
        None => None,
    };
    let new_key = body.trimmed_or_none("api_key");
    if new_key
        .as_ref()
        .is_some_and(|k| k.chars().count() > MAX_KEY_CHARS)
    {
        return Err(AppError::validation("API key is too long"));
    }

    let current = llm::load_settings(&mut t.tx, id).await?;
    let same_provider = current.ai_provider.as_deref() == Some(provider.id());
    let (key_enc, key_hint) = match new_key {
        Some(key) => (
            Some(crate::ai_keys::encrypt(&state.config.jwt_secret, id, &key)),
            Some(crate::ai_keys::hint(&key)),
        ),
        None if same_provider => (current.ai_api_key_enc, current.ai_key_hint),
        None => (None, None),
    };
    if key_enc.is_none() && provider.needs_key() {
        return Err(AppError::validation(format!(
            "Enter your {} API key",
            provider.label()
        )));
    }

    sqlx::query(
        "UPDATE settings SET ai_provider = $1, ai_model = $2, ai_base_url = $3,
                ai_api_key_enc = $4, ai_key_hint = $5, updated_at = NOW()
         WHERE account_id = $6",
    )
    .bind(provider.id())
    .bind(&model)
    .bind(&base_url)
    .bind(&key_enc)
    .bind(&key_hint)
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    let saved = llm::load_settings(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(status_of(&state, id, &saved)))
}

/// DELETE /api/ai/settings: forget the provider, model, base URL and key.
async fn delete_settings(
    State(state): State<AppState>,
    mut t: Tenant,
) -> Result<Json<AiStatus>, AppError> {
    let id = t.account.id;
    sqlx::query(
        "UPDATE settings SET ai_provider = NULL, ai_model = NULL, ai_base_url = NULL,
                ai_api_key_enc = NULL, ai_key_hint = NULL, updated_at = NOW()
         WHERE account_id = $1",
    )
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(status_of(&state, id, &AiSettings::default())))
}

async fn test(State(state): State<AppState>, mut t: Tenant) -> Result<Response, AppError> {
    let id = t.account.id;
    check_ai_limit(&state, id).await?;
    let settings = llm::load_settings(&mut t.tx, id).await?;
    t.tx.commit().await?;
    let ai = llm::resolve(&state, id, &settings)
        .map_err(|e| setup_error(e).with("configured", false))?;
    match llm::message(&state, &ai, Purpose::Test, 10, None, "ping").await {
        Ok(_) => Ok(Json(json!({ "configured": true, "working": true })).into_response()),
        Err(e) => Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "configured": true, "working": false, "error": e.to_string() })),
        )
            .into_response()),
    }
}
