// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/settings.js`.
//!
//! GET    /api/settings              — this account's settings + account meta
//! PUT    /api/settings              — partial update (only fields present change)
//! GET    /api/settings/public       — SEC6 legacy: account #1's public profile
//! GET    /api/settings/export       — this account's data as JSON
//! DELETE /api/settings/clear-sample — remove the demo client and its history

use std::sync::LazyLock;

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use ts_rs::TS;

use crate::accounts::{self, Settings};
use crate::db::{Tenant, Tx};
use crate::error::{AppError, DbResult};
use crate::js;
use crate::models::{
    Client, GroceryItemRow, Ingredient, IntakeRow, InvoiceLineItem, InvoiceRow, RecipeRow,
    SessionRow, SessionTemplateRow,
};
use crate::routes::JsonBody;
use crate::routes::public::{PublicProfile, profile_for};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_settings).put(put_settings))
        .route("/public", get(legacy_public))
        .route("/export", get(export))
        .route("/clear-sample", delete(clear_sample))
}

pub static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").expect("valid regex"));

/// Field rules for PUT /api/settings. Anything not listed is ignored.
const TEXT_FIELDS: &[(&str, usize)] = &[
    ("business_name", 120),
    ("chef_name", 120),
    ("phone", 40),
    ("email", 254),
    ("website", 300),
    ("location", 120),
    ("payment_methods", 300),
    ("booking_terms", 4000),
    ("response_time_text", 120),
];

#[derive(Clone, Copy)]
enum Num {
    Decimal,
    Int,
    BigInt,
}

const NUMBER_FIELDS: &[(&str, f64, f64, Num)] = &[
    ("default_session_window", 0.5, 24.0, Num::Decimal),
    ("default_serving_size", 1.0, 100.0, Num::Int),
    ("default_service_fee", 0.0, 100_000.0, Num::Decimal),
    ("deposit_cents", 0.0, 10_000_000.0, Num::BigInt),
    ("cancellation_hours", 0.0, 720.0, Num::Int),
];

/// 2 MB image as a base64 data URL (the client caps uploads at 2 MB).
const MAX_LOGO_CHARS: usize = 2_800_000;

/// GET/PUT /api/settings: the settings row plus account meta.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SettingsResponse {
    #[serde(flatten)]
    #[ts(flatten)]
    pub settings: Settings,
    pub slug: String,
    pub account_email: String,
    pub ai_configured: bool,
}

/// JS `.length` (UTF-16 code units).
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

async fn with_meta(state: &AppState, tx: &mut Tx, account_id: i64) -> DbResult<SettingsResponse> {
    let settings = accounts::get_settings(tx, account_id).await?;
    let account = accounts::get_account(tx, account_id).await?;
    Ok(SettingsResponse {
        settings,
        slug: account.as_ref().map(|a| a.slug.clone()).unwrap_or_default(),
        account_email: account.map(|a| a.email).unwrap_or_default(),
        ai_configured: state.config.anthropic_api_key.is_some(),
    })
}

