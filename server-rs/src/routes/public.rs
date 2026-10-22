// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/public.js`: the public (no-auth) chef profile for
//! the booking page, `GET /api/public/:slug/settings`.
//!
//! Only fields a prospective client should see, never phone/email/fees.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use sqlx::PgConnection;
use ts_rs::TS;

use crate::accounts;
use crate::db::begin_tenant;
use crate::error::{DbError, DbResult};
use crate::serde_util::json_array;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/{slug}/settings", get(by_slug))
}

/// GET /api/public/:slug/settings and the legacy GET /api/settings/public.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, TS)]
#[ts(export)]
pub struct PublicProfile {
    pub slug: String,
    pub business_name: Option<String>,
    pub chef_name: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub logo_base64: Option<String>,
    #[ts(type = "number | null")]
    pub deposit_cents: Option<i64>,
    pub cancellation_hours: Option<i32>,
    pub payment_methods: Option<String>,
    pub booking_terms: Option<String>,
    pub response_time_text: Option<String>,
    #[serde(serialize_with = "json_array::serialize")]
    #[ts(type = "string[]")]
    pub service_types: Option<serde_json::Value>,
    #[sqlx(skip)]
    #[ts(type = "string")]
    pub payments_mode: &'static str,
}

/// Port of `publicProfile(db, accountId)`; run on that account's tenant
/// transaction (settings is under RLS).
pub async fn public_profile(
    conn: &mut PgConnection,
    account_id: i64,
    payments_mode: &'static str,
) -> Result<Option<PublicProfile>, sqlx::Error> {
    let row = sqlx::query_as::<_, PublicProfile>(
        "SELECT a.slug, s.business_name, s.chef_name, s.website, s.location, s.logo_base64,
                s.deposit_cents, s.cancellation_hours, s.payment_methods, s.booking_terms,
                s.response_time_text, s.service_types
         FROM accounts a LEFT JOIN settings s ON s.account_id = a.id
         WHERE a.id = $1",
    )
    .bind(account_id)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|p| PublicProfile { payments_mode, ..p }))
}

/// The public profile of `account_id`, looked up under its own tenant.
pub async fn profile_for(state: &AppState, account_id: i64) -> DbResult<Option<PublicProfile>> {
    let mut tx = begin_tenant(&state.pool, account_id).await?;
    let profile = public_profile(&mut tx, account_id, state.config.payments_mode.as_str()).await?;
    tx.commit().await?;
    Ok(profile)
}

async fn by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> DbResult<Json<PublicProfile>> {
    let mut conn = state.pool.acquire().await?;
    let Some(account) = accounts::get_account_by_slug(&mut conn, &slug).await? else {
        return Err(DbError::from(crate::error::AppError::not_found_msg(
            "Chef not found",
        )));
    };
    drop(conn);
    match profile_for(&state, account.id).await? {
        Some(p) => Ok(Json(p)),
        None => Err(crate::error::AppError::not_found_msg("Chef not found").into()),
    }
}
