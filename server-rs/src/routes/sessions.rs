// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/sessions.js`.
//!
//! GET    /api/sessions                          — list (status, client_id, upcoming, today)
//! POST   /api/sessions                          — create (+ dishes)
//! GET    /api/sessions/:id                      — detail with dishes and margin
//! PUT    /api/sessions/:id                      — partial update (+ replace dishes)
//! DELETE /api/sessions/:id                      — blocked while a non-void invoice is attached
//! POST   /api/sessions/:id/clone                — copy shifted by days_offset
//! POST   /api/sessions/:id/complete             — mark Completed
//! POST   /api/sessions/:id/dishes               — replace the dish list
//! POST   /api/sessions/from-template/:template_id
//! POST   /api/sessions/:id/email-pdf            — email the meal plan PDF to the client
//! GET    /api/sessions/:id/pdf                  — the meal plan PDF
//!
//! Every `/:id` route first checks the session belongs to the account, like
//! the Node `router.param('id')` guard.

use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, NaiveDate};
use regex::Regex;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgConnection, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::accounts;
use crate::db::{Tenant, begin_tenant};
use crate::email;
use crate::error::{AppError, BareResult, DbResult};
use crate::js;
use crate::models::{RecipeRow, SUCCESS, SessionRow, Success};
use crate::money;
use crate::ownership::{Scoped, owned, owned_ids};
use crate::pdf::{self, Template, docs};
use crate::routes::costs::{MARGIN_PCT_SQL, pct};
use crate::routes::{JsonBody, body_id, parse_id};
use crate::serde_util::{json_array, json_text};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/clone", post(clone))
        .route("/{id}/complete", post(complete))
        .route("/{id}/dishes", post(replace_dishes))
        .route("/{id}/email-pdf", post(email_pdf))
        .route("/{id}/pdf", get(meal_plan_pdf))
        .route("/from-template/{template_id}", post(from_template))
        .merge(crate::routes::grocery::session_router())
}

pub const SESSION_STATUSES: &[&str] = &["Pending", "Confirmed", "Completed", "Cancelled"];

/// `repeat_every_weeks` from the body: `None` = absent, `Some(None)` = one-off
/// (null, "" or 0), `Some(Some(n))` for 1–8; anything else is a 400.
fn body_repeat(body: &JsonBody) -> Result<Option<Option<i16>>, AppError> {
    let invalid =
        || AppError::validation("repeat_every_weeks must be a whole number from 1 to 8 (or empty)");
    // The raw value, not opt_i64: that rounds 2.5 up and reads junk as "empty".
    let weeks = match body.get("repeat_every_weeks") {
        None => return Ok(None),
        Some(Value::Null) => return Ok(Some(None)),
        Some(Value::String(s)) if s.trim().is_empty() => return Ok(Some(None)),
        Some(Value::String(s)) => s.trim().parse::<i64>().map_err(|_| invalid())?,
        Some(Value::Number(n)) => n.as_i64().ok_or_else(invalid)?,
        Some(_) => return Err(invalid()),
    };
    match weeks {
        0 => Ok(Some(None)),
        1..=8 => Ok(Some(Some(weeks as i16))),
        _ => Err(invalid()),
    }
}

fn invalid_status() -> AppError {
    AppError::validation(format!(
        "Invalid status. Must be one of: {}",
        SESSION_STATUSES.join(", ")
    ))
}

fn session_not_found() -> AppError {
    AppError::not_found_msg("Session not found")
}

/// The `router.param('id')` guard: the id, if this account owns the session.
async fn guard(t: &mut Tenant, raw_id: &str) -> Result<i64, AppError> {
    let id = parse_id(raw_id).ok_or_else(session_not_found)?;
    if !owned(&mut t.tx, Scoped::Sessions, id, t.account.id).await? {
        return Err(session_not_found());
    }
    Ok(id)
}

