// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/invoices.js`.
//!
//! GET    /api/invoices                          — list (client_id, session_id, status)
//! GET    /api/invoices/:id                      — one invoice with line items
//! POST   /api/invoices                          — blank invoice
//! POST   /api/invoices/from-session/:session_id
//! POST   /api/invoices/from-client/:client_id
//! PUT    /api/invoices/:id                      — full update (paid/void are terminal)
//! POST   /api/invoices/:id/email                — email the PDF; draft → sent
//! DELETE /api/invoices/:id                      — paid: refused; void: soft delete
//! GET    /api/invoices/:id/pdf                  — the invoice as a PDF
//!
//! Money is integer-cents math (`money.rs`); totals are derived from line items
//! when present, else from the service fee + groceries + tax.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, PgConnection, Postgres, QueryBuilder};
use ts_rs::TS;

use crate::accounts::{self, Settings};
use crate::db::{Tenant, begin_tenant};
use crate::email;
use crate::error::{AppError, BareResult};
use crate::js;
use crate::models::{InvoiceLineItem, InvoiceRow};
use crate::money;
use crate::ownership::{Scoped, owned};
use crate::pdf::{self, Template, docs};
use crate::routes::sessions::{EmailSentResponse, body_date};
use crate::routes::{JsonBody, body_id, parse_id};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/email", post(email_invoice))
        .route("/{id}/pdf", get(invoice_pdf))
        .route("/from-session/{session_id}", post(from_session))
        .route("/from-client/{client_id}", post(from_client))
}

/// GET /api/invoices/:id and every invoice mutation.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct InvoiceDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub invoice: InvoiceRow,
    pub client_name_ref: Option<String>,
    pub client_email_ref: Option<String>,
    pub line_items: Vec<InvoiceLineItem>,
}

#[derive(FromRow)]
struct DetailRow {
    #[sqlx(flatten)]
    invoice: InvoiceRow,
    client_name_ref: Option<String>,
    client_email_ref: Option<String>,
}

/// GET /api/invoices row.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct InvoiceListItem {
    #[ts(type = "number")]
    pub id: i64,
    pub invoice_number: Option<String>,
    #[ts(as = "crate::api_types::InvoiceStatus")]
    pub status: Option<String>,
    #[ts(type = "string | null")]
    pub issue_date: Option<NaiveDate>,
    #[ts(type = "string | null")]
    pub due_date: Option<NaiveDate>,
    pub client_name: Option<String>,
    #[ts(type = "number")]
    pub total: Option<Decimal>,
    #[serde(serialize_with = "crate::serde_util::ts::serialize")]
    #[ts(type = "string | null")]
    pub paid_at: Option<chrono::DateTime<Utc>>,
    #[serde(serialize_with = "crate::serde_util::ts::serialize")]
    #[ts(type = "string")]
    pub created_at: Option<chrono::DateTime<Utc>>,
    #[ts(type = "number | null")]
    pub client_id: Option<i64>,
    #[ts(type = "number | null")]
    pub session_id: Option<i64>,
    pub client_name_ref: Option<String>,
    /// Last failed email of this invoice (cleared by a successful one).
    #[serde(serialize_with = "crate::serde_util::ts::serialize")]
    #[ts(type = "string | null")]
    pub email_failed_at: Option<chrono::DateTime<Utc>>,
}

fn invoice_not_found() -> AppError {
    AppError::not_found_msg("Invoice not found")
}

/// The `router.param('id')` guard.
async fn guard(t: &mut Tenant, raw_id: &str) -> Result<i64, AppError> {
    let id = parse_id(raw_id).ok_or_else(invoice_not_found)?;
    if !owned(&mut t.tx, Scoped::Invoices, id, t.account.id).await? {
        return Err(invoice_not_found());
    }
    Ok(id)
}

