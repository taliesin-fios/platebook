// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/payments.js` + `server/services/paymentService.js`.
//!
//! Behaviour follows `PAYMENTS_MODE`:
//!   disabled — status display only; checkout returns 503
//!   manual   — chef records payments offline via /manual/action (default)
//!   stripe   — online checkout + webhook (`stripe.rs`)
//!   paypal / square — reserved (503)
//!
//! GET  /api/payments/mode
//! GET  /api/payments/status/:intake_id
//! POST /api/payments/checkout
//! POST /api/payments/webhook   (public; raw body, Stripe signature)
//! POST /api/payments/manual/action

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::accounts;
use crate::config::PaymentsMode;
use crate::db::{Tenant, begin_tenant};
use crate::error::{AppError, BareResult, is_unique_violation};
use crate::js;
use crate::models::IntakeRow;
use crate::routes::{JsonBody, body_id, parse_id};
use crate::state::AppState;
use crate::stripe;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mode", get(mode))
        .route("/status/{intake_id}", get(status))
        .route("/checkout", post(checkout))
        .route("/webhook", post(webhook))
        .route("/manual/action", post(manual_action))
}

/// Every payment_status value: manual statuses first, then legacy Stripe ones
/// (kept for existing rows). Order matters to the client's pickers.
const PAYMENT_STATUSES: &[(&str, &str, &str, &str)] = &[
    ("not_requested", "Not Requested", "gray", "manual"),
    ("deposit_requested", "Deposit Requested", "amber", "manual"),
    ("invoice_sent", "Invoice Sent", "blue", "manual"),
    ("payment_pending", "Payment Pending", "amber", "manual"),
    ("paid_offline", "Paid (Offline)", "green", "manual"),
    ("waived", "Waived", "purple", "manual"),
    ("unpaid", "Unpaid", "gray", "legacy"),
    ("pending", "Pending", "amber", "legacy"),
    ("paid", "✓ Paid", "green", "legacy"),
    ("refunded", "Refunded", "gray", "legacy"),
    ("failed", "Failed", "red", "legacy"),
];

pub struct ManualAction {
    pub name: &'static str,
    pub payment_status: &'static str,
    pub intake_status: Option<&'static str>,
    pub label: &'static str,
}

/// Manual admin actions and the state each produces.
pub const MANUAL_ACTIONS: &[ManualAction] = &[
    ManualAction {
        name: "request_deposit",
        payment_status: "deposit_requested",
        intake_status: None,
        label: "Request Deposit",
    },
    ManualAction {
        name: "mark_invoice_sent",
        payment_status: "invoice_sent",
        intake_status: None,
        label: "Mark Invoice Sent",
    },
    ManualAction {
        name: "mark_pending",
        payment_status: "payment_pending",
        intake_status: None,
        label: "Mark Payment Pending",
    },
    ManualAction {
        name: "mark_paid_offline",
        payment_status: "paid_offline",
        intake_status: Some("deposit_paid"),
        label: "Mark Deposit Paid",
    },
    ManualAction {
        name: "waive",
        payment_status: "waived",
        intake_status: None,
        label: "Waive Deposit",
    },
    ManualAction {
        name: "reset",
        payment_status: "not_requested",
        intake_status: None,
        label: "Reset / Clear",
    },
];

fn statuses_json() -> Value {
    Value::Object(
        PAYMENT_STATUSES
            .iter()
            .map(|(k, label, color, group)| {
                (
                    k.to_string(),
                    json!({ "label": label, "color": color, "group": group }),
                )
            })
            .collect(),
    )
}

fn actions_json() -> Value {
    Value::Object(
        MANUAL_ACTIONS
            .iter()
            .map(|a| {
                (
                    a.name.to_string(),
                    json!({ "payment_status": a.payment_status,
                            "intake_status": a.intake_status,
                            "label": a.label }),
                )
            })
            .collect(),
    )
}

async fn mode(State(state): State<AppState>) -> Json<Value> {
    let mode = state.config.payments_mode;
    Json(json!({
        "mode": mode.as_str(),
        "online_enabled": mode == PaymentsMode::Stripe,
        "statuses": statuses_json(),
        "actions": actions_json(),
    }))
}