/// A recipe attached to a session.
#[derive(Debug, Clone, Serialize, FromRow, TS)]
#[ts(export)]
pub struct SessionDish {
    #[ts(type = "number")]
    pub session_dish_id: i64,
    pub sort_order: Option<i32>,
    #[serde(flatten)]
    #[sqlx(flatten)]
    #[ts(flatten)]
    pub recipe: RecipeRow,
    #[sqlx(skip)]
    pub total_time: i32,
}

#[derive(FromRow)]
struct DetailRow {
    #[sqlx(flatten)]
    session: SessionRow,
    client_name: Option<String>,
    client_email: Option<String>,
    client_allergens: Option<Value>,
    household_type: Option<String>,
    client_serving_size: Option<i32>,
}

/// GET /api/sessions/:id and the body of most session mutations.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SessionDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub session: SessionRow,
    pub client_name: Option<String>,
    pub client_email: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub client_allergens: Option<Value>,
    pub household_type: Option<String>,
    pub client_serving_size: Option<i32>,
    pub dishes: Vec<SessionDish>,
    #[ts(type = "number")]
    pub gross_margin: Decimal,
    #[ts(type = "number")]
    pub margin_pct: Decimal,
}

/// Port of `getSessionWithDetails(db, id)`.
pub async fn session_with_details(
    conn: &mut PgConnection,
    id: i64,
) -> Result<Option<SessionDetail>, sqlx::Error> {
    let Some(row) = sqlx::query_as::<_, DetailRow>(
        "SELECT s.*, c.name AS client_name, c.email AS client_email,
                c.allergens AS client_allergens,
                c.household_type, c.serving_size AS client_serving_size
         FROM sessions s LEFT JOIN clients c ON c.id = s.client_id
         WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let mut dishes = sqlx::query_as::<_, SessionDish>(
        "SELECT sd.id AS session_dish_id, sd.sort_order, r.*
         FROM session_dishes sd JOIN recipes r ON r.id = sd.recipe_id
         WHERE sd.session_id = $1
         ORDER BY sd.sort_order ASC, sd.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    for d in &mut dishes {
        d.total_time = d.recipe.total_time();
    }
    let fee = row.session.service_fee.unwrap_or_default();
    let cost = row.session.ingredient_cost.unwrap_or_default();
    let gross = money::subtract_money(fee, cost);
    Ok(Some(SessionDetail {
        margin_pct: pct(gross, fee),
        gross_margin: gross,
        session: row.session,
        client_name: row.client_name,
        client_email: row.client_email,
        client_allergens: row.client_allergens,
        household_type: row.household_type,
        client_serving_size: row.client_serving_size,
        dishes,
    }))
}

async fn details_or_404(conn: &mut PgConnection, id: i64) -> Result<SessionDetail, AppError> {
    session_with_details(conn, id)
        .await?
        .ok_or_else(session_not_found)
}

/// True if every id is an active recipe of this account (`allRecipesOwned`).
async fn all_recipes_owned(
    conn: &mut PgConnection,
    raw: &[Value],
    account_id: i64,
) -> Result<bool, sqlx::Error> {
    let mut unique: Vec<i64> = Vec::new();
    for v in raw {
        let Some(n) = js::integer(v) else {
            return Ok(false);
        };
        if !unique.contains(&n) {
            unique.push(n);
        }
    }
    let found: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recipes WHERE account_id = $1 AND id = ANY($2) AND is_active",
    )
    .bind(account_id)
    .bind(&unique)
    .fetch_one(conn)
    .await?;
    Ok(found == unique.len() as i64)
}

async fn insert_dishes(
    conn: &mut PgConnection,
    session_id: i64,
    recipe_ids: &[i64],
) -> Result<(), sqlx::Error> {
    for (i, rid) in recipe_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO session_dishes (session_id, recipe_id, sort_order) VALUES ($1, $2, $3)",
        )
        .bind(session_id)
        .bind(rid)
        .bind(i as i32)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("regex"));