/// Port of `getInvoice(db, id)`.
async fn get_invoice(
    conn: &mut PgConnection,
    id: i64,
) -> Result<Option<InvoiceDetail>, sqlx::Error> {
    let Some(row) = sqlx::query_as::<_, DetailRow>(
        "SELECT i.*, c.name AS client_name_ref, c.email AS client_email_ref
         FROM invoices i LEFT JOIN clients c ON c.id = i.client_id
         WHERE i.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let line_items = sqlx::query_as::<_, InvoiceLineItem>(
        "SELECT * FROM invoice_line_items WHERE invoice_id = $1 ORDER BY sort_order ASC, id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Some(InvoiceDetail {
        invoice: row.invoice,
        client_name_ref: row.client_name_ref,
        client_email_ref: row.client_email_ref,
        line_items,
    }))
}

async fn invoice_or_404(conn: &mut PgConnection, id: i64) -> Result<InvoiceDetail, AppError> {
    get_invoice(conn, id)
        .await?
        .ok_or_else(|| AppError::bare(StatusCode::NOT_FOUND, "Invoice not found"))
}

/// `parseFloat(v) || 0` as money.
fn money_val(v: Option<&Value>) -> Decimal {
    v.and_then(js::parse_float)
        .and_then(js::decimal)
        .unwrap_or_default()
}

/// A numeric body value stored as-is (`total_hours = 0` default etc.).
fn num_or_zero(body: &JsonBody, key: &str) -> Decimal {
    body.opt_decimal(key).unwrap_or_default()
}

struct Totals {
    subtotal: Decimal,
    tax_amount: Decimal,
    total: Decimal,
}

fn compute_totals(service_fee: Decimal, grocery_total: Decimal, tax_rate: Decimal) -> Totals {
    let subtotal = money::add_money(&[service_fee, grocery_total]);
    let tax_amount = money::pct_of_money(subtotal, tax_rate);
    Totals {
        total: money::add_money(&[subtotal, tax_amount]),
        subtotal,
        tax_amount,
    }
}

/// A line item's amount: its `amount` when numeric, else quantity × unit
/// price. Used for both the stored line and the invoice totals, so they agree.
fn line_amount(li: &Value) -> Decimal {
    match li
        .get("amount")
        .and_then(js::parse_float)
        .filter(|f| f.is_finite())
    {
        Some(a) => money::round_money(js::decimal(a).unwrap_or_default()),
        None => {
            let num = |k: &str| {
                li.get(k)
                    .and_then(js::parse_float)
                    .filter(|f| f.is_finite())
                    .unwrap_or(0.0)
            };
            money::round_money(js::decimal(num("quantity") * num("unit_price")).unwrap_or_default())
        }
    }
}

/// The status in the body, if present: one of the four invoice statuses.
fn body_status(body: &JsonBody) -> Result<Option<String>, AppError> {
    match body.opt_text("status") {
        None => Ok(None),
        Some(s) if INVOICE_STATUSES.contains(&s.as_str()) => Ok(Some(s)),
        Some(_) => Err(AppError::validation(format!(
            "status must be one of: {}",
            INVOICE_STATUSES.join(", ")
        ))),
    }
}

const INVOICE_STATUSES: &[&str] = &["draft", "sent", "paid", "void"];

/// Totals from line items when there are any, else from the legacy fields.
fn body_totals(body: &JsonBody, line_items: &[Value]) -> Totals {
    let tax_rate = money_val(body.get("tax_rate"));
    if line_items.is_empty() {
        return compute_totals(
            money_val(body.get("service_fee")),
            money_val(body.get("grocery_total")),
            tax_rate,
        );
    }
    let amounts: Vec<Decimal> = line_items.iter().map(line_amount).collect();
    let subtotal = money::add_money(&amounts);
    let tax_amount = money::pct_of_money(subtotal, tax_rate);
    Totals {
        total: money::add_money(&[subtotal, tax_amount]),
        subtotal,
        tax_amount,
    }
}

fn line_items(body: &JsonBody) -> Vec<Value> {
    match body.get("line_items") {
        Some(Value::Array(a)) => a.clone(),
        _ => vec![],
    }
}