/// GET /api/payments/status/:intake_id.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct PaymentStatusResponse {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(as = "crate::api_types::IntakeStatus")]
    pub status: Option<String>,
    #[ts(as = "crate::api_types::PaymentStatus")]
    pub payment_status: Option<String>,
    pub payment_notes: Option<String>,
    #[ts(type = "number | null")]
    pub booking_fee_amount: Option<Decimal>,
    #[serde(serialize_with = "crate::serde_util::ts::serialize")]
    #[ts(type = "string | null")]
    pub booking_fee_paid_at: Option<DateTime<Utc>>,
    #[sqlx(skip)]
    #[ts(type = "string")]
    pub mode: &'static str,
}

fn intake_not_found() -> AppError {
    AppError::bare(StatusCode::NOT_FOUND, "Intake not found")
}

async fn status(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<PaymentStatusResponse>> {
    let id = parse_id(&raw_id).ok_or_else(intake_not_found)?;
    let row = sqlx::query_as::<_, PaymentStatusResponse>(
        "SELECT id, name, status, payment_status, payment_notes,
                booking_fee_amount, booking_fee_paid_at
         FROM client_intakes WHERE id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(t.account.id)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(intake_not_found)?;
    t.tx.commit().await?;
    Ok(Json(PaymentStatusResponse {
        mode: state.config.payments_mode.as_str(),
        ..row
    }))
}

async fn checkout(
    State(state): State<AppState>,
    mut t: Tenant,
    body: JsonBody,
) -> BareResult<Response> {
    let mode = state.config.payments_mode;
    if mode != PaymentsMode::Stripe {
        return Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "Online payments are not enabled.",
                "detail": format!("Current mode: \"{}\". Set PAYMENTS_MODE=stripe and add Stripe API keys to enable online checkout.", mode.as_str()),
                "mode": mode.as_str(),
                "code": "PAYMENTS_DISABLED",
            })),
        )
            .into_response());
    }

    let account = t.account.id;
    if !body.truthy("intake_id") {
        return Err(AppError::bare(StatusCode::BAD_REQUEST, "intake_id is required").into());
    }
    let id = body_id(body.get("intake_id")).ok_or_else(intake_not_found)?;
    let intake = sqlx::query_as::<_, IntakeRow>(
        "SELECT * FROM client_intakes WHERE id = $1 AND account_id = $2 FOR UPDATE",
    )
    .bind(id)
    .bind(account)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(intake_not_found)?;
    if matches!(
        intake.payment_status.as_deref(),
        Some("paid" | "paid_offline" | "payment_pending" | "deposit_paid")
    ) {
        return Err(AppError::bare(
            StatusCode::BAD_REQUEST,
            "Booking fee already paid or in progress for this intake",
        )
        .into());
    }
    let settings = accounts::get_settings(&mut t.tx, account).await?;
    // An explicit 0 means "no deposit"; BOOKING_FEE_CENTS applies only when unset.
    let fee_cents = settings
        .deposit_cents
        .unwrap_or(state.config.booking_fee_cents);
    if fee_cents <= 0 {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "NO_DEPOSIT",
            "This chef does not take a booking deposit",
        )
        .into());
    }

    let session = stripe::create_checkout_session(
        &state,
        stripe::CheckoutRequest {
            account_id: account,
            intake_id: id,
            client_name: &intake.name,
            client_email: intake.email.as_deref(),
            fee_cents,
            cancellation_hours: settings.cancellation_hours.unwrap_or(48),
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "[stripe] createCheckout");
        AppError::internal_bare(e)
    })?;

    sqlx::query(
        "UPDATE client_intakes
         SET stripe_session_id = $1, payment_status = 'payment_pending',
             status = 'deposit_pending', booking_fee_amount = $2, updated_at = now()
         WHERE id = $3",
    )
    .bind(&session.id)
    .bind(crate::money::from_cents(fee_cents))
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(json!({ "checkout_url": session.url, "session_id": session.id })).into_response())
}

