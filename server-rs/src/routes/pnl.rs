// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Profit and loss by month, for tax time.
//!
//! GET /api/costs/pnl?year=YYYY            — 12 monthly rows plus year totals
//! GET /api/costs/pnl/export-csv?year=YYYY — the same as CSV (shares the CSV export limit)
//!
//! Cash basis: revenue is the subtotal of invoices marked paid, in the month
//! of `paid_at` in the account's time zone. Sales tax collected is shown apart
//! and is not income. Ingredient costs are those of non-cancelled sessions
//! dated on or before today, by session date. Booking deposits are a memo
//! line, not part of the totals: a chef may already have put one on an invoice.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::FromRow;
use ts_rs::TS;

use crate::accounts;
use crate::db::Tenant;
use crate::error::{AppError, DbResult};
use crate::rate_limit::AccountLimits;
use crate::routes::costs::{csv_quote, js_num};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pnl", get(report))
        .route("/pnl/export-csv", get(export_csv))
}

/// One month of GET /api/costs/pnl.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PnlMonth {
    /// 1–12.
    #[ts(type = "number")]
    pub month: i32,
    #[ts(type = "number")]
    pub revenue: Decimal,
    #[ts(type = "number")]
    pub sales_tax: Decimal,
    #[ts(type = "number")]
    pub ingredient_costs: Decimal,
    #[ts(type = "number")]
    pub net: Decimal,
    #[ts(type = "number")]
    pub invoices_paid: i64,
    #[ts(type = "number")]
    pub sessions: i64,
}

/// GET /api/costs/pnl.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PnlReport {
    #[ts(type = "number")]
    pub year: i32,
    pub months: Vec<PnlMonth>,
    pub totals: PnlMonth,
    /// Booking deposits received in the year. Not in the totals.
    #[ts(type = "number")]
    pub deposits_received: Decimal,
}

#[derive(FromRow)]
struct RevenueRow {
    month: i32,
    revenue: Decimal,
    sales_tax: Decimal,
    invoices_paid: i64,
}

#[derive(FromRow)]
struct CostRow {
    month: i32,
    ingredient_costs: Decimal,
    sessions: i64,
}

fn year_param(q: &HashMap<String, String>, default: i32) -> Result<i32, AppError> {
    match q.get("year") {
        None => Ok(default),
        Some(y) => y
            .parse::<i32>()
            .ok()
            .filter(|y| (2000..=2100).contains(y))
            .ok_or_else(|| AppError::validation("year must be a year from 2000 to 2100")),
    }
}