/// Replace all line items for an invoice (delete + re-insert).
async fn save_line_items(
    conn: &mut PgConnection,
    invoice_id: i64,
    items: &[Value],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM invoice_line_items WHERE invoice_id = $1")
        .bind(invoice_id)
        .execute(&mut *conn)
        .await?;
    for li in items {
        let qty = li.get("quantity").and_then(js::parse_float);
        let price = li.get("unit_price").and_then(js::parse_float);
        let (Some(qty), Some(price)) = (
            qty.filter(|f| f.is_finite()),
            price.filter(|f| f.is_finite()),
        ) else {
            return Err(AppError::validation(
                "Line item quantity and unit_price must be numeric",
            ));
        };
        let amount = line_amount(li);
        let sort_order = li
            .get("sort_order")
            .filter(|v| !v.is_null())
            .and_then(js::integer)
            .unwrap_or(0);
        let description = li
            .get("description")
            .filter(|v| js::truthy(v))
            .map(js::string)
            .unwrap_or_default();
        sqlx::query(
            "INSERT INTO invoice_line_items (invoice_id, sort_order, description, quantity, unit_price, amount)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(invoice_id)
        .bind(sort_order as i32)
        .bind(description)
        .bind(js::decimal(qty).unwrap_or_default())
        .bind(js::decimal(price).unwrap_or_default())
        .bind(amount)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The account's invoice prefix: letters/digits only, at most 8, default INV.
pub fn invoice_prefix(settings: &Settings) -> String {
    let p: String = settings
        .invoice_prefix
        .clone()
        .unwrap_or_default()
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        .take(8)
        .collect();
    if p.is_empty() { "INV".into() } else { p }
}

/// SQLite `CAST(x AS INTEGER)`: the leading integer, else 0.
fn sqlite_int(s: &str) -> i64 {
    js::parse_int(&Value::String(s.to_string())).unwrap_or(0)
}

/// Next `PREFIX-YEAR-0001` for this account. Takes a per-account advisory lock
/// for the rest of the transaction so concurrent creates can't collide.
async fn next_invoice_number(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<String, sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(7101, $1::int)")
        .bind(account_id as i32)
        .execute(&mut *conn)
        .await?;
    let settings = accounts::get_settings(&mut *conn, account_id).await?;
    let prefix = format!("{}-{}-", invoice_prefix(&settings), Utc::now().year());
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT invoice_number FROM invoices WHERE account_id = $1 AND invoice_number LIKE $2",
    )
    .bind(account_id)
    .bind(format!("{prefix}%"))
    .fetch_all(&mut *conn)
    .await?;
    // MAX of the numeric suffix, so deletions never cause number reuse
    let max = existing
        .iter()
        .map(|n| sqlite_int(&n[prefix.len()..]))
        .max()
        .unwrap_or(0);
    Ok(format!("{prefix}{:04}", max + 1))
}

/// "2026-03-15" → "Sunday, March 15, 2026".
fn format_date_label(d: Option<NaiveDate>) -> String {
    d.map(|d| d.format("%A, %B %-d, %Y").to_string())
        .unwrap_or_default()
}

/// "09:00" + 3.5 hours → "12:30" (24-hour wrap).
fn add_hours_to_time(time: &str, hours: Decimal) -> String {
    let mut parts = time.split(':');
    let (Some(h), Some(m)) = (parts.next(), parts.next()) else {
        return String::new();
    };
    let (Ok(h), Ok(m)) = (h.trim().parse::<i64>(), m.trim().parse::<i64>()) else {
        return String::new();
    };
    let add = (hours * Decimal::from(60))
        .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    let total = h * 60 + m + i64::try_from(add).unwrap_or(0);
    format!("{:02}:{:02}", (total / 60) % 24, total % 60)
}

/// Issue date (the account's local today) and due date, 14 days later.
async fn today_and_due(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<(NaiveDate, NaiveDate), sqlx::Error> {
    let today = accounts::local_clock(conn, account_id).await?.today;
    Ok((today, today + Duration::days(14)))
}

async fn list(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> BareResult<Json<Vec<InvoiceListItem>>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT i.id, i.invoice_number, i.status, i.issue_date, i.due_date,
                i.client_name, i.total, i.paid_at, i.created_at, i.client_id, i.session_id,
                c.name AS client_name_ref, i.email_failed_at
         FROM invoices i LEFT JOIN clients c ON c.id = i.client_id
         WHERE i.deleted_at IS NULL AND i.account_id = ",
    );
    qb.push_bind(t.account.id);
    if let Some(c) = q.get("client_id").filter(|c| !c.is_empty()) {
        qb.push(" AND i.client_id = ")
            .push_bind(parse_id(c).unwrap_or(-1));
    }
    if let Some(s) = q.get("session_id").filter(|s| !s.is_empty()) {
        qb.push(" AND i.session_id = ")
            .push_bind(parse_id(s).unwrap_or(-1));
    }
    if let Some(s) = q.get("status").filter(|s| !s.is_empty()) {
        qb.push(" AND i.status = ").push_bind(s.clone());
    }
    qb.push(" ORDER BY i.created_at DESC, i.id DESC");
    let rows = qb
        .build_query_as::<InvoiceListItem>()
        .fetch_all(&mut *t.tx)
        .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

async fn detail(mut t: Tenant, Path(raw_id): Path<String>) -> BareResult<Json<InvoiceDetail>> {
    let id = guard(&mut t, &raw_id).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(inv))
}

async fn create(mut t: Tenant, body: JsonBody) -> BareResult<(StatusCode, Json<InvoiceDetail>)> {
    let account = t.account.id;
    let client_id = body
        .truthy("client_id")
        .then(|| body_id(body.get("client_id")))
        .flatten();
    if body.truthy("client_id")
        && !matches!(client_id, Some(c) if owned(&mut t.tx, Scoped::Clients, c, account).await?)
    {
        return Err(AppError::validation("Client not found").into());
    }
    let session_id = body
        .truthy("session_id")
        .then(|| body_id(body.get("session_id")))
        .flatten();
    if body.truthy("session_id")
        && !matches!(session_id, Some(s) if owned(&mut t.tx, Scoped::Sessions, s, account).await?)
    {
        return Err(AppError::validation("Session not found").into());
    }
    let settings = accounts::get_settings(&mut t.tx, account).await?;
    let items = line_items(&body);
    let totals = body_totals(&body, &items);
    let status = body_status(&body)?.unwrap_or_else(|| "draft".into());
    let number = next_invoice_number(&mut t.tx, account).await?;
    let text = |k: &str| body.text_or_empty(k);
    let business = body
        .text_or_none("business_name")
        .or(settings.business_name.clone().filter(|b| !b.is_empty()))
        .unwrap_or_default();

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO invoices (
           account_id, client_id, session_id, invoice_number, status,
           issue_date, due_date,
           client_name, client_email, client_address, client_phone,
           business_name, chef_name, chef_email, chef_phone,
           message, internal_notes,
           session_date_label, recipes_summary,
           session_start_time, session_end_time, total_hours,
           container_count, store_trip, grocery_total, service_fee,
           subtotal, tax_rate, tax_amount, total, updated_at
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,
                   $23,$24,$25,$26,$27,$28,$29,$30, now())
         RETURNING id",
    )
    .bind(account)
    .bind(client_id)
    .bind(session_id)
    .bind(&number)
    .bind(status)
    .bind(body_date(&body, "issue_date")?)
    .bind(body_date(&body, "due_date")?)
    .bind(text("client_name"))
    .bind(text("client_email"))
    .bind(text("client_address"))
    .bind(text("client_phone"))
    .bind(business)
    .bind(text("chef_name"))
    .bind(text("chef_email"))
    .bind(text("chef_phone"))
    .bind(text("message"))
    .bind(text("internal_notes"))
    .bind(text("session_date_label"))
    .bind(body.get("recipes_summary").cloned().unwrap_or(json!([])))
    .bind(text("session_start_time"))
    .bind(text("session_end_time"))
    .bind(num_or_zero(&body, "total_hours"))
    .bind(body.opt_i64("container_count").unwrap_or(0) as i32)
    .bind(body.truthy("store_trip"))
    .bind(num_or_zero(&body, "grocery_total"))
    .bind(num_or_zero(&body, "service_fee"))
    .bind(totals.subtotal)
    .bind(num_or_zero(&body, "tax_rate"))
    .bind(totals.tax_amount)
    .bind(totals.total)
    .fetch_one(&mut *t.tx)
    .await?;
    save_line_items(&mut t.tx, id, &items).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(inv)))
}

