// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/intake.js` — client intake forms.
//!
//! Public (no auth):
//!   POST /api/intake              — submit ({ slug } picks the chef; omitted = account #1,
//!                                   for pre-account /intake/new links)
//!   GET  /api/intake/:id?token=   — read with the HMAC edit token
//!   PUT  /api/intake/:id?token=   — client-editable fields via the token
//!   POST /api/intake/:id/checkout?token= — pay the booking deposit online (Stripe)
//!   (token holders get a [`ClientIntakeView`]: never the chef's notes,
//!   payment details or email bookkeeping)
//! Chef (owning account; GET/PUT above also accept the owner's session):
//!   GET    /api/intake            — list this account's intakes (?deleted=1: deleted ones)
//!   POST   /api/intake/:id/convert — intake → client record
//!   POST   /api/intake/:id/resend-confirmation — email the client's confirmation again
//!   GET    /api/intake/:id/payments — payment timeline (payment_events)
//!   GET    /api/intake/:id/client-link — the client's booking-page link, to send again
//!   DELETE /api/intake/:id         — soft delete (sets deleted_at)
//!   POST   /api/intake/:id/restore — undo a delete
//!
//! A deleted intake is kept with its payment history. Only the owning chef
//! can still read it (and its timeline) and restore it; the client's edit
//! link and every other action answer 404 until it is restored.
//!
//! Public requests have no tenant yet: the owning account is looked up by
//! slug or intake id on the pool (the connecting user, which bypasses RLS on
//! Railway), and every read/write then runs under that account's tenant.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgConnection, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::accounts;
use crate::auth::MaybeAccount;
use crate::db::{Tenant, begin_tenant};
use crate::error::{AppError, BareResult};
use crate::intake_token;
use crate::js;
use crate::models::{Client, IntakeRow, SUCCESS, Success};
use crate::rate_limit::ClientIp;
use crate::routes::sessions::EmailSentResponse;
use crate::routes::settings::EMAIL_RE;
use crate::routes::{JsonBody, json_or_empty, parse_id};
use crate::serde_util::{json_array, opt_bool_as_int, ts};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(submit))
        .route("/{id}", get(read).put(update).delete(remove))
        .route("/{id}/convert", post(convert))
        .route("/{id}/resend-confirmation", post(resend_confirmation))
        .route("/{id}/payments", get(payments))
        .route("/{id}/checkout", post(checkout))
        .route("/{id}/client-link", get(client_link))
        .route("/{id}/restore", post(restore))
}

const LEGACY_ACCOUNT_ID: i64 = 1;

/// An intake plus the owning chef's booking slug.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct IntakeWithSlug {
    #[serde(flatten)]
    #[ts(flatten)]
    pub intake: IntakeRow,
    pub chef_slug: Option<String>,
}

/// GET and PUT /api/intake/:id with the client's edit token: what they filled
/// in on the booking form and where their booking stands. Never the chef's
/// internal or payment notes, Stripe ids, conversion or email bookkeeping.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ClientIntakeView {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub household_type: Option<String>,
    pub adults: Option<i32>,
    pub children: Option<i32>,
    pub preferred_serving_size: Option<i32>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub allergens: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub dietary_styles: Option<Value>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub liked_proteins: Option<Value>,
    pub disliked_foods: Option<String>,
    pub spice_tolerance: Option<String>,
    pub cuisine_preferences: Option<String>,
    pub preferred_day: Option<String>,
    pub preferred_time: Option<String>,
    pub scheduling_notes: Option<String>,
    pub kitchen_notes: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1 | null")]
    pub client_containers: Option<bool>,
    pub grocery_handling: Option<String>,
    pub number_of_meals: Option<i32>,
    pub notes_for_chef: Option<String>,
    #[serde(serialize_with = "opt_bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub terms_accepted: Option<bool>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub terms_accepted_at: Option<DateTime<Utc>>,
    #[ts(as = "crate::api_types::IntakeStatus")]
    pub status: Option<String>,
    /// Their own deposit: whether it's due, paid or waived, so their page can
    /// offer to pay online or say it's done. (The chef's payment notes stay
    /// private.)
    #[ts(as = "crate::api_types::PaymentStatus")]
    pub payment_status: Option<String>,
    #[ts(type = "number | null")]
    pub booking_fee_amount: Option<Decimal>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string | null")]
    pub booking_fee_paid_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub updated_at: Option<DateTime<Utc>>,
    pub chef_slug: Option<String>,
}

