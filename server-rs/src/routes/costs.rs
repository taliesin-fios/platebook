// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/costs.js` plus the inline CSV export in
//! `server/index.js`.
//!
//! GET /api/costs/summary     — month-to-date + year-to-date totals, AR snapshot
//! GET /api/costs/sessions    — recent sessions with cost fields + invoice status
//! GET /api/costs/by-client   — per-client averages
//! GET /api/costs/export-csv  — CSV of all non-cancelled sessions (10 / 15 min per account)
//!
//! "Today" is the UTC date, as SQLite's `date('now')` was.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;
use sqlx::FromRow;
use ts_rs::TS;

use crate::accounts;
use crate::db::Tenant;
use crate::error::DbResult;
use crate::rate_limit::AccountLimits;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/summary", get(summary))
        .route("/sessions", get(sessions))
        .route("/by-client", get(by_client))
        .route("/export-csv", get(export_csv))
}

/// UTC "today", like SQLite `date('now')`.

#[derive(FromRow)]
struct Mtd {
    total_revenue: Decimal,
    total_cost: Decimal,
    gross_margin: Decimal,
    session_count: i64,
}

#[derive(FromRow)]
struct Ytd {
    ytd_revenue: Decimal,
    ytd_cost: Decimal,
    ytd_margin: Decimal,
    ytd_sessions: i64,
}

#[derive(FromRow)]
struct Ar {
    invoiced_total: Decimal,
    collected_total: Decimal,
    outstanding_total: Decimal,
    overdue_count: i64,
    collected_mtd: Decimal,
    collected_prev_mtd: Decimal,
}

/// GET /api/costs/summary.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CostSummary {
    #[ts(type = "number")]
    pub total_revenue: Decimal,
    #[ts(type = "number")]
    pub total_cost: Decimal,
    #[ts(type = "number")]
    pub gross_margin: Decimal,
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number")]
    pub margin_pct: Decimal,
    #[ts(type = "number")]
    pub invoiced_total: Decimal,
    #[ts(type = "number")]
    pub collected_total: Decimal,
    #[ts(type = "number")]
    pub collected_mtd: Decimal,
    /// Collected over the same days of last month (the 1st up to today's day
    /// number, or the month's end), for the dashboard's month-to-date trend.
    #[ts(type = "number")]
    pub collected_prev_mtd: Decimal,
    #[ts(type = "number")]
    pub outstanding_total: Decimal,
    #[ts(type = "number")]
    pub overdue_count: i64,
    #[ts(type = "number")]
    pub ytd_revenue: Decimal,
    #[ts(type = "number")]
    pub ytd_cost: Decimal,
    #[ts(type = "number")]
    pub ytd_margin: Decimal,
    #[ts(type = "number")]
    pub ytd_margin_pct: Decimal,
    #[ts(type = "number")]
    pub ytd_sessions: i64,
}