#[derive(FromRow)]
struct SessionForInvoice {
    client_id: i64,
    session_date: Option<NaiveDate>,
    session_time: Option<String>,
    session_window_hours: Option<Decimal>,
    serving_size: Option<i32>,
    service_fee: Option<Decimal>,
    ingredient_cost: Option<Decimal>,
    client_name: Option<String>,
    client_email: Option<String>,
    client_address: Option<String>,
    client_phone: Option<String>,
    grocery_handling: Option<String>,
    client_serving: Option<i32>,
}

fn seed_item(sort: i64, description: &str, amount: Decimal) -> Value {
    json!({ "sort_order": sort, "description": description, "quantity": 1,
            "unit_price": amount, "amount": amount })
}

async fn from_session(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<(StatusCode, Json<InvoiceDetail>)> {
    let account = t.account.id;
    let not_found = || AppError::bare(StatusCode::NOT_FOUND, "Session not found");
    let session_id = parse_id(&raw_id).ok_or_else(not_found)?;
    let s = sqlx::query_as::<_, SessionForInvoice>(
        "SELECT s.client_id, s.session_date, s.session_time, s.session_window_hours,
                s.serving_size, s.service_fee, s.ingredient_cost,
                c.name AS client_name, c.email AS client_email,
                c.address AS client_address, c.phone AS client_phone,
                c.grocery_handling, c.serving_size AS client_serving
         FROM sessions s LEFT JOIN clients c ON c.id = s.client_id
         WHERE s.id = $1 AND s.account_id = $2",
    )
    .bind(session_id)
    .bind(account)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(not_found)?;
    let dishes: Vec<String> = sqlx::query_scalar(
        "SELECT r.name FROM session_dishes sd JOIN recipes r ON r.id = sd.recipe_id
         WHERE sd.session_id = $1 ORDER BY sd.sort_order, sd.id",
    )
    .bind(session_id)
    .fetch_all(&mut *t.tx)
    .await?;
    let settings = accounts::get_settings(&mut t.tx, account).await?;

    let start = s.session_time.clone().unwrap_or_default();
    let window = s
        .session_window_hours
        .filter(|w| !w.is_zero())
        .unwrap_or(Decimal::new(3, 0));
    let end = if start.is_empty() {
        String::new()
    } else {
        add_hours_to_time(&start, window)
    };
    // Container estimate: one container per portion of each dish
    let serving = s
        .serving_size
        .filter(|n| *n != 0)
        .or(s.client_serving.filter(|n| *n != 0))
        .unwrap_or(4);
    let containers = dishes.len() as i32 * serving;
    // Store trip if the chef shops
    let store_trip = matches!(s.grocery_handling.as_deref(), Some("Chef shops" | "Split"));
    let (today, due) = today_and_due(&mut t.tx, account).await?;
    let fee = s.service_fee.unwrap_or_default();
    let grocery = s.ingredient_cost.unwrap_or_default();
    let totals = compute_totals(fee, grocery, Decimal::ZERO);
    let number = next_invoice_number(&mut t.tx, account).await?;
    let opt = |v: &Option<String>| v.clone().unwrap_or_default();

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO invoices (
           account_id, client_id, session_id, invoice_number, status,
           issue_date, due_date,
           client_name, client_email, client_address, client_phone,
           business_name, chef_name, chef_email, chef_phone,
           session_date_label, recipes_summary,
           session_start_time, session_end_time, total_hours,
           container_count, store_trip, grocery_total, service_fee,
           subtotal, tax_rate, tax_amount, total, updated_at
         ) VALUES ($1,$2,$3,$4,'draft',$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,
                   $20,$21,$22,$23,$24,0,$25,$26, now())
         RETURNING id",
    )
    .bind(account)
    .bind(s.client_id)
    .bind(session_id)
    .bind(&number)
    .bind(today)
    .bind(due)
    .bind(opt(&s.client_name))
    .bind(opt(&s.client_email))
    .bind(opt(&s.client_address))
    .bind(opt(&s.client_phone))
    .bind(opt(&settings.business_name))
    .bind(opt(&settings.chef_name))
    .bind(opt(&settings.email))
    .bind(opt(&settings.phone))
    .bind(format_date_label(s.session_date))
    .bind(json!(dishes))
    .bind(&start)
    .bind(&end)
    .bind(window)
    .bind(containers)
    .bind(store_trip)
    .bind(grocery)
    .bind(fee)
    .bind(totals.subtotal)
    .bind(totals.tax_amount)
    .bind(totals.total)
    .fetch_one(&mut *t.tx)
    .await?;

    let mut seed = vec![];
    if fee > Decimal::ZERO {
        seed.push(seed_item(0, "Chef Service", fee));
    }
    if grocery > Decimal::ZERO {
        seed.push(seed_item(1, "Grocery Shopping", grocery));
    }
    save_line_items(&mut t.tx, id, &seed).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(inv)))
}