/// A `YYYY-MM-DD` body field. Blank/`null` is `None`; anything else must be a
/// real date (SQLite stored any text; Postgres has a DATE column).
pub fn body_date(body: &JsonBody, key: &str) -> Result<Option<NaiveDate>, AppError> {
    match body.opt_text(key) {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => {
            let day = s.get(..10).unwrap_or(&s);
            NaiveDate::parse_from_str(day, "%Y-%m-%d")
                .map(Some)
                .map_err(|_| AppError::validation(format!("{key} must be a date (YYYY-MM-DD)")))
        }
    }
}

/// GET /api/sessions row.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct SessionListItem {
    #[serde(flatten)]
    #[sqlx(flatten)]
    #[ts(flatten)]
    pub session: SessionRow,
    pub client_name: Option<String>,
    pub client_email: Option<String>,
    /// Raw JSON array text: the list route doesn't parse it.
    #[serde(serialize_with = "json_text::serialize")]
    #[ts(type = "string | null")]
    pub client_allergens: Option<Value>,
    #[ts(type = "number")]
    pub dish_count: i64,
    #[ts(type = "number")]
    pub gross_margin: Option<Decimal>,
    #[ts(type = "number")]
    pub margin_pct: Option<Decimal>,
}

async fn list(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Json<Vec<SessionListItem>>> {
    let status = q.get("status").filter(|s| !s.is_empty());
    if let Some(s) = status
        && !SESSION_STATUSES.contains(&s.as_str())
    {
        return Err(invalid_status().into());
    }
    let upcoming = q.get("upcoming").map(String::as_str) == Some("true");
    // The client sends its own local YYYY-MM-DD so "upcoming" doesn't drop
    // today's session in the evening; else the account's local date.
    let client_today = q
        .get("today")
        .filter(|d| DATE_RE.is_match(d))
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!(
        "SELECT s.*, c.name AS client_name, c.email AS client_email,
                c.allergens AS client_allergens,
                COUNT(sd.id) AS dish_count,
                (s.service_fee - s.ingredient_cost) AS gross_margin,
                {MARGIN_PCT_SQL} AS margin_pct
         FROM sessions s
         LEFT JOIN clients c ON c.id = s.client_id
         LEFT JOIN session_dishes sd ON sd.session_id = s.id
         WHERE s.account_id = "
    ));
    qb.push_bind(t.account.id);
    if let Some(s) = status {
        qb.push(" AND s.status = ").push_bind(s.clone());
    }
    if let Some(c) = q.get("client_id").filter(|c| !c.is_empty()) {
        // SQLite compared the text param to the integer column; junk matched nothing.
        qb.push(" AND s.client_id = ")
            .push_bind(parse_id(c).unwrap_or(-1));
    }
    if upcoming {
        let today = match client_today {
            Some(d) => d,
            None => accounts::local_clock(&mut t.tx, t.account.id).await?.today,
        };
        qb.push(" AND s.session_date >= ").push_bind(today);
        qb.push(" AND s.status NOT IN ('Cancelled', 'Completed')");
    }
    qb.push(" GROUP BY s.id, c.id ORDER BY s.session_date ");
    qb.push(if upcoming {
        "ASC NULLS FIRST, s.id"
    } else {
        "DESC NULLS LAST, s.id"
    });
    let rows = qb
        .build_query_as::<SessionListItem>()
        .fetch_all(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

async fn active_client(
    conn: &mut PgConnection,
    id: Option<i64>,
    account_id: i64,
) -> Result<bool, sqlx::Error> {
    let Some(id) = id else { return Ok(false) };
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM clients WHERE id = $1 AND account_id = $2 AND is_active)",
    )
    .bind(id)
    .bind(account_id)
    .fetch_one(conn)
    .await
}

fn recipes_not_found() -> AppError {
    AppError::validation("One or more recipe IDs not found")
}