fn text(status: StatusCode, body: String) -> Response {
    (status, body).into_response()
}

/// POST /api/payments/webhook — public; the Stripe signature is the auth.
async fn webhook(State(state): State<AppState>, headers: HeaderMap, raw: Bytes) -> Response {
    let mode = state.config.payments_mode;
    if mode != PaymentsMode::Stripe {
        // Acknowledge so Stripe doesn't retry-storm a server not in stripe mode.
        return Json(json!({
            "received": true,
            "skipped": format!("payments mode is \"{}\"", mode.as_str()),
        }))
        .into_response();
    }
    let Some(secret) = state.config.stripe_webhook_secret.as_deref() else {
        tracing::error!("[stripe] STRIPE_WEBHOOK_SECRET not set — cannot verify webhook signature");
        return text(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Webhook secret not configured".into(),
        );
    };
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok());
    if let Err(e) = stripe::verify_signature(&raw, sig, secret, Utc::now().timestamp()) {
        tracing::warn!(error = %e, "[stripe] Webhook signature verification failed");
        return text(StatusCode::BAD_REQUEST, format!("Webhook Error: {e}"));
    }
    let event: Value = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(e) => return text(StatusCode::BAD_REQUEST, format!("Webhook Error: {e}")),
    };
    match handle_event(&state, &event).await {
        Ok(res) => Json(res).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "[stripe] Webhook handler error");
            AppError::internal_bare(e).into_response()
        }
    }
}

