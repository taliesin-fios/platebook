// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `ownedRow()` / `ownedIds()` from `server/utils/helpers.js`.
//!
//! Row-Level Security already hides other accounts' rows, but ids that come
//! from the request (params or body) are still checked explicitly: it states
//! the intent, gives the same 404/400 answers as the Node routes, and keeps
//! working if a query ever runs outside a tenant transaction.

use sqlx::PgConnection;

/// Tables that carry `account_id`. An enum so the name interpolated into SQL
/// can only ever be one of these.
#[derive(Debug, Clone, Copy)]
pub enum Scoped {
    Clients,
    Recipes,
    Sessions,
    SessionTemplates,
    ClientIntakes,
    Invoices,
}

impl Scoped {
    fn table(self) -> &'static str {
        match self {
            Scoped::Clients => "clients",
            Scoped::Recipes => "recipes",
            Scoped::Sessions => "sessions",
            Scoped::SessionTemplates => "session_templates",
            Scoped::ClientIntakes => "client_intakes",
            Scoped::Invoices => "invoices",
        }
    }
}

/// True if row `id` of `table` belongs to `account_id`.
pub async fn owned(
    conn: &mut PgConnection,
    table: Scoped,
    id: i64,
    account_id: i64,
) -> Result<bool, sqlx::Error> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {} WHERE id = $1 AND account_id = $2)",
        table.table()
    );
    sqlx::query_scalar(&sql)
        .bind(id)
        .bind(account_id)
        .fetch_one(conn)
        .await
}

/// The subset of `ids` (order preserved) that exist in `table` for `account_id`.
pub async fn owned_ids(
    conn: &mut PgConnection,
    table: Scoped,
    ids: &[i64],
    account_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let sql = format!(
        "SELECT id FROM {} WHERE account_id = $1 AND id = ANY($2)",
        table.table()
    );
    let found: Vec<i64> = sqlx::query_scalar(&sql)
        .bind(account_id)
        .bind(ids)
        .fetch_all(conn)
        .await?;
    Ok(ids
        .iter()
        .copied()
        .filter(|id| found.contains(id))
        .collect())
}