async fn create(mut t: Tenant, body: JsonBody) -> DbResult<(StatusCode, Json<SessionDetail>)> {
    let account = t.account.id;
    if !body.truthy("client_id") {
        return Err(AppError::validation("client_id is required").into());
    }
    let client_id = body_id(body.get("client_id"));
    if !active_client(&mut t.tx, client_id, account).await? {
        return Err(AppError::validation("Client not found").into());
    }
    if let Some(s) = body.text_or_none("status")
        && !SESSION_STATUSES.contains(&s.as_str())
    {
        return Err(invalid_status().into());
    }
    let recipe_values = match body.get("recipe_ids") {
        Some(Value::Array(a)) if !a.is_empty() => a.clone(),
        _ => vec![],
    };
    if !recipe_values.is_empty() && !all_recipes_owned(&mut t.tx, &recipe_values, account).await? {
        return Err(recipes_not_found().into());
    }
    let session_date = body_date(&body, "session_date")?;
    let repeat = body_repeat(&body)?.flatten();

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (
           account_id, client_id, session_date, session_time, serving_size, session_window_hours,
           status, service_fee, ingredient_cost, chef_notes, template_name, repeat_every_weeks
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
    )
    .bind(account)
    .bind(client_id)
    .bind(session_date)
    .bind(body.opt_text("session_time"))
    .bind(body.i64_or("serving_size", 4) as i32)
    .bind(body.decimal_or("session_window_hours", Decimal::new(3, 0)))
    .bind(
        body.text_or_none("status")
            .unwrap_or_else(|| "Pending".into()),
    )
    .bind(body.decimal_or("service_fee", Decimal::ZERO))
    .bind(body.decimal_or("ingredient_cost", Decimal::ZERO))
    .bind(body.opt_text("chef_notes"))
    .bind(body.opt_text("template_name"))
    .bind(repeat)
    .fetch_one(&mut *t.tx)
    .await?;

    let ids: Vec<i64> = recipe_values.iter().filter_map(js::integer).collect();
    insert_dishes(&mut t.tx, id, &ids).await?;
    let res = details_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(res)))
}