#[derive(FromRow)]
struct ClientForInvoice {
    name: String,
    email: Option<String>,
    address: Option<String>,
    phone: Option<String>,
}

async fn from_client(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<(StatusCode, Json<InvoiceDetail>)> {
    let account = t.account.id;
    let not_found = || AppError::bare(StatusCode::NOT_FOUND, "Client not found");
    let client_id = parse_id(&raw_id).ok_or_else(not_found)?;
    let c = sqlx::query_as::<_, ClientForInvoice>(
        "SELECT name, email, address, phone FROM clients WHERE id = $1 AND account_id = $2",
    )
    .bind(client_id)
    .bind(account)
    .fetch_optional(&mut *t.tx)
    .await?
    .ok_or_else(not_found)?;
    let settings = accounts::get_settings(&mut t.tx, account).await?;
    let (today, due) = today_and_due(&mut t.tx, account).await?;
    let fee = settings.default_service_fee.unwrap_or_default();
    let totals = compute_totals(fee, Decimal::ZERO, Decimal::ZERO);
    let number = next_invoice_number(&mut t.tx, account).await?;
    let opt = |v: &Option<String>| v.clone().unwrap_or_default();

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO invoices (
           account_id, client_id, invoice_number, status,
           issue_date, due_date,
           client_name, client_email, client_address, client_phone,
           business_name, chef_name, chef_email, chef_phone,
           recipes_summary, service_fee,
           subtotal, tax_rate, tax_amount, total, updated_at
         ) VALUES ($1,$2,$3,'draft',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'[]',$14,$15,0,$16,$17, now())
         RETURNING id",
    )
    .bind(account)
    .bind(client_id)
    .bind(&number)
    .bind(today)
    .bind(due)
    .bind(&c.name)
    .bind(opt(&c.email))
    .bind(opt(&c.address))
    .bind(opt(&c.phone))
    .bind(opt(&settings.business_name))
    .bind(opt(&settings.chef_name))
    .bind(opt(&settings.email))
    .bind(opt(&settings.phone))
    .bind(fee)
    .bind(totals.subtotal)
    .bind(totals.tax_amount)
    .bind(totals.total)
    .fetch_one(&mut *t.tx)
    .await?;
    if fee > Decimal::ZERO {
        save_line_items(&mut t.tx, id, &[seed_item(0, "Chef Service", fee)]).await?;
    }
    let inv = invoice_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok((StatusCode::CREATED, Json(inv)))
}