async fn build(t: &mut Tenant, q: &HashMap<String, String>) -> DbResult<PnlReport> {
    let id = t.account.id;
    let clock = accounts::local_clock(&mut t.tx, id).await?;
    let year = year_param(q, clock.today.year())?;

    let revenue = sqlx::query_as::<_, RevenueRow>(
        "SELECT EXTRACT(MONTH FROM paid_at AT TIME ZONE $2)::int AS month,
                COALESCE(SUM(subtotal), 0)   AS revenue,
                COALESCE(SUM(tax_amount), 0) AS sales_tax,
                COUNT(*)                     AS invoices_paid
         FROM invoices
         WHERE account_id = $1 AND status = 'paid' AND deleted_at IS NULL
           AND paid_at >= make_timestamptz($3, 1, 1, 0, 0, 0, $2)
           AND paid_at < make_timestamptz($3 + 1, 1, 1, 0, 0, 0, $2)
         GROUP BY 1",
    )
    .bind(id)
    .bind(&clock.tz)
    .bind(year)
    .fetch_all(&mut *t.tx)
    .await?;
    let costs = sqlx::query_as::<_, CostRow>(
        "SELECT EXTRACT(MONTH FROM session_date)::int AS month,
                COALESCE(SUM(ingredient_cost), 0)     AS ingredient_costs,
                COUNT(*)                              AS sessions
         FROM sessions
         WHERE account_id = $1 AND status != 'Cancelled'
           AND session_date <= $2
           AND session_date >= make_date($3, 1, 1)
           AND session_date < make_date($3 + 1, 1, 1)
         GROUP BY 1",
    )
    .bind(id)
    .bind(clock.today)
    .bind(year)
    .fetch_all(&mut *t.tx)
    .await?;
    let deposits_received: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(booking_fee_amount), 0)
         FROM client_intakes
         WHERE account_id = $1 AND deleted_at IS NULL
           AND payment_status IN ('paid', 'paid_offline')
           AND booking_fee_paid_at >= make_timestamptz($3, 1, 1, 0, 0, 0, $2)
           AND booking_fee_paid_at < make_timestamptz($3 + 1, 1, 1, 0, 0, 0, $2)",
    )
    .bind(id)
    .bind(&clock.tz)
    .bind(year)
    .fetch_one(&mut *t.tx)
    .await?;

    let mut months: Vec<PnlMonth> = (1..=12)
        .map(|month| PnlMonth {
            month,
            revenue: Decimal::ZERO,
            sales_tax: Decimal::ZERO,
            ingredient_costs: Decimal::ZERO,
            net: Decimal::ZERO,
            invoices_paid: 0,
            sessions: 0,
        })
        .collect();
    for r in revenue {
        let m = &mut months[(r.month - 1) as usize];
        m.revenue = r.revenue;
        m.sales_tax = r.sales_tax;
        m.invoices_paid = r.invoices_paid;
    }
    for r in costs {
        let m = &mut months[(r.month - 1) as usize];
        m.ingredient_costs = r.ingredient_costs;
        m.sessions = r.sessions;
    }
    let mut totals = PnlMonth {
        month: 0,
        revenue: Decimal::ZERO,
        sales_tax: Decimal::ZERO,
        ingredient_costs: Decimal::ZERO,
        net: Decimal::ZERO,
        invoices_paid: 0,
        sessions: 0,
    };
    for m in &mut months {
        m.net = m.revenue - m.ingredient_costs;
        totals.revenue += m.revenue;
        totals.sales_tax += m.sales_tax;
        totals.ingredient_costs += m.ingredient_costs;
        totals.invoices_paid += m.invoices_paid;
        totals.sessions += m.sessions;
    }
    totals.net = totals.revenue - totals.ingredient_costs;
    Ok(PnlReport {
        year,
        months,
        totals,
        deposits_received,
    })
}

async fn report(
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Json<PnlReport>> {
    let report = build(&mut t, &q).await?;
    t.tx.commit().await?;
    Ok(Json(report))
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn csv_text(r: &PnlReport) -> String {
    let row = |label: &str, m: &PnlMonth| {
        [
            csv_quote(label),
            csv_quote(&js_num(m.revenue)),
            csv_quote(&js_num(m.ingredient_costs)),
            csv_quote(&js_num(m.net)),
            csv_quote(&js_num(m.sales_tax)),
            csv_quote(&m.invoices_paid.to_string()),
            csv_quote(&m.sessions.to_string()),
        ]
        .join(",")
    };
    let mut lines = vec![
        "Month,Revenue,Ingredient Costs,Net,Sales Tax Collected,Invoices Paid,Sessions".to_string(),
    ];
    for m in &r.months {
        lines.push(row(
            &format!("{} {}", MONTHS[(m.month - 1) as usize], r.year),
            m,
        ));
    }
    lines.push(row(&format!("Total {}", r.year), &r.totals));
    lines.push(
        [
            csv_quote("Booking deposits received (not in totals)"),
            csv_quote(&js_num(r.deposits_received)),
        ]
        .join(","),
    );
    lines.join("\n")
}

async fn export_csv(
    State(state): State<AppState>,
    mut t: Tenant,
    Query(q): Query<HashMap<String, String>>,
) -> DbResult<Response> {
    // A bad year is refused before it can use up the shared export limit.
    year_param(&q, 0)?;
    if let Err(limited) = state
        .account_limits
        .csv_export
        .check(&state.pool, &AccountLimits::key(t.account.id))
        .await
    {
        return Ok(limited.with_code().into_response());
    }
    let report = build(&mut t, &q).await?;
    t.tx.commit().await?;

    let mut res = (StatusCode::OK, csv_text(&report)).into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"pnl-{}.csv\"", report.year))
            .expect("ASCII filename"),
    );
    Ok(res)
}