async fn detail(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<SessionDetail>> {
    let id = guard(&mut t, &raw_id).await?;
    let res = details_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

async fn update(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<SessionDetail>> {
    let id = guard(&mut t, &raw_id).await?;
    let account = t.account.id;

    if body.has("status") {
        let s = body.opt_text("status").unwrap_or_default();
        if !SESSION_STATUSES.contains(&s.as_str()) {
            return Err(invalid_status().into());
        }
    }
    let repeat = body_repeat(&body)?;
    // If reassigning the client, the target must exist and be active
    if body.has("client_id")
        && !active_client(&mut t.tx, body_id(body.get("client_id")), account).await?
    {
        return Err(AppError::validation("Client not found").into());
    }

    // The slot before the update: a reminder already sent for it is resent
    // if the date or time moves.
    let old_slot: Option<(Option<NaiveDate>, Option<String>)> =
        if body.has("session_date") || body.has("session_time") {
            sqlx::query_as("SELECT session_date, session_time FROM sessions WHERE id = $1")
                .bind(id)
                .fetch_optional(&mut *t.tx)
                .await?
        } else {
            None
        };

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE sessions SET ");
    let mut sets = qb.separated(", ");
    let mut changed = false;
    if body.has("client_id") {
        sets.push("client_id = ")
            .push_bind_unseparated(body_id(body.get("client_id")));
        changed = true;
    }
    if body.has("session_date") {
        sets.push("session_date = ")
            .push_bind_unseparated(body_date(&body, "session_date")?);
        changed = true;
    }
    for f in ["session_time", "status", "chef_notes", "template_name"] {
        if body.has(f) {
            sets.push(format!("{f} = "))
                .push_bind_unseparated(body.opt_text(f));
            changed = true;
        }
    }
    if let Some(r) = repeat {
        sets.push("repeat_every_weeks = ").push_bind_unseparated(r);
        changed = true;
    }
    if body.has("serving_size") {
        sets.push("serving_size = ")
            .push_bind_unseparated(body.opt_i64("serving_size").map(|n| n as i32));
        changed = true;
    }
    for f in ["session_window_hours", "service_fee", "ingredient_cost"] {
        if body.has(f) {
            sets.push(format!("{f} = "))
                .push_bind_unseparated(body.opt_decimal(f));
            changed = true;
        }
    }
    if changed {
        qb.push(" WHERE id = ").push_bind(id);
        qb.build().execute(&mut *t.tx).await?;
    }
    if let Some((date, time)) = old_slot {
        sqlx::query(
            "UPDATE sessions SET client_reminder_sent_at = NULL, chef_reminder_sent_at = NULL
             WHERE id = $1 AND (session_date IS DISTINCT FROM $2 OR session_time IS DISTINCT FROM $3)",
        )
        .bind(id)
        .bind(date)
        .bind(time)
        .execute(&mut *t.tx)
        .await?;
    }

    if let Some(raw) = body.get("recipe_ids") {
        let Value::Array(values) = raw else {
            return Err(AppError::validation("recipe_ids must be an array").into());
        };
        // Validate every referenced recipe before swapping dishes
        if !values.is_empty() && !all_recipes_owned(&mut t.tx, values, account).await? {
            return Err(recipes_not_found().into());
        }
        sqlx::query("DELETE FROM session_dishes WHERE session_id = $1")
            .bind(id)
            .execute(&mut *t.tx)
            .await?;
        let ids: Vec<i64> = values.iter().filter_map(js::integer).collect();
        insert_dishes(&mut t.tx, id, &ids).await?;
    }

    let res = details_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

async fn remove(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<Success>> {
    let id = guard(&mut t, &raw_id).await?;
    // Block deletion while a non-void invoice is linked — protect financial records
    let active: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, invoice_number FROM invoices
         WHERE session_id = $1 AND status IS DISTINCT FROM 'void' ORDER BY id LIMIT 1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *t.tx)
    .await?;
    if let Some((inv_id, number)) = active {
        let label = number
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("#{inv_id}"));
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "INVOICE_ATTACHED",
            format!("Cannot delete session — invoice {label} is attached. Void the invoice first."),
        )
        .with("invoice_id", inv_id)
        .into());
    }
    sqlx::query("DELETE FROM invoices WHERE session_id = $1")
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(SUCCESS))
}

async fn clone(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<(StatusCode, Json<SessionDetail>)> {
    let id = guard(&mut t, &raw_id).await?;
    let days = body
        .get("days_offset")
        .and_then(js::parse_int)
        .ok_or_else(|| AppError::validation("days_offset must be an integer"))?;
    if !(-366..=366).contains(&days) {
        return Err(AppError::validation("days_offset must be between -366 and 366").into());
    }
    let src = sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *t.tx)
        .await?
        .ok_or_else(session_not_found)?;
    let new_date = src.session_date.map(|d| d + Duration::days(days));

    let new_id: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (
           account_id, client_id, session_date, session_time, serving_size, session_window_hours,
           status, service_fee, ingredient_cost, chef_notes, template_name
         ) VALUES ($1, $2, $3, $4, $5, $6, 'Pending', $7, $8, $9, $10) RETURNING id",
    )
    .bind(t.account.id)
    .bind(src.client_id)
    .bind(new_date)
    .bind(&src.session_time)
    .bind(src.serving_size)
    .bind(src.session_window_hours)
    .bind(src.service_fee)
    .bind(src.ingredient_cost)
    .bind(&src.chef_notes)
    .bind(&src.template_name)
    .fetch_one(&mut *t.tx)
    .await?;
    sqlx::query(
        "INSERT INTO session_dishes (session_id, recipe_id, sort_order)
         SELECT $1, recipe_id, sort_order FROM session_dishes WHERE session_id = $2
         ORDER BY sort_order, id",
    )
    .bind(new_id)
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    let res = details_or_404(&mut t.tx, new_id).await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(res)))
}