async fn update(
    mut t: Tenant,
    Path(raw_id): Path<String>,
    body: JsonBody,
) -> BareResult<Json<InvoiceDetail>> {
    let id = guard(&mut t, &raw_id).await?;
    let (existing_status, existing_paid): (Option<String>, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as("SELECT status, paid_at FROM invoices WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *t.tx)
            .await?;

    let status = body_status(&body)?;
    // paid and void are terminal: the status can't change once reached
    if let (Some(new), Some(old)) = (&status, &existing_status)
        && new != old
        && (old == "paid" || old == "void")
    {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "STATUS_LOCKED",
            format!("Cannot change the status of a {old} invoice"),
        )
        .into());
    }
    let items = line_items(&body);
    let totals = body_totals(&body, &items);
    // paid_at: kept when status is absent, stamped on first "paid", else cleared
    let paid_at = match status.as_deref() {
        None => existing_paid,
        Some("paid") if existing_status.as_deref() == Some("paid") => existing_paid,
        Some("paid") => Some(Utc::now()),
        Some(_) => None,
    };
    let recipes_summary = match body.get("recipes_summary") {
        Some(Value::Array(a)) => Value::Array(a.clone()),
        _ => json!([]),
    };
    let text = |k: &str| body.text_or_empty(k);

    sqlx::query(
        "UPDATE invoices SET
           status = $1, issue_date = $2, due_date = $3,
           client_name = $4, client_email = $5, client_address = $6, client_phone = $7,
           business_name = $8, chef_name = $9, chef_email = $10, chef_phone = $11,
           message = $12, internal_notes = $13,
           session_date_label = $14, recipes_summary = $15,
           session_start_time = $16, session_end_time = $17, total_hours = $18,
           container_count = $19, store_trip = $20,
           grocery_total = $21, service_fee = $22,
           subtotal = $23, tax_rate = $24, tax_amount = $25, total = $26,
           paid_at = $27, updated_at = now()
         WHERE id = $28",
    )
    .bind(status.or(existing_status))
    .bind(body_date(&body, "issue_date")?)
    .bind(body_date(&body, "due_date")?)
    .bind(text("client_name"))
    .bind(text("client_email"))
    .bind(text("client_address"))
    .bind(text("client_phone"))
    .bind(text("business_name"))
    .bind(text("chef_name"))
    .bind(text("chef_email"))
    .bind(text("chef_phone"))
    .bind(text("message"))
    .bind(text("internal_notes"))
    .bind(text("session_date_label"))
    .bind(recipes_summary)
    .bind(text("session_start_time"))
    .bind(text("session_end_time"))
    .bind(num_or_zero(&body, "total_hours"))
    .bind(body.opt_i64("container_count").unwrap_or(0) as i32)
    .bind(body.truthy("store_trip"))
    .bind(num_or_zero(&body, "grocery_total"))
    .bind(num_or_zero(&body, "service_fee"))
    .bind(totals.subtotal)
    .bind(num_or_zero(&body, "tax_rate"))
    .bind(totals.tax_amount)
    .bind(totals.total)
    .bind(paid_at)
    .bind(id)
    .execute(&mut *t.tx)
    .await?;
    save_line_items(&mut t.tx, id, &items).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    t.tx.commit().await?;
    Ok(Json(inv))
}