async fn get_settings(
    State(state): State<AppState>,
    mut t: Tenant,
) -> DbResult<Json<SettingsResponse>> {
    let res = with_meta(&state, &mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

async fn put_settings(
    State(state): State<AppState>,
    mut t: Tenant,
    body: JsonBody,
) -> DbResult<Json<SettingsResponse>> {
    let id = t.account.id;
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE settings SET ");
    let mut sets = qb.separated(", ");
    let mut changed = false;

    for &(field, max) in TEXT_FIELDS {
        let Some(raw) = body.get(field) else { continue };
        let v = if raw.is_null() {
            String::new()
        } else {
            js::string(raw).trim().to_string()
        };
        if js_len(&v) > max {
            return Err(
                AppError::validation(format!("{field} must be {max} characters or fewer")).into(),
            );
        }
        if field == "email" && !v.is_empty() && !EMAIL_RE.is_match(&v) {
            return Err(AppError::validation("Invalid email format").into());
        }
        sets.push(format!("{field} = ")).push_bind_unseparated(v);
        changed = true;
    }

    for &(field, min, max, kind) in NUMBER_FIELDS {
        let Some(raw) = body.get(field) else { continue };
        if raw.is_null() || raw.as_str() == Some("") {
            continue;
        }
        let n = js::number(raw);
        if !n.is_finite() || n < min || n > max {
            return Err(AppError::validation(format!(
                "{field} must be between {} and {}",
                fmt_num(min),
                fmt_num(max)
            ))
            .into());
        }
        sets.push(format!("{field} = "));
        match kind {
            Num::Decimal => sets.push_bind_unseparated(js::decimal(n).unwrap_or_default()),
            Num::Int => sets.push_bind_unseparated(js_round(n) as i32),
            Num::BigInt => sets.push_bind_unseparated(js_round(n) as i64),
        };
        changed = true;
    }

    for field in ["remind_client", "remind_chef"] {
        if let Some(on) = body.opt_flag(field) {
            sets.push(format!("{field} = ")).push_bind_unseparated(on);
            changed = true;
        }
    }

    if let Some(raw) = body.get("timezone") {
        let tz = if raw.is_null() {
            String::new()
        } else {
            js::string(raw).trim().to_string()
        };
        // Postgres's own zone list: the reminder job converts with AT TIME ZONE.
        let known: bool = tz.is_empty()
            || (tz.len() <= 64
                && sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1)",
                )
                .bind(&tz)
                .fetch_one(&mut *t.tx)
                .await?);
        if !known {
            return Err(AppError::validation("Unknown time zone").into());
        }
        sets.push("timezone = ")
            .push_bind_unseparated((!tz.is_empty()).then_some(tz));
        changed = true;
    }

    if body.has("invoice_prefix") {
        let p: String = body
            .str("invoice_prefix")
            .to_uppercase()
            .chars()
            .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .take(8)
            .collect();
        let p = if p.is_empty() { "INV".to_string() } else { p };
        sets.push("invoice_prefix = ").push_bind_unseparated(p);
        changed = true;
    }

    if let Some(raw) = body.get("service_types") {
        let Value::Array(items) = raw else {
            return Err(AppError::validation("service_types must be an array").into());
        };
        let types: Vec<String> = items
            .iter()
            .take(12)
            .map(|t| js::string(t).chars().take(60).collect())
            .collect();
        sets.push("service_types = ")
            .push_bind_unseparated(json!(types));
        changed = true;
    }

    if let Some(raw) = body.get("logo_base64") {
        let logo = if js::truthy(raw) { Some(raw) } else { None };
        let logo = match logo {
            None => None,
            Some(Value::String(s))
                if s.starts_with("data:image/") && js_len(s) <= MAX_LOGO_CHARS =>
            {
                Some(s.clone())
            }
            Some(_) => {
                return Err(AppError::validation("Logo must be an image under 2 MB").into());
            }
        };
        sets.push("logo_base64 = ").push_bind_unseparated(logo);
        changed = true;
    }

    if changed {
        sqlx::query("INSERT INTO settings (account_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id)
            .execute(&mut *t.tx)
            .await?;
        sets.push("updated_at = now()");
        qb.push(" WHERE account_id = ").push_bind(id);
        qb.build().execute(&mut *t.tx).await?;
    }

    let res = with_meta(&state, &mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

/// `Math.round`: halves toward +infinity.
fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// A JS number as `String(n)` would print it.
fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// GET /api/settings/public — SEC6 legacy: intake links shared before
/// accounts existed (/intake/new) belong to account #1.
async fn legacy_public(State(state): State<AppState>) -> DbResult<Response> {
    let profile: Option<PublicProfile> = profile_for(&state, 1).await?;
    Ok(match profile {
        Some(p) => Json(p).into_response(),
        None => Json(json!({})).into_response(),
    })
}

/// GET /api/settings/export — everything this account owns. Same layout as
/// the Node export (raw rows: JSON columns as JSON text), since it's an
/// archive format; see [`raw_json_columns`].
#[derive(Serialize)]
pub struct DataExport {
    pub exported_at: String,
    pub account: Option<Value>,
    pub settings: Settings,
    pub clients: Vec<Client>,
    pub recipes: Vec<RecipeRow>,
    pub ingredients: Vec<Ingredient>,
    pub sessions: Vec<SessionRow>,
    pub session_dishes: Vec<Value>,
    pub grocery_items: Vec<GroceryItemRow>,
    pub session_templates: Vec<SessionTemplateRow>,
    pub client_intakes: Vec<IntakeRow>,
    pub invoices: Vec<InvoiceRow>,
    pub invoice_line_items: Vec<InvoiceLineItem>,
    pub payment_events: Vec<Value>,
}

async fn export(mut t: Tenant) -> DbResult<Response> {
    let id = t.account.id;
    let tx = &mut t.tx;
    macro_rules! all {
        ($ty:ty, $sql:expr) => {
            sqlx::query_as::<_, $ty>($sql)
                .bind(id)
                .fetch_all(&mut **tx)
                .await?
        };
    }
    let account: Option<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', id, 'email', email, 'slug', slug,
                'onboarding_completed_at', to_char(onboarding_completed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),
                'created_at', to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'))
         FROM accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    let settings = accounts::get_settings(tx, id).await?;
    let session_dishes: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(sd) FROM session_dishes sd JOIN sessions s ON s.id = sd.session_id
         WHERE s.account_id = $1 ORDER BY sd.id",
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;
    let payment_events: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', id, 'intake_id', intake_id, 'invoice_id', invoice_id,
                'event_type', event_type,
                'amount', amount, 'currency', currency, 'status', status,
                'created_at', to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'))
         FROM payment_events WHERE account_id = $1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut **tx)
    .await?;

    let data = DataExport {
        exported_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        account,
        settings,
        clients: all!(
            Client,
            "SELECT * FROM clients WHERE account_id = $1 ORDER BY id"
        ),
        recipes: all!(
            RecipeRow,
            "SELECT * FROM recipes WHERE account_id = $1 ORDER BY id"
        ),
        ingredients: all!(
            Ingredient,
            "SELECT i.* FROM ingredients i JOIN recipes r ON r.id = i.recipe_id
             WHERE r.account_id = $1 ORDER BY i.id"
        ),
        sessions: all!(
            SessionRow,
            "SELECT * FROM sessions WHERE account_id = $1 ORDER BY id"
        ),
        session_dishes,
        grocery_items: all!(
            GroceryItemRow,
            "SELECT g.* FROM grocery_items g JOIN sessions s ON s.id = g.session_id
             WHERE s.account_id = $1 ORDER BY g.id"
        ),
        session_templates: all!(
            SessionTemplateRow,
            "SELECT * FROM session_templates WHERE account_id = $1 ORDER BY id"
        ),
        client_intakes: all!(
            IntakeRow,
            "SELECT * FROM client_intakes WHERE account_id = $1 ORDER BY id"
        ),
        invoices: all!(
            InvoiceRow,
            "SELECT * FROM invoices WHERE account_id = $1 ORDER BY id"
        ),
        invoice_line_items: all!(
            InvoiceLineItem,
            "SELECT li.* FROM invoice_line_items li JOIN invoices i ON i.id = li.invoice_id
             WHERE i.account_id = $1 ORDER BY li.id"
        ),
        payment_events,
    };
    t.tx.commit().await?;

    let mut data = serde_json::to_value(data).map_err(AppError::internal)?;
    raw_json_columns(&mut data);
    let stamp = chrono::Utc::now().format("%Y-%m-%d");
    let mut res = Json(data).into_response();
    res.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"chef-data-{stamp}.json\""))
            .expect("ASCII header"),
    );
    Ok(res)
}