impl From<IntakeWithSlug> for ClientIntakeView {
    fn from(w: IntakeWithSlug) -> Self {
        let i = w.intake;
        ClientIntakeView {
            id: i.id,
            name: i.name,
            email: i.email,
            phone: i.phone,
            address: i.address,
            household_type: i.household_type,
            adults: i.adults,
            children: i.children,
            preferred_serving_size: i.preferred_serving_size,
            allergens: i.allergens,
            dietary_styles: i.dietary_styles,
            liked_proteins: i.liked_proteins,
            disliked_foods: i.disliked_foods,
            spice_tolerance: i.spice_tolerance,
            cuisine_preferences: i.cuisine_preferences,
            preferred_day: i.preferred_day,
            preferred_time: i.preferred_time,
            scheduling_notes: i.scheduling_notes,
            kitchen_notes: i.kitchen_notes,
            client_containers: i.client_containers,
            grocery_handling: i.grocery_handling,
            number_of_meals: i.number_of_meals,
            notes_for_chef: i.notes_for_chef,
            terms_accepted: i.terms_accepted,
            terms_accepted_at: i.terms_accepted_at,
            status: i.status,
            payment_status: i.payment_status,
            booking_fee_amount: i.booking_fee_amount,
            booking_fee_paid_at: i.booking_fee_paid_at,
            created_at: i.created_at,
            updated_at: i.updated_at,
            chef_slug: w.chef_slug,
        }
    }
}

/// The owning chef gets the whole intake; a token holder the client's view.
fn for_role(role: &Role, intake: IntakeWithSlug) -> Response {
    match role {
        Role::Chef => Json(intake).into_response(),
        Role::Client => Json(ClientIntakeView::from(intake)).into_response(),
    }
}

/// GET /api/intake/:id/client-link.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ClientLink {
    /// The client's booking page with its edit token.
    pub url: String,
}

/// GET /api/intake/:id/payments: one entry of the intake's payment timeline.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PaymentEventItem {
    #[ts(type = "number")]
    pub id: i64,
    /// `manual.<action>` or the Stripe event type.
    pub event_type: Option<String>,
    /// What happened, in words ("Mark Deposit Paid", "Paid online", …).
    pub label: String,
    #[ts(type = "number | null")]
    pub amount: Option<Decimal>,
    pub currency: Option<String>,
    /// The payment status the event set (manual) or Stripe reported.
    pub status: Option<String>,
    #[serde(serialize_with = "ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct PaymentEventRow {
    id: i64,
    event_type: Option<String>,
    amount: Option<Decimal>,
    currency: Option<String>,
    status: Option<String>,
    created_at: Option<DateTime<Utc>>,
}

/// A payment event type in words: the manual action's label, or a plain
/// name for the Stripe events the webhook handles.
fn payment_event_label(event_type: Option<&str>) -> String {
    let Some(t) = event_type else {
        return "Payment event".into();
    };
    if let Some(name) = t.strip_prefix("manual.") {
        return crate::routes::payments::MANUAL_ACTIONS
            .iter()
            .find(|a| a.name == name)
            .map_or_else(|| t.to_string(), |a| a.label.to_string());
    }
    match t {
        "checkout.session.completed" => "Paid online".into(),
        "payment_intent.payment_failed" => "Online payment failed".into(),
        "charge.refunded" => "Refunded".into(),
        other => other.to_string(),
    }
}

/// POST /api/intake (201).
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct IntakeSubmitted {
    #[serde(flatten)]
    #[ts(flatten)]
    pub intake: IntakeWithSlug,
    pub edit_token: String,
    pub edit_url: String,
    pub message: String,
}

/// POST /api/intake/:id/convert.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct IntakeConverted {
    pub client: Client,
    #[ts(type = "number")]
    pub intake_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "true")]
    pub already_converted: Option<bool>,
}

fn bare(status: StatusCode, msg: &str) -> AppError {
    AppError::bare(status, msg)
}

fn field_error(msg: &str, field: &str) -> AppError {
    bare(StatusCode::BAD_REQUEST, msg).with("field", field)
}