async fn email_invoice(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<EmailSentResponse>> {
    let id = guard(&mut t, &raw_id).await?;
    email::check_limit(&state, t.account.id).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    let Some(to) = inv.invoice.client_email.clone().filter(|e| !e.is_empty()) else {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "NO_EMAIL",
            "This invoice has no client email address",
        )
        .into());
    };
    // Rendered here, never taken from the request (see sessions::email_pdf).
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    let account_id = t.account.id;
    // Don't hold the transaction across rendering and SMTP (which may retry).
    t.tx.commit().await?;
    let (data, logo) = docs::invoice(&inv, &settings);
    let pdf = pdf::render_blocking(Template::Invoice, data, logo.into_iter().collect()).await?;
    let sent = email::send_invoice_pdf(
        &state,
        email::InvoiceMail {
            account_id,
            client_name: inv.invoice.client_name.as_deref().unwrap_or("there"),
            client_email: &to,
            invoice_number: inv.invoice.invoice_number.as_deref(),
            invoice_total: inv.invoice.total,
            due_date: inv.invoice.due_date,
            pdf,
            business_name: inv.invoice.business_name.as_deref(),
        },
    )
    .await;

    // Remember the outcome (see sessions::email_pdf). A successful send also
    // advances draft → sent, as before.
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    sqlx::query(if sent.is_ok() {
        "UPDATE invoices SET emailed_at = now(), email_failed_at = NULL,
                status = CASE WHEN status = 'draft' THEN 'sent' ELSE status END,
                updated_at = CASE WHEN status = 'draft' THEN now() ELSE updated_at END
          WHERE id = $1 AND account_id = $2"
    } else {
        "UPDATE invoices SET email_failed_at = now() WHERE id = $1 AND account_id = $2"
    })
    .bind(id)
    .bind(account_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    sent.map_err(|e| {
        tracing::error!(error = %e, "[invoices] email");
        AppError::internal_bare(e)
    })?;
    Ok(Json(EmailSentResponse {
        success: true,
        sent_to: to,
    }))
}