/// POST /api/sessions/:id/complete: the completed session, plus the session
/// its repeat booked (or had already booked), if it repeats.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CompletedSession {
    #[serde(flatten)]
    #[ts(flatten)]
    pub session: SessionDetail,
    #[ts(type = "number | null")]
    pub next_session_id: Option<i64>,
    #[ts(type = "string | null")]
    pub next_session_date: Option<NaiveDate>,
}

/// Books the follow-up of repeating session `id`: same client, time, menu
/// and fees, `repeat_every_weeks` later, Pending. A no-op for one-off or
/// undated sessions and for inactive clients. The unique index on
/// `(account_id, repeat_of)` makes it happen at most once per session.
/// Returns the follow-up (new or existing), if there is one.
async fn book_repeat(
    conn: &mut PgConnection,
    id: i64,
    account_id: i64,
) -> Result<Option<(i64, Option<NaiveDate>)>, sqlx::Error> {
    let new_id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO sessions (
           account_id, client_id, session_date, session_time, serving_size, session_window_hours,
           status, service_fee, ingredient_cost, chef_notes, template_name,
           repeat_every_weeks, repeat_of
         )
         SELECT s.account_id, s.client_id, s.session_date + s.repeat_every_weeks * 7,
                s.session_time, s.serving_size, s.session_window_hours, 'Pending',
                s.service_fee, s.ingredient_cost, s.chef_notes, s.template_name,
                s.repeat_every_weeks, s.id
         FROM sessions s JOIN clients c ON c.id = s.client_id
         WHERE s.id = $1 AND s.account_id = $2
           AND s.repeat_every_weeks IS NOT NULL AND s.session_date IS NOT NULL
           AND c.is_active IS NOT FALSE
         ON CONFLICT (account_id, repeat_of) WHERE repeat_of IS NOT NULL DO NOTHING
         RETURNING id",
    )
    .bind(id)
    .bind(account_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(new_id) = new_id {
        sqlx::query(
            "INSERT INTO session_dishes (session_id, recipe_id, sort_order)
             SELECT $1, sd.recipe_id, sd.sort_order FROM session_dishes sd
             JOIN recipes r ON r.id = sd.recipe_id
             WHERE sd.session_id = $2 AND r.is_active
             ORDER BY sd.sort_order, sd.id",
        )
        .bind(new_id)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query_as("SELECT id, session_date FROM sessions WHERE account_id = $1 AND repeat_of = $2")
        .bind(account_id)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
}

async fn complete(mut t: Tenant, Path(raw_id): Path<String>) -> DbResult<Json<CompletedSession>> {
    let id = guard(&mut t, &raw_id).await?;
    // Atomic: only proceeds if the session is not already Completed
    let res = sqlx::query(
        "UPDATE sessions SET status = 'Completed', completed_at = now()
         WHERE id = $1 AND status IS DISTINCT FROM 'Completed'",
    )
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "ALREADY_COMPLETED",
            "Session is already completed",
        )
        .into());
    }
    let next = book_repeat(&mut t.tx, id, t.account.id).await?;
    let detail = details_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(CompletedSession {
        session: detail,
        next_session_id: next.map(|(id, _)| id),
        next_session_date: next.and_then(|(_, d)| d),
    }))
}