/// `parseFloat(((margin / revenue) * 100).toFixed(1))`, 0 without revenue.
pub fn pct(margin: Decimal, revenue: Decimal) -> Decimal {
    if revenue > Decimal::ZERO {
        (margin / revenue * Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
            .normalize()
    } else {
        Decimal::ZERO
    }
}

async fn summary(mut t: Tenant) -> DbResult<Json<CostSummary>> {
    let id = t.account.id;
    let clock = accounts::local_clock(&mut t.tx, id).await?;
    let mtd = sqlx::query_as::<_, Mtd>(
        "SELECT COALESCE(SUM(service_fee), 0)                   AS total_revenue,
                COALESCE(SUM(ingredient_cost), 0)               AS total_cost,
                COALESCE(SUM(service_fee - ingredient_cost), 0) AS gross_margin,
                COUNT(*)                                        AS session_count
         FROM sessions
         WHERE account_id = $1
           AND date_trunc('month', session_date::timestamp) = date_trunc('month', $2::timestamp)
           AND status != 'Cancelled'",
    )
    .bind(id)
    .bind(clock.today)
    .fetch_one(&mut *t.tx)
    .await?;
    let ytd = sqlx::query_as::<_, Ytd>(
        "SELECT COALESCE(SUM(service_fee), 0)                   AS ytd_revenue,
                COALESCE(SUM(ingredient_cost), 0)               AS ytd_cost,
                COALESCE(SUM(service_fee - ingredient_cost), 0) AS ytd_margin,
                COUNT(*)                                        AS ytd_sessions
         FROM sessions
         WHERE account_id = $1
           AND date_trunc('year', session_date::timestamp) = date_trunc('year', $2::timestamp)
           AND status != 'Cancelled'",
    )
    .bind(id)
    .bind(clock.today)
    .fetch_one(&mut *t.tx)
    .await?;
    let ar = sqlx::query_as::<_, Ar>(
        "SELECT
           COALESCE(SUM(CASE WHEN status != 'void' THEN total ELSE 0 END), 0)           AS invoiced_total,
           COALESCE(SUM(CASE WHEN status = 'paid' THEN total ELSE 0 END), 0)            AS collected_total,
           COALESCE(SUM(CASE WHEN status IN ('draft','sent') THEN total ELSE 0 END), 0) AS outstanding_total,
           COUNT(CASE WHEN status IN ('draft','sent') AND due_date < $2 THEN 1 END) AS overdue_count,
           COALESCE(SUM(CASE WHEN status = 'paid'
             AND date_trunc('month', paid_at AT TIME ZONE $3) = date_trunc('month', $2::timestamp)
             THEN total ELSE 0 END), 0) AS collected_mtd,
           COALESCE(SUM(CASE WHEN status = 'paid'
             AND (paid_at AT TIME ZONE $3)::date
                 BETWEEN (date_trunc('month', $2::date) - interval '1 month')::date
                     AND LEAST(
                           (date_trunc('month', $2::date) - interval '1 month')::date
                             + ($2::date - date_trunc('month', $2::date)::date),
                           date_trunc('month', $2::date)::date - 1)
             THEN total ELSE 0 END), 0) AS collected_prev_mtd
         FROM invoices
         WHERE account_id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(clock.today)
    .bind(&clock.tz)
    .fetch_one(&mut *t.tx)
    .await?;
    t.tx.commit().await?;

    Ok(Json(CostSummary {
        margin_pct: pct(mtd.gross_margin, mtd.total_revenue),
        total_revenue: mtd.total_revenue,
        total_cost: mtd.total_cost,
        gross_margin: mtd.gross_margin,
        session_count: mtd.session_count,
        invoiced_total: ar.invoiced_total,
        collected_total: ar.collected_total,
        collected_mtd: ar.collected_mtd,
        collected_prev_mtd: ar.collected_prev_mtd,
        outstanding_total: ar.outstanding_total,
        overdue_count: ar.overdue_count,
        ytd_margin_pct: pct(ytd.ytd_margin, ytd.ytd_revenue),
        ytd_revenue: ytd.ytd_revenue,
        ytd_cost: ytd.ytd_cost,
        ytd_margin: ytd.ytd_margin,
        ytd_sessions: ytd.ytd_sessions,
    }))
}

/// One row of GET /api/costs/sessions.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct CostSession {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "string | null")]
    pub session_date: Option<NaiveDate>,
    #[ts(as = "crate::api_types::SessionStatus")]
    pub status: Option<String>,
    #[ts(type = "number")]
    pub service_fee: Option<Decimal>,
    #[ts(type = "number")]
    pub ingredient_cost: Option<Decimal>,
    #[ts(type = "number")]
    pub gross_margin: Option<Decimal>,
    #[ts(type = "number")]
    pub margin_pct: Option<Decimal>,
    pub client_name: Option<String>,
    #[ts(type = "number | null")]
    pub invoice_id: Option<i64>,
    #[ts(as = "Option<crate::api_types::InvoiceStatus>")]
    pub invoice_status: Option<String>,
    #[ts(type = "number | null")]
    pub invoice_total: Option<Decimal>,
    #[ts(type = "string | null")]
    pub invoice_due_date: Option<NaiveDate>,
}

/// SQL for `(fee - cost) / fee * 100` rounded to 1 place, 0 without a fee.
pub const MARGIN_PCT_SQL: &str = "CASE WHEN s.service_fee > 0
       THEN ROUND(((s.service_fee - s.ingredient_cost) / s.service_fee) * 100, 1)
       ELSE 0 END";

async fn sessions(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Json<Vec<CostSession>>> {
    let days = q
        .get("days")
        .and_then(|d| crate::js::parse_int(&serde_json::Value::String(d.clone())))
        .filter(|d| *d != 0)
        .unwrap_or(30)
        .clamp(1, 365);
    let today = accounts::local_clock(&mut t.tx, t.account.id).await?.today;
    let rows = sqlx::query_as::<_, CostSession>(&format!(
        "SELECT s.id, s.session_date, s.status, s.service_fee, s.ingredient_cost,
                (s.service_fee - s.ingredient_cost) AS gross_margin,
                {MARGIN_PCT_SQL} AS margin_pct,
                c.name AS client_name,
                inv.id AS invoice_id, inv.status AS invoice_status, inv.total AS invoice_total,
                inv.due_date AS invoice_due_date
         FROM sessions s
         LEFT JOIN clients c ON c.id = s.client_id
         LEFT JOIN LATERAL (
           SELECT i.id, i.status, i.total, i.due_date FROM invoices i
           WHERE i.session_id = s.id AND i.status != 'void' AND i.deleted_at IS NULL
           ORDER BY i.id DESC LIMIT 1
         ) inv ON true
         WHERE s.account_id = $1
           AND s.session_date >= $3::date - $2::int
           AND s.status != 'Cancelled'
         ORDER BY s.session_date DESC NULLS LAST"
    ))
    .bind(t.account.id)
    .bind(days as i32)
    .bind(today)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

/// One row of GET /api/costs/by-client.
#[derive(Debug, Serialize, FromRow, TS)]
#[ts(export)]
pub struct CostByClient {
    #[ts(type = "number")]
    pub client_id: i64,
    pub client_name: String,
    #[ts(type = "number")]
    pub session_count: i64,
    #[ts(type = "number | null")]
    pub avg_fee: Option<Decimal>,
    #[ts(type = "number | null")]
    pub avg_cost: Option<Decimal>,
    #[ts(type = "number | null")]
    pub avg_margin: Option<Decimal>,
    #[ts(type = "number")]
    pub avg_margin_pct: Decimal,
}

async fn by_client(mut t: Tenant) -> DbResult<Json<Vec<CostByClient>>> {
    let rows = sqlx::query_as::<_, CostByClient>(
        "SELECT c.id AS client_id, c.name AS client_name,
                COUNT(s.id) AS session_count,
                ROUND(AVG(s.service_fee), 2) AS avg_fee,
                ROUND(AVG(s.ingredient_cost), 2) AS avg_cost,
                ROUND(AVG(s.service_fee - s.ingredient_cost), 2) AS avg_margin,
                CASE WHEN AVG(s.service_fee) > 0
                  THEN ROUND((AVG(s.service_fee - s.ingredient_cost) / AVG(s.service_fee)) * 100, 1)
                  ELSE 0 END AS avg_margin_pct
         FROM clients c
         LEFT JOIN sessions s ON s.client_id = c.id AND s.status != 'Cancelled'
         WHERE c.account_id = $1 AND c.is_active
         GROUP BY c.id
         ORDER BY avg_margin_pct DESC, c.id",
    )
    .bind(t.account.id)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;
    Ok(Json(rows))
}

#[derive(FromRow)]
struct CsvRow {
    session_date: Option<NaiveDate>,
    client: Option<String>,
    service_fee: Option<Decimal>,
    ingredient_cost: Option<Decimal>,
    gross_margin: Option<Decimal>,
    margin_pct: Option<Decimal>,
    status: Option<String>,
}

/// A number the way JS `String(n)` prints it (no trailing zeros).
pub fn js_num(d: Decimal) -> String {
    d.normalize().to_string()
}

/// Quote a CSV field; prefix formula-injection characters so spreadsheet
/// apps don't execute them.
pub fn csv_field(v: &str) -> String {
    let s = v.replace('"', "\"\"");
    let safe = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s
    };
    format!("\"{safe}\"")
}

/// A quoted CSV field for server-generated values (numbers, dates): no
/// formula defusing, so spreadsheets still read "-12.5" as a number.
pub fn csv_quote(v: &str) -> String {
    format!("\"{}\"", v.replace('"', "\"\""))
}

async fn export_csv(State(state): State<AppState>, mut t: Tenant) -> DbResult<Response> {
    if let Err(limited) = state
        .account_limits
        .csv_export
        .check(&state.pool, &AccountLimits::key(t.account.id))
        .await
    {
        return Ok(limited.with_code().into_response());
    }
    let rows = sqlx::query_as::<_, CsvRow>(&format!(
        "SELECT s.session_date, c.name AS client, s.service_fee, s.ingredient_cost,
                (s.service_fee - s.ingredient_cost) AS gross_margin,
                {MARGIN_PCT_SQL} AS margin_pct,
                s.status
         FROM sessions s
         LEFT JOIN clients c ON c.id = s.client_id
         WHERE s.account_id = $1 AND s.status != 'Cancelled'
         ORDER BY s.session_date DESC NULLS LAST"
    ))
    .bind(t.account.id)
    .fetch_all(&mut *t.tx)
    .await?;
    t.tx.commit().await?;

    let num = |d: Option<Decimal>| d.map(js_num).unwrap_or_default();
    let body = std::iter::once(
        "Date,Client,Service Fee,Ingredient Cost,Gross Margin,Margin %,Status".to_string(),
    )
    .chain(rows.iter().map(|r| {
        // Only the free-text columns can carry a formula; defusing the
        // numbers would turn a negative margin into text ("'-12.5").
        [
            csv_quote(&r.session_date.map(|d| d.to_string()).unwrap_or_default()),
            csv_field(&r.client.clone().unwrap_or_default()),
            csv_quote(&num(r.service_fee)),
            csv_quote(&num(r.ingredient_cost)),
            csv_quote(&num(r.gross_margin)),
            csv_quote(&format!("{}%", num(r.margin_pct))),
            csv_field(&r.status.clone().unwrap_or_default()),
        ]
        .join(",")
    }))
    .collect::<Vec<_>>();
    // Header line ends with "\n" and rows are "\n"-joined, like the Node route.
    let text = match body.split_first() {
        Some((head, rest)) => format!("{head}\n{}", rest.join("\n")),
        None => String::new(),
    };

    let mut res = text.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"costs.csv\""),
    );
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn csv_fields_are_quoted_and_defused() {
        assert_eq!(csv_field("Smith"), "\"Smith\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=SUM(A1)"), "\"'=SUM(A1)\"");
        assert_eq!(csv_field("-5"), "\"'-5\"");
        assert_eq!(csv_quote("-12.5"), "\"-12.5\"");
    }

    #[test]
    fn percentages_and_numbers_print_like_js() {
        let d = |s| Decimal::from_str(s).unwrap();
        assert_eq!(pct(d("150"), d("200")), d("75"));
        assert_eq!(pct(d("1"), d("3")), d("33.3"));
        assert_eq!(pct(d("1"), d("0")), d("0"));
        assert_eq!(js_num(d("200.00")), "200");
        assert_eq!(js_num(d("12.50")), "12.5");
    }
}
