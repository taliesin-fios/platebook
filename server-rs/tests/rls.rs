// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Row-Level Security: proves tenant isolation is enforced by Postgres, not by
//! the WHERE clauses we remember to write.
//!
//! The test pool connects as a superuser (like Railway's `postgres` user),
//! so these also prove `db::begin_tenant` drops the superuser's RLS bypass.

mod common;

use chef_server::accounts::get_settings;
use chef_server::db::{MIGRATOR, Tx, begin_tenant};
use sqlx::PgPool;

/// Every tenant-scoped table, top-level and child.
const ALL_TABLES: &[&str] = &[
    "clients",
    "recipes",
    "ingredients",
    "sessions",
    "session_dishes",
    "session_templates",
    "grocery_items",
    "client_intakes",
    "invoices",
    "invoice_line_items",
    "recipe_import_log",
    "payment_events",
    "settings",
];

struct Seeded {
    client: i64,
    recipe: i64,
    session: i64,
    invoice: i64,
}

/// One row in every tenant table for `account`, written through RLS.
async fn seed(pool: &PgPool, account: i64) -> Seeded {
    let mut tx = begin_tenant(pool, account).await.unwrap();
    let one = |sql: &'static str| sqlx::query_scalar::<_, i64>(sql).bind(account);

    let client = one("INSERT INTO clients (account_id, name) VALUES ($1, 'Client') RETURNING id")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let recipe = one("INSERT INTO recipes (account_id, name) VALUES ($1, 'Soup') RETURNING id")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let session = sqlx::query_scalar::<_, i64>(
        "INSERT INTO sessions (account_id, client_id, session_date) VALUES ($1, $2, '2026-10-20') RETURNING id",
    )
    .bind(account)
    .bind(client)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let invoice = sqlx::query_scalar::<_, i64>(
        "INSERT INTO invoices (account_id, client_id, session_id, invoice_number, total)
         VALUES ($1, $2, $3, 'INV-1', 120.50) RETURNING id",
    )
    .bind(account)
    .bind(client)
    .bind(session)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let intake =
        one("INSERT INTO client_intakes (account_id, name) VALUES ($1, 'Lead') RETURNING id")
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    for (sql, a, b) in [
        (
            "INSERT INTO ingredients (recipe_id, name) VALUES ($1, 'Salt')",
            recipe,
            0,
        ),
        (
            "INSERT INTO session_dishes (session_id, recipe_id) VALUES ($1, $2)",
            session,
            recipe,
        ),
        (
            "INSERT INTO grocery_items (session_id, ingredient_name) VALUES ($1, 'Salt')",
            session,
            0,
        ),
        (
            "INSERT INTO invoice_line_items (invoice_id, description, amount) VALUES ($1, 'Cooking', 120.50)",
            invoice,
            0,
        ),
        (
            "INSERT INTO session_templates (account_id, name) VALUES ($1, 'Weekly')",
            account,
            0,
        ),
        (
            "INSERT INTO recipe_import_log (account_id, recipe_id) VALUES ($1, $2)",
            account,
            recipe,
        ),
        (
            "INSERT INTO payment_events (account_id, intake_id, amount) VALUES ($1, $2, 50)",
            account,
            intake,
        ),
    ] {
        let q = sqlx::query(sql).bind(a);
        let q = if sql.contains("$2") { q.bind(b) } else { q };
        q.execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    Seeded {
        client,
        recipe,
        session,
        invoice,
    }
}

/// `SELECT count(*) FROM <table>` — deliberately with no account filter.
async fn count(tx: &mut Tx, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(&mut **tx)
        .await
        .unwrap()
}

fn sqlstate(err: sqlx::Error) -> String {
    err.as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned())
        .unwrap_or_else(|| panic!("expected a database error, got {err:?}"))
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn account_b_cannot_read_account_a_rows_without_a_filter(pool: PgPool) {
    let (a, _) = common::make_account(&pool, "Kitchen A").await;
    let (b, _) = common::make_account(&pool, "Kitchen B").await;
    seed(&pool, a).await;

    let mut as_a = begin_tenant(&pool, a).await.unwrap();
    for table in ALL_TABLES {
        assert_eq!(
            count(&mut as_a, table).await,
            1,
            "A should see its own {table}"
        );
    }
    as_a.rollback().await.unwrap();

    let mut as_b = begin_tenant(&pool, b).await.unwrap();
    for table in ALL_TABLES {
        let n = if *table == "settings" { 1 } else { 0 }; // B's own settings row only
        assert_eq!(
            count(&mut as_b, table).await,
            n,
            "B must not see A's {table}"
        );
    }
    // Even naming A's id explicitly returns nothing.
    let leaked: Option<String> =
        sqlx::query_scalar("SELECT name FROM clients WHERE account_id = $1")
            .bind(a)
            .fetch_optional(&mut *as_b)
            .await
            .unwrap();
    assert_eq!(leaked, None);
    // And B sees only its own account row.
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM accounts")
        .fetch_all(&mut *as_b)
        .await
        .unwrap();
    assert_eq!(ids, vec![b]);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn no_tenant_set_means_no_rows(pool: PgPool) {
    let (a, _) = common::make_account(&pool, "Kitchen A").await;
    seed(&pool, a).await;

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE chef_app")
        .execute(&mut *tx)
        .await
        .unwrap();
    for table in ALL_TABLES {
        assert_eq!(
            count(&mut tx, table).await,
            0,
            "{table} visible without a tenant"
        );
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn account_b_cannot_write_account_a_rows(pool: PgPool) {
    let (a, _) = common::make_account(&pool, "Kitchen A").await;
    let (b, _) = common::make_account(&pool, "Kitchen B").await;
    let rows = seed(&pool, a).await;

    // UPDATE / DELETE without a filter touch nothing of A's.
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    for sql in [
        "UPDATE clients SET name = 'pwned'",
        "DELETE FROM invoice_line_items",
        "DELETE FROM ingredients",
        "UPDATE grocery_items SET is_checked = true",
        "DELETE FROM sessions",
    ] {
        let done = sqlx::query(sql).execute(&mut *tx).await.unwrap();
        assert_eq!(done.rows_affected(), 0, "{sql}");
    }
    tx.commit().await.unwrap();

    // Inserting a row stamped with A's account_id fails WITH CHECK (42501).
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    let err = sqlx::query("INSERT INTO clients (account_id, name) VALUES ($1, 'x')")
        .bind(a)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(err), "42501");
    drop(tx);

    // Moving one's own row into A's account fails too.
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    let own: i64 = sqlx::query_scalar(
        "INSERT INTO clients (account_id, name) VALUES ($1, 'mine') RETURNING id",
    )
    .bind(b)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let err = sqlx::query("UPDATE clients SET account_id = $1 WHERE id = $2")
        .bind(a)
        .bind(own)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(err), "42501");
    drop(tx);

    // Child rows can't be hung off A's parents.
    for (sql, id) in [
        (
            "INSERT INTO ingredients (recipe_id, name) VALUES ($1, 'x')",
            rows.recipe,
        ),
        (
            "INSERT INTO grocery_items (session_id) VALUES ($1)",
            rows.session,
        ),
        (
            "INSERT INTO session_dishes (session_id) VALUES ($1)",
            rows.session,
        ),
        (
            "INSERT INTO invoice_line_items (invoice_id) VALUES ($1)",
            rows.invoice,
        ),
    ] {
        let mut tx = begin_tenant(&pool, b).await.unwrap();
        let err = sqlx::query(sql)
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap_err();
        assert_eq!(sqlstate(err), "42501", "{sql}");
    }

    // B's own session can't list A's recipe as a dish.
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    let b_client: i64 =
        sqlx::query_scalar("INSERT INTO clients (account_id, name) VALUES ($1, 'c') RETURNING id")
            .bind(b)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    let b_session: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (account_id, client_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(b)
    .bind(b_client)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let err = sqlx::query("INSERT INTO session_dishes (session_id, recipe_id) VALUES ($1, $2)")
        .bind(b_session)
        .bind(rows.recipe)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(err), "42501");
    drop(tx);

    // Composite FKs: B's session can't reference A's client (23503), even
    // though foreign-key checks themselves bypass RLS.
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    let err = sqlx::query("INSERT INTO sessions (account_id, client_id) VALUES ($1, $2)")
        .bind(b)
        .bind(rows.client)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(err), "23503");
    drop(tx);

    // A's data is intact.
    let mut tx = begin_tenant(&pool, a).await.unwrap();
    let name: String = sqlx::query_scalar("SELECT name FROM clients WHERE id = $1")
        .bind(rows.client)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(name, "Client");
    for table in [
        "ingredients",
        "grocery_items",
        "invoice_line_items",
        "sessions",
    ] {
        assert_eq!(count(&mut tx, table).await, 1, "{table}");
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn tenant_settings_do_not_leak_to_the_next_transaction(pool: PgPool) {
    let (a, _) = common::make_account(&pool, "Kitchen A").await;
    let pool_one = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();

    begin_tenant(&pool_one, a)
        .await
        .unwrap()
        .commit()
        .await
        .unwrap();

    // Same physical connection, next transaction: no role, no tenant.
    let (role, tenant): (String, Option<String>) = sqlx::query_as(
        "SELECT current_user::text, NULLIF(current_setting('app.account_id', true), '')",
    )
    .fetch_one(&pool_one)
    .await
    .unwrap();
    assert_ne!(role, "chef_app");
    assert_eq!(tenant, None);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn get_settings_is_scoped_by_rls(pool: PgPool) {
    let (a, _) = common::make_account(&pool, "Sunday Suppers").await;
    let (b, _) = common::make_account(&pool, "Other Kitchen").await;

    let mut tx = begin_tenant(&pool, a).await.unwrap();
    let s = get_settings(&mut tx, a).await.unwrap();
    assert_eq!(s.business_name.as_deref(), Some("Sunday Suppers"));
    assert_eq!(s.invoice_prefix.as_deref(), Some("INV"));
    assert_eq!(s.deposit_cents, Some(0));
    drop(tx);

    // Asking for A's settings from B's tenant returns the empty default.
    let mut tx = begin_tenant(&pool, b).await.unwrap();
    let s = get_settings(&mut tx, a).await.unwrap();
    assert_eq!(s.account_id, a);
    assert_eq!(s.business_name, None);
}
