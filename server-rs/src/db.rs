// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Postgres pool, migrations and per-request tenant transactions.
//!
//! Tenant data is only ever read or written inside a transaction opened by
//! [`begin_tenant`]: it switches to the `chef_app` role and sets
//! `app.account_id`, so the Row-Level Security policies in
//! `migrations/0002_row_level_security.sql` scope every statement to that
//! account. Both settings are `LOCAL`, so they end with the transaction and
//! never leak to the next request that reuses the pooled connection.

use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};

use crate::auth::Account;
use crate::error::AppError;
use crate::state::AppState;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

pub type Tx = Transaction<'static, Postgres>;

/// Opens a transaction scoped to `account_id` by Row-Level Security.
pub async fn begin_tenant(pool: &PgPool, account_id: i64) -> Result<Tx, sqlx::Error> {
    let mut tx = pool.begin().await?;
    enter_tenant(&mut tx, account_id).await?;
    Ok(tx)
}

/// Scopes the rest of an open transaction to `account_id`, for work that
/// starts before the tenant is known (e.g. locking a row found across
/// accounts, as the reminder job does).
pub async fn enter_tenant(tx: &mut Tx, account_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("SET LOCAL ROLE chef_app")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(account_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Extractor: the signed-in account plus a tenant-scoped transaction.
///
/// ```ignore
/// async fn list(mut t: Tenant) -> AppResult<Json<Vec<Client>>> {
///     let rows = sqlx::query_as!(Client, "SELECT ... FROM clients").fetch_all(&mut *t.tx).await?;
///     t.tx.commit().await?;
///     Ok(Json(rows))
/// }
/// ```
///
/// Dropping it without `commit()` rolls back.
pub struct Tenant {
    pub account: Account,
    pub tx: Tx,
}

impl FromRequestParts<AppState> for Tenant {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let account = Account::from_request_parts(parts, state).await?;
        let tx = begin_tenant(&state.pool, account.id).await?;
        Ok(Tenant { account, tx })
    }
}