async fn replace_dishes(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> DbResult<Json<SessionDetail>> {
    let id = guard(&mut t, &raw_id).await?;
    let values = match body.get("recipe_ids") {
        None | Some(Value::Null) => vec![],
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err(AppError::validation("recipe_ids must be an array").into()),
    };
    if !values.is_empty() && !all_recipes_owned(&mut t.tx, &values, t.account.id).await? {
        return Err(recipes_not_found().into());
    }
    sqlx::query("DELETE FROM session_dishes WHERE session_id = $1")
        .bind(id)
        .execute(&mut *t.tx)
        .await?;
    let ids: Vec<i64> = values.iter().filter_map(js::integer).collect();
    insert_dishes(&mut t.tx, id, &ids).await?;
    let res = details_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

/// POST /api/sessions/from-template/:template_id.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct FromTemplateResponse {
    #[ts(type = "number[]")]
    pub recipe_ids: Vec<i64>,
    pub template_name: String,
}

async fn from_template(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> DbResult<Json<FromTemplateResponse>> {
    let not_found = || AppError::not_found_msg("Template not found");
    let id = parse_id(&raw_id).ok_or_else(not_found)?;
    let template = sqlx::query_as::<_, crate::models::SessionTemplateRow>(
        "SELECT * FROM session_templates WHERE id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(not_found)?;
    let recipe_ids = owned_ids(&mut t.tx, Scoped::Recipes, &template.ids(), t.account.id).await?;
    t.tx.commit().await?;
    Ok(Json(FromTemplateResponse {
        recipe_ids,
        template_name: template.name,
    }))
}

/// GET /api/sessions/:id/pdf: the client meal plan.
async fn meal_plan_pdf(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> DbResult<Response> {
    pdf::check_limit(&state, t.account.id).await?;
    let id = guard(&mut t, &raw_id).await?;
    let session = details_or_404(&mut t.tx, id).await?;
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    let bytes = pdf::render_blocking(
        Template::MealPlan,
        docs::meal_plan(&session, &settings),
        vec![],
    )
    .await?;
    Ok(pdf::response(bytes, &docs::meal_plan_filename(&session)))
}

/// POST /api/sessions/:id/email-pdf and POST /api/invoices/:id/email.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct EmailSentResponse {
    #[ts(type = "true")]
    pub success: bool,
    pub sent_to: String,
}

async fn email_pdf(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<EmailSentResponse>> {
    let id = guard(&mut t, &raw_id)
        .await
        .map_err(|e| e.restyle(crate::error::ErrorStyle::Db))?;
    email::check_limit(&state, t.account.id).await?;
    let Some(session) = session_with_details(&mut t.tx, id).await? else {
        return Err(AppError::bare(StatusCode::NOT_FOUND, "Session not found").into());
    };
    let client: Option<(Option<String>, String)> =
        sqlx::query_as("SELECT email, name FROM clients WHERE id = $1")
            .bind(session.session.client_id)
            .fetch_optional(&mut *t.tx)
            .await?;
    let Some((Some(to), name)) =
        client.filter(|(e, _)| e.as_deref().is_some_and(|e| !e.is_empty()))
    else {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "NO_EMAIL",
            "Client has no email address on file",
        )
        .into());
    };

    // The attachment is always rendered here: emailing bytes from the request
    // would let any account mail arbitrary files to any address.
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    let account_id = t.account.id;
    // Don't hold the transaction across rendering and SMTP (which may retry).
    t.tx.commit().await?;
    let pdf = pdf::render_blocking(
        Template::MealPlan,
        docs::meal_plan(&session, &settings),
        vec![],
    )
    .await?;
    let sent = email::send_meal_plan_pdf(
        &state,
        email::MealPlan {
            account_id,
            client_name: &name,
            client_email: &to,
            session_date: session.session.session_date,
            pdf,
            serving_size: session.session.serving_size,
        },
    )
    .await;

    // Remember the outcome, so the session page can show a failure (and offer
    // a retry) after the chef has left it.
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    sqlx::query(if sent.is_ok() {
        "UPDATE sessions SET pdf_emailed_at = now(), pdf_email_failed_at = NULL
          WHERE id = $1 AND account_id = $2"
    } else {
        "UPDATE sessions SET pdf_email_failed_at = now() WHERE id = $1 AND account_id = $2"
    })
    .bind(id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    sent.map_err(|e| {
        tracing::error!(error = %e, "[sessions] email-pdf");
        AppError::internal_bare(e)
    })?;
    Ok(Json(EmailSentResponse {
        success: true,
        sent_to: to,
    }))
}
