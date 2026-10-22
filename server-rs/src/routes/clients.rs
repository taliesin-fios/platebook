// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/clients.js`.
//!
//! GET    /api/clients      — active clients with session counts
//! GET    /api/clients/:id  — one client with its sessions
//! POST   /api/clients      — create
//! PUT    /api/clients/:id  — partial update
//! DELETE /api/clients/:id  — soft delete (is_active = false)

use std::sync::LazyLock;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::db::Tenant;
use crate::error::{AppError, DbResult};
use crate::js;
use crate::models::{Client, SUCCESS, SessionRow, Success};
use crate::routes::settings::EMAIL_RE;
use crate::routes::{JsonBody, json_or_empty, parse_id};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
}

static PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\d\s()+.\-]{7,20}$").expect("valid regex"));

/// GET /api/clients.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct ClientListItem {
    #[serde(flatten)]
    #[sqlx(flatten)]
    #[ts(flatten)]
    pub client: Client,
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "string | null")]
    pub last_session_date: Option<NaiveDate>,
}

/// A session row in GET /api/clients/:id.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct ClientSession {
    #[serde(flatten)]
    #[sqlx(flatten)]
    #[ts(flatten)]
    pub session: SessionRow,
    #[ts(type = "number")]
    pub dish_count: i64,
}

/// GET /api/clients/:id.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ClientDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub client: Client,
    pub sessions: Vec<ClientSession>,
}

fn not_found() -> AppError {
    AppError::not_found_msg("Client not found")
}

/// `typeof v === 'string'` and its JS length.
fn as_str(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str)
}

fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `/regex/.test(v)` on a truthy value (non-strings are stringified like JS).
fn invalid(v: Option<&Value>, re: &Regex) -> bool {
    match v {
        Some(v) if js::truthy(v) => !re.is_match(&js::string(v)),
        _ => false,
    }
}