/// JSONB columns as the JSON text SQLite stored, per exported table.
const EXPORT_JSON_COLUMNS: &[(&str, &[&str])] = &[
    (
        "clients",
        &["allergens", "dietary_styles", "liked_proteins", "equipment"],
    ),
    ("recipes", &["key_overlap_ingredients"]),
    ("grocery_items", &["source_dishes"]),
    ("session_templates", &["recipe_ids"]),
    (
        "client_intakes",
        &["allergens", "dietary_styles", "liked_proteins"],
    ),
    ("invoices", &["recipes_summary"]),
];

/// Turns parsed JSON columns back into JSON text, like the Node export.
fn raw_json_columns(data: &mut Value) {
    let text = |v: &mut Value| {
        if !v.is_null() && !v.is_string() {
            *v = Value::String(v.to_string());
        }
    };
    for (table, cols) in EXPORT_JSON_COLUMNS {
        if let Some(Value::Array(rows)) = data.get_mut(*table) {
            for row in rows {
                for col in *cols {
                    if let Some(v) = row.get_mut(*col) {
                        text(v);
                    }
                }
            }
        }
    }
    if let Some(v) = data.pointer_mut("/settings/service_types") {
        text(v);
    }
}

/// DELETE /api/settings/clear-sample.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ClearSampleResponse {
    #[ts(type = "true")]
    pub success: bool,
    pub removed: usize,
    pub message: String,
}

async fn clear_sample(mut t: Tenant) -> DbResult<Json<ClearSampleResponse>> {
    let id = t.account.id;
    let samples: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM clients WHERE account_id = $1 AND is_sample")
            .bind(id)
            .fetch_all(&mut *t.tx)
            .await?;
    for client in &samples {
        // Invoices reference sessions (no cascade): remove them first, including
        // any invoice that only points at one of the sample client's sessions.
        // Their payment events (if any) are kept, detached.
        sqlx::query(
            "UPDATE payment_events SET invoice_id = NULL
              WHERE account_id = $1 AND invoice_id IN (
                SELECT id FROM invoices WHERE account_id = $1 AND (client_id = $2
                  OR session_id IN (SELECT id FROM sessions WHERE client_id = $2 AND account_id = $1)))",
        )
        .bind(id)
        .bind(client)
        .execute(&mut *t.tx)
        .await?;
        sqlx::query(
            "DELETE FROM invoices WHERE account_id = $1 AND (client_id = $2
               OR session_id IN (SELECT id FROM sessions WHERE client_id = $2 AND account_id = $1))",
        )
        .bind(id)
        .bind(client)
        .execute(&mut *t.tx)
        .await?;
        sqlx::query("DELETE FROM sessions WHERE client_id = $1 AND account_id = $2")
            .bind(client)
            .bind(id)
            .execute(&mut *t.tx)
            .await?;
        sqlx::query(
            "UPDATE client_intakes SET converted_client_id = NULL
             WHERE converted_client_id = $1 AND account_id = $2",
        )
        .bind(client)
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
        sqlx::query("DELETE FROM clients WHERE id = $1 AND account_id = $2")
            .bind(client)
            .bind(id)
            .execute(&mut *t.tx)
            .await?;
    }
    t.tx.commit().await?;
    Ok(Json(ClearSampleResponse {
        success: true,
        removed: samples.len(),
        message: "Sample data cleared".into(),
    }))
}