/// GET /api/invoices/:id/pdf: the saved invoice, laid out like the preview card.
async fn invoice_pdf(
    State(state): State<AppState>,
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Response> {
    pdf::check_limit(&state, t.account.id).await?;
    let id = guard(&mut t, &raw_id).await?;
    let inv = invoice_or_404(&mut t.tx, id).await?;
    let settings = accounts::get_settings(&mut t.tx, t.account.id).await?;
    t.tx.commit().await?;
    let (data, logo) = docs::invoice(&inv, &settings);
    let bytes = pdf::render_blocking(Template::Invoice, data, logo.into_iter().collect()).await?;
    Ok(pdf::response(bytes, &docs::invoice_filename(&inv)))
}

/// DELETE /api/invoices/:id. Void invoices are soft-deleted.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct InvoiceDeleteResponse {
    #[ts(type = "true")]
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "true")]
    pub soft_deleted: Option<bool>,
}

async fn remove(
    mut t: Tenant,
    Path(raw_id): Path<String>,
) -> BareResult<Json<InvoiceDeleteResponse>> {
    let id = guard(&mut t, &raw_id).await?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM invoices WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *t.tx)
            .await?;
    match status.as_deref() {
        // Paid invoices are financial records — never delete.
        Some("paid") => Err(AppError::new(
            StatusCode::CONFLICT,
            "STATUS_LOCKED",
            "Cannot delete a paid invoice — financial records must be retained",
        )
        .into()),
        // DI6: void invoices are soft-deleted so the audit trail is kept.
        Some("void") => {
            sqlx::query("UPDATE invoices SET deleted_at = now(), updated_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *t.tx)
                .await?;
            t.tx.commit().await?;
            Ok(Json(InvoiceDeleteResponse {
                success: true,
                soft_deleted: Some(true),
            }))
        }
        // Draft / sent: no financial event yet, hard delete is fine.
        _ => {
            sqlx::query("DELETE FROM invoices WHERE id = $1")
                .bind(id)
                .execute(&mut *t.tx)
                .await?;
            t.tx.commit().await?;
            Ok(Json(InvoiceDeleteResponse {
                success: true,
                soft_deleted: None,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_labels_and_end_times() {
        assert_eq!(
            format_date_label(NaiveDate::from_ymd_opt(2026, 3, 15)),
            "Sunday, March 15, 2026"
        );
        assert_eq!(format_date_label(None), "");
        assert_eq!(add_hours_to_time("09:00", Decimal::new(35, 1)), "12:30");
        assert_eq!(add_hours_to_time("22:30", Decimal::new(3, 0)), "01:30");
        assert_eq!(add_hours_to_time("9am", Decimal::new(3, 0)), "");
    }

    #[test]
    fn prefix_and_numbering_helpers() {
        let s = Settings {
            invoice_prefix: Some("bk-1!".into()),
            ..Settings::default()
        };
        assert_eq!(invoice_prefix(&s), "BK1");
        assert_eq!(invoice_prefix(&Settings::default()), "INV");
        assert_eq!(sqlite_int("0042"), 42);
        assert_eq!(sqlite_int("abc"), 0);
    }

    #[test]
    fn totals_use_cents() {
        let t = compute_totals(
            Decimal::new(10010, 2),
            Decimal::new(2005, 2),
            Decimal::new(825, 2),
        );
        assert_eq!(t.subtotal, Decimal::new(12015, 2));
        assert_eq!(t.tax_amount, Decimal::new(991, 2));
        assert_eq!(t.total, Decimal::new(13006, 2));
    }
}