async fn with_slug(
    conn: &mut PgConnection,
    intake: IntakeRow,
) -> Result<IntakeWithSlug, sqlx::Error> {
    let chef_slug: Option<String> = sqlx::query_scalar("SELECT slug FROM accounts WHERE id = $1")
        .bind(intake.account_id)
        .fetch_optional(conn)
        .await?;
    Ok(IntakeWithSlug { intake, chef_slug })
}

/// Remembers how the client's confirmation email went, so the intake screens
/// can show a failure (and offer a retry) after the fact. A success clears an
/// earlier failure.
async fn record_confirmation(
    state: &AppState,
    account_id: i64,
    id: i64,
    sent: bool,
) -> Result<(), sqlx::Error> {
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    sqlx::query(if sent {
        "UPDATE client_intakes SET confirmation_emailed_at = now(), confirmation_email_failed_at = NULL
          WHERE id = $1 AND account_id = $2"
    } else {
        "UPDATE client_intakes SET confirmation_email_failed_at = now()
          WHERE id = $1 AND account_id = $2"
    })
    .bind(id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Remembers that the new-booking alert to the chef failed.
async fn record_chef_alert_failure(
    state: &AppState,
    account_id: i64,
    id: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    sqlx::query(
        "UPDATE client_intakes SET chef_alert_failed_at = now() WHERE id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

async fn fetch(conn: &mut PgConnection, id: i64) -> Result<Option<IntakeRow>, sqlx::Error> {
    sqlx::query_as::<_, IntakeRow>("SELECT * FROM client_intakes WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await
}

// ── GET /api/intake — chef list ──────────────────────────────────────────────
async fn list(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> BareResult<Json<Vec<IntakeRow>>> {
    let num = |k: &str| {
        q.get(k)
            .map(|v| js::number(&Value::String(v.clone())))
            .filter(|n| *n != 0.0 && n.is_finite())
    };
    let limit = num("limit").unwrap_or(50.0).clamp(1.0, 100.0) as i64;
    let offset = num("offset").unwrap_or(0.0).max(0.0) as i64;

    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT * FROM client_intakes WHERE account_id = ");
    qb.push_bind(t.account.id);
    qb.push(
        if q.get("deleted").is_some_and(|v| v == "1" || v == "true") {
            " AND deleted_at IS NOT NULL"
        } else {
            " AND deleted_at IS NULL"
        },
    );
    if let Some(s) = q.get("status").filter(|s| !s.is_empty()) {
        qb.push(" AND status = ").push_bind(s.clone());
    }
    if let Some(s) = q.get("payment_status").filter(|s| !s.is_empty()) {
        qb.push(" AND payment_status = ").push_bind(s.clone());
    }
    qb.push(" ORDER BY created_at DESC, id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let rows = qb
        .build_query_as::<IntakeRow>()
        .fetch_all(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

// ── POST /api/intake — public submission ─────────────────────────────────────
async fn submit(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: JsonBody,
) -> BareResult<(StatusCode, Json<IntakeSubmitted>)> {
    state
        .limits
        .intake
        .check(&state.pool, &ip)
        .await
        .map_err(|e| e.with_code())?;
    let mut conn = state.pool.acquire().await?;
    let account = match body.text_or_none("slug") {
        Some(slug) => accounts::get_account_by_slug(&mut conn, &slug).await?,
        None => accounts::get_account(&mut conn, LEGACY_ACCOUNT_ID).await?,
    };
    drop(conn);
    let Some(account) = account else {
        return Err(AppError::not_found_msg("This booking link is no longer active").into());
    };

    let Some(name) = body.trimmed_or_none("name") else {
        return Err(field_error("Name is required", "name").into());
    };
    if !body.truthy("terms_accepted") {
        return Err(field_error("You must accept the terms to submit", "terms_accepted").into());
    }
    if let Some(email) = body.text_or_none("email")
        && !EMAIL_RE.is_match(email.trim())
    {
        return Err(field_error("Enter a valid email address", "email").into());
    }

    let mut tx = begin_tenant(&state.pool, account.id).await?;
    let intake = sqlx::query_as::<_, IntakeRow>(
        "INSERT INTO client_intakes (
           account_id, name, email, phone, address,
           household_type, adults, children, preferred_serving_size,
           allergens, dietary_styles, liked_proteins,
           disliked_foods, spice_tolerance, cuisine_preferences,
           preferred_day, preferred_time, scheduling_notes,
           kitchen_notes, client_containers, grocery_handling, number_of_meals,
           notes_for_chef, terms_accepted, terms_accepted_at,
           status, payment_status, source
         ) VALUES (
           $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
           $19, $20, $21, $22, $23, $24, now(), 'submitted', 'not_requested', 'intake_form'
         ) RETURNING *",
    )
    .bind(account.id)
    .bind(&name)
    .bind(body.trimmed_or_none("email"))
    .bind(body.trimmed_or_none("phone"))
    .bind(body.trimmed_or_none("address"))
    // Household answers the client skipped stay blank: the chef shouldn't
    // read a guess as their answer.
    .bind(body.text_or_none("household_type"))
    .bind(body.opt_i64("adults").map(|n| n as i32))
    .bind(body.opt_i64("children").map(|n| n as i32))
    .bind(body.opt_i64("preferred_serving_size").map(|n| n as i32))
    .bind(json_or_empty(body.get("allergens")))
    .bind(json_or_empty(body.get("dietary_styles")))
    .bind(json_or_empty(body.get("liked_proteins")))
    .bind(body.text_or_none("disliked_foods"))
    .bind(body.text_or_none("spice_tolerance"))
    .bind(body.text_or_none("cuisine_preferences"))
    .bind(body.text_or_none("preferred_day"))
    .bind(body.text_or_none("preferred_time"))
    .bind(body.text_or_none("scheduling_notes"))
    .bind(body.text_or_none("kitchen_notes"))
    .bind(body.opt_flag("client_containers").unwrap_or(true))
    .bind(
        body.text_or_none("grocery_handling")
            .unwrap_or_else(|| "Client shops".into()),
    )
    .bind(
        body.get("number_of_meals")
            .filter(|v| js::truthy(v))
            .and_then(|_| body.opt_i64("number_of_meals"))
            .map(|n| n as i32),
    )
    .bind(body.text_or_none("notes_for_chef"))
    .bind(body.truthy("terms_accepted"))
    .fetch_one(&mut *tx)
    .await?;
    let res = with_slug(&mut tx, intake).await?;
    tx.commit().await?;

    let token = intake_token::make_token(&state.config.intake_secret, res.intake.id);
    let edit_url = format!("/intake/{}?token={token}", res.intake.id);

    // Emails are sent in the background (each with one automatic retry). The
    // outcome is recorded on the intake, so a failure shows on the chef's
    // intake screens instead of only in the log.
    let intake_id = res.intake.id;
    if let Some(email) = res.intake.email.clone().filter(|e| !e.is_empty()) {
        let (state, name, url, account_id) = (
            state.clone(),
            res.intake.name.clone(),
            edit_url.clone(),
            account.id,
        );
        tokio::spawn(async move {
            let sent =
                crate::email::send_intake_confirmation(&state, account_id, &name, &email, &url)
                    .await;
            if let Err(e) = &sent {
                tracing::error!(error = %e, "[intake] Client confirmation email failed");
            }
            if let Err(e) = record_confirmation(&state, account_id, intake_id, sent.is_ok()).await {
                tracing::error!(error = %e, "[intake] Could not record the confirmation email");
            }
        });
    }
    {
        let state = state.clone();
        let (account_id, name, email, phone) = (
            account.id,
            res.intake.name.clone(),
            res.intake.email.clone(),
            res.intake.phone.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = crate::email::send_chef_new_intake(
                &state,
                account_id,
                &name,
                email.as_deref(),
                phone.as_deref(),
                "/intake",
            )
            .await
            {
                tracing::error!(error = %e, "[intake] Chef notification email failed");
                if let Err(e) = record_chef_alert_failure(&state, account_id, intake_id).await {
                    tracing::error!(error = %e, "[intake] Could not record the chef alert failure");
                }
            }
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(IntakeSubmitted {
            intake: res,
            edit_token: token,
            edit_url,
            message: "Intake submitted successfully. Your chef will be in touch soon.".into(),
        }),
    ))
}

#[derive(PartialEq)]
enum Role {
    Chef,
    Client,
}

/// The owning chef (signed in) gets full access; anyone else needs the HMAC
/// edit token. Returns the role and the owning account (if the intake exists).
async fn access(
    state: &AppState,
    who: &MaybeAccount,
    raw_id: &str,
    q: &HashMap<String, String>,
) -> Result<(Role, Option<i64>), AppError> {
    let owner: Option<i64> = match parse_id(raw_id) {
        Some(id) => {
            sqlx::query_scalar("SELECT account_id FROM client_intakes WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.pool)
                .await?
        }
        None => None,
    };
    if let (Some(account), Some(owner)) = (&who.0, owner)
        && account.id == owner
    {
        return Ok((Role::Chef, Some(owner)));
    }
    let token = q.get("token").map(String::as_str).unwrap_or("");
    if token.is_empty() {
        return Err(bare(StatusCode::UNAUTHORIZED, "Authentication required"));
    }
    if !intake_token::verify_token(&state.config.intake_secret, raw_id, token) {
        return Err(bare(StatusCode::FORBIDDEN, "Invalid or expired link"));
    }
    Ok((Role::Client, owner))
}

fn intake_not_found() -> AppError {
    bare(StatusCode::NOT_FOUND, "Intake not found")
}

// ── GET /api/intake/:id — owner or token ─────────────────────────────────────
async fn read(
    State(state): State<AppState>,
    who: MaybeAccount,
    Path(raw_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> BareResult<Response> {
    let (role, owner) = access(&state, &who, &raw_id, &q).await?;
    let (Some(owner), Some(id)) = (owner, parse_id(&raw_id)) else {
        return Err(intake_not_found().into());
    };
    let mut tx = begin_tenant(&state.pool, owner).await?;
    let row = fetch(&mut tx, id).await?.ok_or_else(intake_not_found)?;
    // The chef can still open a deleted intake (to restore it); its client can't.
    if role == Role::Client && row.deleted_at.is_some() {
        return Err(intake_not_found().into());
    }
    let res = with_slug(&mut tx, row).await?;
    tx.commit().await?;
    Ok(for_role(&role, res))
}

const CLIENT_TEXT: &[&str] = &[
    "name",
    "email",
    "phone",
    "address",
    "household_type",
    "disliked_foods",
    "spice_tolerance",
    "cuisine_preferences",
    "preferred_day",
    "preferred_time",
    "scheduling_notes",
    "kitchen_notes",
    "grocery_handling",
    "notes_for_chef",
];
const CLIENT_INT: &[&str] = &[
    "adults",
    "children",
    "preferred_serving_size",
    "number_of_meals",
];
const CLIENT_JSON: &[&str] = &["allergens", "dietary_styles", "liked_proteins"];
/// Admin-only: never accepted with a bare token.
const ADMIN_TEXT: &[&str] = &[
    "status",
    "payment_status",
    "payment_notes",
    "internal_notes",
    "pdf_path",
];

// ── PUT /api/intake/:id — token (client) or owner (chef) ─────────────────────
async fn update(
    State(state): State<AppState>,
    who: MaybeAccount,
    Path(raw_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    body: JsonBody,
) -> BareResult<Response> {
    let (role, owner) = access(&state, &who, &raw_id, &q).await?;
    let (Some(owner), Some(id)) = (owner, parse_id(&raw_id)) else {
        return Err(intake_not_found().into());
    };

    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("UPDATE client_intakes SET updated_at = now()");
    for &f in CLIENT_TEXT {
        if body.has(f) {
            qb.push(format!(", {f} = ")).push_bind(body.opt_text(f));
        }
    }
    for &f in CLIENT_INT {
        if body.has(f) {
            qb.push(format!(", {f} = "))
                .push_bind(body.opt_i64(f).map(|n| n as i32));
        }
    }
    if let Some(v) = body.get("client_containers") {
        qb.push(", client_containers = ")
            .push_bind((!v.is_null()).then(|| js::truthy(v)));
    }
    for &f in CLIENT_JSON {
        if let Some(v) = body.get(f) {
            qb.push(format!(", {f} = ")).push_bind(v.clone());
        }
    }
    if role == Role::Chef {
        for &f in ADMIN_TEXT {
            if body.has(f) {
                qb.push(format!(", {f} = ")).push_bind(body.opt_text(f));
            }
        }
    }
    qb.push(" WHERE id = ")
        .push_bind(id)
        .push(" AND deleted_at IS NULL");

    let mut tx = begin_tenant(&state.pool, owner).await?;
    if qb.build().execute(&mut *tx).await?.rows_affected() == 0 {
        // Gone, or deleted: restore it before editing.
        return Err(intake_not_found().into());
    }
    let row = fetch(&mut tx, id).await?.ok_or_else(intake_not_found)?;
    let res = with_slug(&mut tx, row).await?;
    tx.commit().await?;
    Ok(for_role(&role, res))
}

/// The client row as returned by convert: arrays taken from the intake.
fn client_with_intake_arrays(client: Client, intake: &IntakeRow) -> Client {
    Client {
        allergens: intake.allergens.clone(),
        dietary_styles: intake.dietary_styles.clone(),
        liked_proteins: intake.liked_proteins.clone(),
        ..client
    }
}

// ── POST /api/intake/:id/convert — intake → client ───────────────────────────
async fn convert(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<(StatusCode, Json<IntakeConverted>)> {
    let account = t.account.id;
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    let intake = sqlx::query_as::<_, IntakeRow>(
        "SELECT * FROM client_intakes
          WHERE id = $1 AND account_id = $2 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .bind(account)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(intake_not_found)?;

    // Idempotency: return the existing client if already converted
    if let Some(cid) = intake.converted_client_id {
        let existing =
            sqlx::query_as::<_, Client>("SELECT * FROM clients WHERE id = $1 AND account_id = $2")
                .bind(cid)
                .bind(account)
                .fetch_optional(&mut *t.tx)
                .await?;
        if let Some(client) = existing {
            t.tx.commit().await?;
            return Ok((
                StatusCode::OK,
                Json(IntakeConverted {
                    client: client_with_intake_arrays(client, &intake),
                    intake_id: intake.id,
                    already_converted: Some(true),
                }),
            ));
        }
    }

    let notes: Vec<&str> = [
        &intake.notes_for_chef,
        &intake.kitchen_notes,
        &intake.scheduling_notes,
    ]
    .into_iter()
    .filter_map(|n| n.as_deref().filter(|s| !s.is_empty()))
    .collect();
    let chef_notes = (!notes.is_empty()).then(|| notes.join("\n\n"));
    let client = sqlx::query_as::<_, Client>(
        "INSERT INTO clients (
           account_id, name, phone, email, address,
           household_type, adults, children, serving_size,
           allergens, dietary_styles, liked_proteins,
           disliked_foods, spice_tolerance,
           client_containers, preferred_day, preferred_time,
           grocery_handling, chef_notes, is_active
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                   $18, $19, true)
         RETURNING *",
    )
    .bind(account)
    .bind(&intake.name)
    .bind(&intake.phone)
    .bind(&intake.email)
    .bind(&intake.address)
    .bind(&intake.household_type)
    .bind(intake.adults)
    .bind(intake.children)
    .bind(
        intake
            .preferred_serving_size
            .filter(|n| *n != 0)
            .unwrap_or(4),
    )
    .bind(intake.allergens.clone().unwrap_or(Value::Array(vec![])))
    .bind(
        intake
            .dietary_styles
            .clone()
            .unwrap_or(Value::Array(vec![])),
    )
    .bind(
        intake
            .liked_proteins
            .clone()
            .unwrap_or(Value::Array(vec![])),
    )
    .bind(&intake.disliked_foods)
    .bind(&intake.spice_tolerance)
    .bind(intake.client_containers)
    .bind(&intake.preferred_day)
    .bind(&intake.preferred_time)
    .bind(&intake.grocery_handling)
    .bind(chef_notes)
    .fetch_one(&mut *t.tx)
    .await?;
    sqlx::query(
        "UPDATE client_intakes SET converted_client_id = $1, status = 'booked', updated_at = now()
         WHERE id = $2 AND account_id = $3",
    )
    .bind(client.id)
    .bind(id)
    .bind(account)
    .execute(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(IntakeConverted {
            client: client_with_intake_arrays(client, &intake),
            intake_id: intake.id,
            already_converted: None,
        }),
    ))
}

// ── POST /api/intake/:id/resend-confirmation — chef ──────────────────────────
async fn resend_confirmation(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<EmailSentResponse>> {
    let account_id = t.account.id;
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, email FROM client_intakes
              WHERE id = $1 AND account_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(account_id)
    .fetch_optional(&mut *t.tx)
    .await?;
    let (name, email) = row.ok_or_else(intake_not_found)?;
    let Some(to) = email.filter(|e| !e.is_empty()) else {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "NO_EMAIL",
            "This inquiry has no email address",
        )
        .into());
    };
    // Don't hold the transaction across SMTP (which may retry).
    t.tx.commit().await?;
    crate::email::check_limit(&state, account_id).await?;

    let token = intake_token::make_token(&state.config.intake_secret, id);
    let url = format!("/intake/{id}?token={token}");
    let sent = crate::email::send_intake_confirmation(&state, account_id, &name, &to, &url).await;
    record_confirmation(&state, account_id, id, sent.is_ok()).await?;
    sent.map_err(|e| {
        tracing::error!(error = %e, "[intake] resend-confirmation");
        AppError::internal_bare(e)
    })?;
    Ok(Json(EmailSentResponse {
        success: true,
        sent_to: to,
    }))
}

// ── DELETE /api/intake/:id ───────────────────────────────────────────────────
async fn remove(mut t: Tenant, Path(raw_id): Path<String>) -> BareResult<Json<Success>> {
    let not_found = || AppError::not_found_msg("Intake not found");
    let id = parse_id(&raw_id).ok_or_else(not_found)?;
    // Soft delete: the intake and its payment events stay, so the lead's
    // history survives and POST /restore can bring it back.
    let res = sqlx::query(
        "UPDATE client_intakes SET deleted_at = now(), updated_at = now()
          WHERE id = $1 AND account_id = $2 AND deleted_at IS NULL",
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

// ── POST /api/intake/:id/checkout — client (token) or owner ──────────────────
/// Pays the booking deposit online: a fresh Stripe Checkout session each
/// time, so a link the client keeps never goes stale.
async fn checkout(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    who: MaybeAccount,
    Path(raw_id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> BareResult<Response> {
    state
        .limits
        .checkout
        .check(&state.pool, &ip)
        .await
        .map_err(|e| e.with_code())?;
    let (_, owner) = access(&state, &who, &raw_id, &q).await?;
    let (Some(owner), Some(id)) = (owner, parse_id(&raw_id)) else {
        return Err(intake_not_found().into());
    };
    crate::routes::payments::start_checkout(&state, owner, id).await
}

// ── GET /api/intake/:id/client-link — chef ───────────────────────────────────
/// The client's own booking-page link, for the chef to send again: the
/// client can review their answers there and, with online payments on, pay
/// the deposit.
async fn client_link(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<ClientLink>> {
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM client_intakes
          WHERE id = $1 AND account_id = $2 AND deleted_at IS NULL)",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_one(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    if !exists {
        return Err(intake_not_found().into());
    }
    let token = intake_token::make_token(&state.config.intake_secret, id);
    Ok(Json(ClientLink {
        url: format!("{}/intake/{id}?token={token}", state.config.client_url),
    }))
}

// ── POST /api/intake/:id/restore — undo a delete ─────────────────────────────
async fn restore(mut t: Tenant, Path(raw_id): Path<String>) -> BareResult<Json<IntakeWithSlug>> {
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    let row = sqlx::query_as::<_, IntakeRow>(
        "UPDATE client_intakes SET deleted_at = NULL, updated_at = now()
          WHERE id = $1 AND account_id = $2 AND deleted_at IS NOT NULL
          RETURNING *",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(intake_not_found)?;
    let res = with_slug(&mut t.tx, row).await?;
    t.tx.commit().await?;
    Ok(Json(res))
}

// ── GET /api/intake/:id/payments — payment timeline ──────────────────────────
async fn payments(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<Vec<PaymentEventItem>>> {
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    // Deleted intakes included: the history is what soft delete keeps.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM client_intakes WHERE id = $1 AND account_id = $2)",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_one(&mut *t.tx)
    .await?;
    if !exists {
        return Err(intake_not_found().into());
    }
    let rows = sqlx::query_as::<_, PaymentEventRow>(
        "SELECT id, event_type, amount, currency, status, created_at FROM payment_events
          WHERE intake_id = $1 AND account_id = $2 ORDER BY created_at, id",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| PaymentEventItem {
                label: payment_event_label(r.event_type.as_deref()),
                id: r.id,
                event_type: r.event_type,
                amount: r.amount,
                currency: r.currency,
                status: r.status,
                created_at: r.created_at,
            })
            .collect(),
    ))
}