async fn list(mut t: Tenant) -> DbResult<Json<Vec<ClientListItem>>> {
    let rows = sqlx::query_as::<_, ClientListItem>(
        r#"SELECT c.*,
             (SELECT COUNT(*) FROM sessions WHERE client_id = c.id) AS session_count,
             (SELECT MAX(session_date) FROM sessions WHERE client_id = c.id) AS last_session_date
           FROM clients c
           WHERE c.account_id = $1 AND c.is_active
           ORDER BY c.name COLLATE "C" ASC"#,
    )
    .bind(t.account.id)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

async fn detail(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<ClientDetail>> {
    let Some(id) = parse_id(&raw_id) else {
        return Err(not_found().into());
    };
    let client = sqlx::query_as::<_, Client>(
        "SELECT * FROM clients WHERE id = $1 AND account_id = $2 AND is_active",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(not_found)?;
    let sessions = sqlx::query_as::<_, ClientSession>(
        "SELECT s.*, COUNT(sd.id) AS dish_count
         FROM sessions s
         LEFT JOIN session_dishes sd ON sd.session_id = s.id
         WHERE s.client_id = $1 AND s.account_id = $2
         GROUP BY s.id
         ORDER BY s.session_date DESC NULLS LAST",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(ClientDetail { client, sessions }))
}

async fn create(mut t: Tenant, body: JsonBody) -> DbResult<(StatusCode, Json<Client>)> {
    let Some(name) = as_str(body.get("name")).filter(|n| !n.is_empty()) else {
        return Err(AppError::validation("Name is required").into());
    };
    if js_len(name) > 200 {
        return Err(AppError::validation("Name must be 200 characters or fewer").into());
    }
    if invalid(body.get("email"), &EMAIL_RE) {
        return Err(AppError::validation("Invalid email format").into());
    }
    if invalid(body.get("phone"), &PHONE_RE) {
        return Err(AppError::validation("Invalid phone format").into());
    }

    let client = sqlx::query_as::<_, Client>(
        "INSERT INTO clients (
           account_id, name, phone, email, address, household_type, adults, children, serving_size,
           allergens, dietary_styles, liked_proteins, disliked_foods, spice_tolerance,
           oven_available, stovetop_type, equipment, client_containers, parking_notes,
           preferred_day, preferred_time, grocery_handling, chef_notes
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                   $18, $19, $20, $21, $22, $23)
         RETURNING *",
    )
    .bind(t.account.id)
    .bind(name)
    .bind(body.opt_text("phone"))
    .bind(body.opt_text("email"))
    .bind(body.opt_text("address"))
    .bind(body.opt_text("household_type"))
    .bind(body.i64_or("adults", 2) as i32)
    .bind(body.i64_or("children", 0) as i32)
    .bind(body.i64_or("serving_size", 4) as i32)
    .bind(json_or_empty(body.get("allergens")))
    .bind(json_or_empty(body.get("dietary_styles")))
    .bind(json_or_empty(body.get("liked_proteins")))
    .bind(body.opt_text("disliked_foods"))
    .bind(body.opt_text("spice_tolerance"))
    .bind(body.opt_flag("oven_available").unwrap_or(true))
    .bind(body.opt_text("stovetop_type"))
    .bind(json_or_empty(body.get("equipment")))
    .bind(body.truthy("client_containers"))
    .bind(body.opt_text("parking_notes"))
    .bind(body.opt_text("preferred_day"))
    .bind(body.opt_text("preferred_time"))
    .bind(body.opt_text("grocery_handling"))
    .bind(body.opt_text("chef_notes"))
    .fetch_one(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(client)))
}

const TEXT_FIELDS: &[&str] = &[
    "name",
    "phone",
    "email",
    "address",
    "household_type",
    "disliked_foods",
    "spice_tolerance",
    "stovetop_type",
    "parking_notes",
    "preferred_day",
    "preferred_time",
    "grocery_handling",
    "chef_notes",
];
const INT_FIELDS: &[&str] = &["adults", "children", "serving_size"];
const FLAG_FIELDS: &[&str] = &["oven_available", "client_containers"];
const JSON_FIELDS: &[&str] = &["allergens", "dietary_styles", "liked_proteins", "equipment"];

async fn update(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<Client>> {
    let Some(id) = parse_id(&raw_id) else {
        return Err(not_found().into());
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM clients WHERE id = $1 AND account_id = $2 AND is_active)",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_one(&mut *t.tx)
    .await?;
    if !exists {
        return Err(not_found().into());
    }

    // V2: reject empty body
    if body.0.is_empty() {
        return Err(
            AppError::new(StatusCode::BAD_REQUEST, "EMPTY_BODY", "No fields provided").into(),
        );
    }
    // V1: validate key field types/lengths
    if body.has("name") {
        match as_str(body.get("name")) {
            Some(n) if !n.is_empty() => {
                if js_len(n) > 200 {
                    return Err(AppError::validation("name must be 200 characters or fewer").into());
                }
            }
            _ => {
                return Err(AppError::validation("name must be a non-empty string").into());
            }
        }
    }
    if invalid(body.get("email"), &EMAIL_RE) {
        return Err(AppError::validation("Invalid email format").into());
    }
    if invalid(body.get("phone"), &PHONE_RE) {
        return Err(AppError::validation("Invalid phone format").into());
    }
    for f in ["adults", "children", "serving_size", "client_containers"] {
        if let Some(v) = body.get(f)
            && js::integer(v).is_none()
        {
            return Err(AppError::validation(format!("{f} must be an integer")).into());
        }
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE clients SET ");
    let mut sets = qb.separated(", ");
    for &f in TEXT_FIELDS {
        if body.has(f) {
            sets.push(format!("{f} = "))
                .push_bind_unseparated(body.opt_text(f));
        }
    }
    for &f in INT_FIELDS {
        if body.has(f) {
            sets.push(format!("{f} = "))
                .push_bind_unseparated(body.opt_i64(f).map(|n| n as i32));
        }
    }
    for &f in FLAG_FIELDS {
        if let Some(v) = body.get(f) {
            let flag = (!v.is_null()).then(|| js::truthy(v));
            sets.push(format!("{f} = ")).push_bind_unseparated(flag);
        }
    }
    for &f in JSON_FIELDS {
        if let Some(v) = body.get(f) {
            sets.push(format!("{f} = "))
                .push_bind_unseparated(v.clone());
        }
    }
    sets.push("updated_at = now()");
    qb.push(" WHERE id = ")
        .push_bind(id)
        .push(" AND account_id = ")
        .push_bind(t.account.id)
        .push(" RETURNING *");
    let client = qb.build_query_as::<Client>().fetch_one(&mut *t.tx).await?;
    t.tx.commit().await?;
    Ok(Json(client))
}

async fn remove(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<Success>> {
    let Some(id) = parse_id(&raw_id) else {
        return Err(not_found().into());
    };
    let res = sqlx::query(
        "UPDATE clients SET is_active = false, updated_at = now() WHERE id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(t.account.id)
    .execute(&mut *t.tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(not_found().into());
    }
    t.tx.commit().await?;
    Ok(Json(SUCCESS))
}