async fn handle_event(state: &AppState, event: &Value) -> Result<Value, sqlx::Error> {
    let event_id = event.get("id").and_then(Value::as_str).unwrap_or_default();
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let object = &event["data"]["object"];

    // Idempotency: skip already-processed events
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM payment_events WHERE stripe_event_id = $1)",
    )
    .bind(event_id)
    .fetch_one(&state.pool)
    .await?;
    if seen {
        return Ok(json!({ "received": true, "skipped": "already processed" }));
    }

    // The tenant comes from the intake named in the metadata.
    let intake_id = object["metadata"].get("intake_id").and_then(js::integer);
    let owner: Option<i64> = match intake_id {
        Some(id) => {
            sqlx::query_scalar("SELECT account_id FROM client_intakes WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.pool)
                .await?
        }
        None => None,
    };
    let Some(owner) = owner else {
        // payment_events.account_id is NOT NULL: an event that names no known
        // intake has no tenant to log under.
        tracing::warn!(
            event_id,
            event_type,
            "[stripe] event has no known intake; not logged"
        );
        return Ok(json!({ "received": true }));
    };
    let intake_id = intake_id.unwrap_or_default();

    // Status update + audit insert atomically (DI4)
    let mut tx = begin_tenant(&state.pool, owner).await?;
    match event_type {
        "checkout.session.completed" => {
            sqlx::query(
                "UPDATE client_intakes
                 SET payment_status = 'paid', status = 'deposit_paid',
                     stripe_payment_intent = $1, booking_fee_paid_at = now(), updated_at = now()
                 WHERE id = $2",
            )
            .bind(object.get("payment_intent").and_then(Value::as_str))
            .bind(intake_id)
            .execute(&mut *tx)
            .await?;
            tracing::info!(intake_id, "[stripe] Booking fee paid");
        }
        "payment_intent.payment_failed" => {
            sqlx::query(
                "UPDATE client_intakes SET payment_status = 'failed', status = 'submitted',
                     updated_at = now() WHERE id = $1",
            )
            .bind(intake_id)
            .execute(&mut *tx)
            .await?;
            tracing::info!(intake_id, "[stripe] Payment failed");
        }
        "charge.refunded" => {
            sqlx::query(
                "UPDATE client_intakes SET payment_status = 'refunded', updated_at = now()
                 WHERE id = $1",
            )
            .bind(intake_id)
            .execute(&mut *tx)
            .await?;
        }
        _ => {}
    }
    let amount = object
        .get("amount_total")
        .filter(|v| js::truthy(v))
        .and_then(Value::as_i64)
        .map(crate::money::from_cents);
    let status = object
        .get("payment_status")
        .filter(|v| js::truthy(v))
        .or_else(|| object.get("status").filter(|v| js::truthy(v)))
        .map(js::string);
    let inserted = sqlx::query(
        "INSERT INTO payment_events (account_id, intake_id, event_type, stripe_event_id, amount, status, raw_payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(owner)
    .bind(intake_id)
    .bind(event_type)
    .bind(event_id)
    .bind(amount)
    .bind(status)
    .bind(event)
    .execute(&mut *tx)
    .await;
    match inserted {
        Ok(_) => {}
        // A concurrent delivery of the same event won the race.
        Err(e) if is_unique_violation(&e) => {
            return Ok(json!({ "received": true, "skipped": "already processed" }));
        }
        Err(e) => return Err(e),
    }
    tx.commit().await?;
    Ok(json!({ "received": true }))
}

async fn manual_action(mut t: Tenant, body: JsonBody) -> BareResult<Json<IntakeRow>> {
    let account = t.account.id;
    if !body.truthy("intake_id") {
        return Err(AppError::bare(StatusCode::BAD_REQUEST, "intake_id is required").into());
    }
    let action_name = match body.get("action") {
        None => "undefined".to_string(),
        Some(v) => js::string(v),
    };
    let Some(action) = MANUAL_ACTIONS.iter().find(|a| a.name == action_name) else {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_ACTION",
            format!("Unknown action \"{action_name}\""),
        )
        .with(
            "valid_actions",
            MANUAL_ACTIONS.iter().map(|a| a.name).collect::<Vec<_>>(),
        )
        .into());
    };
    let id = body_id(body.get("intake_id")).ok_or_else(intake_not_found)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM client_intakes WHERE id = $1 AND account_id = $2)",
    )
    .bind(id)
    .bind(account)
    .fetch_one(&mut *t.tx)
    .await?;
    if !exists {
        return Err(intake_not_found().into());
    }

    let mut qb: QueryBuilder<Postgres> =
        QueryBuilder::new("UPDATE client_intakes SET updated_at = now(), payment_status = ");
    qb.push_bind(action.payment_status);
    // Overwrite payment notes when provided
    if let Some(n) = body.get("notes").filter(|v| !v.is_null()) {
        let notes = js::string(n).trim().to_string();
        qb.push(", payment_notes = ")
            .push_bind((!notes.is_empty()).then_some(notes));
    }
    // Record the deposit amount if provided — it must be a positive number
    let amount = match body.get("amount").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => {
            let n = js::number(v);
            if !n.is_finite() || n <= 0.0 {
                return Err(AppError::validation("amount must be a positive number").into());
            }
            js::decimal(n)
        }
    };
    if let Some(a) = amount {
        qb.push(", booking_fee_amount = ").push_bind(a);
    }
    if let Some(s) = action.intake_status {
        qb.push(", status = ").push_bind(s);
    }
    if action.name == "mark_paid_offline" {
        qb.push(", booking_fee_paid_at = now()");
    }
    if action.name == "reset" {
        qb.push(", booking_fee_paid_at = NULL");
    }
    qb.push(" WHERE id = ").push_bind(id);
    qb.build().execute(&mut *t.tx).await?;

    // Audit trail. Non-fatal: a failure is logged and rolled back to a savepoint.
    let mut sp = sqlx::Acquire::begin(&mut *t.tx).await?;
    let audit = sqlx::query(
        "INSERT INTO payment_events (account_id, intake_id, event_type, amount, status)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(account)
    .bind(id)
    .bind(format!("manual.{}", action.name))
    .bind(amount)
    .bind(action.payment_status)
    .execute(&mut *sp)
    .await;
    match audit {
        Ok(_) => sp.commit().await?,
        Err(e) => {
            tracing::error!(error = %e, "[payments] audit log insert failed");
            sp.rollback().await?;
        }
    }

    let updated = sqlx::query_as::<_, IntakeRow>("SELECT * FROM client_intakes WHERE id = $1")
        .bind(id)
        .fetch_one(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(updated))
}
