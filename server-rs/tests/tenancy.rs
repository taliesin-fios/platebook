// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Cross-account isolation over HTTP: the Rust port of
//! `server/test/tenancy.test.js`, widened to every route that takes an id.
//! (`rls.rs` proves the same at the database level, without the routes.)

mod common;

use axum::http::StatusCode;
use chef_server::db::{MIGRATOR, begin_tenant};
use common::{Chef, app, get, post};
use serde_json::{Value, json};
use sqlx::PgPool;

struct Ids {
    client: i64,
    recipe: i64,
    session: i64,
    grocery_item: i64,
    template: i64,
    invoice: i64,
    intake: i64,
}

/// Account A builds a little of everything.
async fn build(a: &Chef) -> Ids {
    let id = |v: &Value| v["id"].as_i64().unwrap();
    let client = a
        .post(
            "/api/clients",
            json!({ "name": "Alpha Client", "email": "ac@example.com" }),
        )
        .await
        .body;
    let recipe = a
        .post(
            "/api/recipes",
            json!({ "name": "Alpha Stew", "ingredients": [{ "name": "onion", "qty_4_serving": "1" }] }),
        )
        .await
        .body;
    let session = a
        .post(
            "/api/sessions",
            json!({ "client_id": id(&client), "session_date": "2026-11-01", "service_fee": 200,
                    "recipe_ids": [id(&recipe)] }),
        )
        .await
        .body;
    let grocery = a
        .post(
            &format!("/api/sessions/{}/grocery/generate", id(&session)),
            json!({}),
        )
        .await
        .body;
    let template = a
        .post(
            "/api/templates",
            json!({ "name": "Weeknight", "recipe_ids": [id(&recipe)] }),
        )
        .await
        .body;
    let invoice = a
        .post(
            &format!("/api/invoices/from-session/{}", id(&session)),
            json!({}),
        )
        .await
        .body;
    let slug = a.get("/api/settings").await.body["slug"]
        .as_str()
        .unwrap()
        .to_string();
    let intake = post(
        "/api/intake",
        json!({ "slug": slug, "name": "Lead", "terms_accepted": true }),
    )
    .send(&a.app)
    .await
    .body;
    Ids {
        client: id(&client),
        recipe: id(&recipe),
        session: id(&session),
        grocery_item: id(&grocery["items"][0]),
        template: id(&template),
        invoice: id(&invoice),
        intake: id(&intake),
    }
}

async fn two_chefs(pool: &PgPool) -> (Chef, Chef, Ids) {
    let app = app(pool.clone());
    let a = Chef::new(pool, &app, "Alpha Kitchen").await;
    let b = Chef::new(pool, &app, "Beta Kitchen").await;
    let ids = build(&a).await;
    (a, b, ids)
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn b_gets_404_for_every_id_of_a(pool: PgPool) {
    let (_, b, i) = two_chefs(&pool).await;
    let cases: Vec<(&str, String, Value)> = vec![
        ("GET", format!("/api/clients/{}", i.client), json!(null)),
        (
            "PUT",
            format!("/api/clients/{}", i.client),
            json!({ "name": "Hacked" }),
        ),
        ("DELETE", format!("/api/clients/{}", i.client), json!(null)),
        ("GET", format!("/api/recipes/{}", i.recipe), json!(null)),
        (
            "PUT",
            format!("/api/recipes/{}", i.recipe),
            json!({ "name": "Hacked" }),
        ),
        ("DELETE", format!("/api/recipes/{}", i.recipe), json!(null)),
        ("GET", format!("/api/sessions/{}", i.session), json!(null)),
        (
            "PUT",
            format!("/api/sessions/{}", i.session),
            json!({ "chef_notes": "x" }),
        ),
        (
            "DELETE",
            format!("/api/sessions/{}", i.session),
            json!(null),
        ),
        (
            "POST",
            format!("/api/sessions/{}/clone", i.session),
            json!({ "days_offset": 1 }),
        ),
        (
            "POST",
            format!("/api/sessions/{}/complete", i.session),
            json!({}),
        ),
        (
            "POST",
            format!("/api/sessions/{}/dishes", i.session),
            json!({ "recipe_ids": [] }),
        ),
        (
            "POST",
            format!("/api/sessions/{}/email-pdf", i.session),
            json!({ "pdf_base64": "JVBERi0=" }),
        ),
        (
            "GET",
            format!("/api/sessions/{}/grocery", i.session),
            json!(null),
        ),
        (
            "GET",
            format!("/api/sessions/{}/pdf", i.session),
            json!(null),
        ),
        (
            "GET",
            format!("/api/sessions/{}/grocery/pdf", i.session),
            json!(null),
        ),
        (
            "POST",
            format!("/api/sessions/{}/grocery/generate", i.session),
            json!({}),
        ),
        (
            "PUT",
            format!("/api/grocery/{}", i.grocery_item),
            json!({ "is_checked": true }),
        ),
        (
            "POST",
            format!("/api/sessions/from-template/{}", i.template),
            json!({}),
        ),
        (
            "PUT",
            format!("/api/templates/{}", i.template),
            json!({ "name": "Hacked" }),
        ),
        (
            "DELETE",
            format!("/api/templates/{}", i.template),
            json!(null),
        ),
        ("GET", format!("/api/invoices/{}", i.invoice), json!(null)),
        (
            "GET",
            format!("/api/invoices/{}/pdf", i.invoice),
            json!(null),
        ),
        (
            "PUT",
            format!("/api/invoices/{}", i.invoice),
            json!({ "status": "void" }),
        ),
        (
            "DELETE",
            format!("/api/invoices/{}", i.invoice),
            json!(null),
        ),
        (
            "POST",
            format!("/api/invoices/{}/email", i.invoice),
            json!({ "pdf_base64": "JVBERi0=" }),
        ),
        (
            "POST",
            format!("/api/invoices/from-session/{}", i.session),
            json!({}),
        ),
        (
            "POST",
            format!("/api/invoices/from-client/{}", i.client),
            json!({}),
        ),
        (
            "POST",
            format!("/api/intake/{}/convert", i.intake),
            json!({}),
        ),
        (
            "POST",
            format!("/api/intake/{}/resend-confirmation", i.intake),
            json!({}),
        ),
        ("DELETE", format!("/api/intake/{}", i.intake), json!(null)),
        (
            "POST",
            format!("/api/intake/{}/restore", i.intake),
            json!({}),
        ),
        (
            "GET",
            format!("/api/intake/{}/payments", i.intake),
            json!(null),
        ),
        (
            "GET",
            format!("/api/intake/{}/client-link", i.intake),
            json!(null),
        ),
        (
            "GET",
            format!("/api/payments/status/{}", i.intake),
            json!(null),
        ),
        (
            "POST",
            "/api/payments/manual/action".into(),
            json!({ "intake_id": i.intake, "action": "mark_paid_offline" }),
        ),
        (
            "POST",
            "/api/ai/suggest-menu".into(),
            json!({ "client_id": i.client }),
        ),
    ];
    for (method, uri, body) in cases {
        let res = match method {
            "GET" => b.get(&uri).await,
            "PUT" => b.put(&uri, body).await,
            "DELETE" => b.delete(&uri).await,
            _ => b.post(&uri, body).await,
        };
        // AI routes check the key before ownership (no key in tests → 503).
        let expected = if uri.starts_with("/api/ai") {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::NOT_FOUND
        };
        assert_eq!(res.status, expected, "{method} {uri}: {}", res.body);
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_data_is_intact_after_b_attempts_writes(pool: PgPool) {
    let (a, b, i) = two_chefs(&pool).await;
    b.put(
        &format!("/api/clients/{}", i.client),
        json!({ "name": "Hacked" }),
    )
    .await;
    b.delete(&format!("/api/sessions/{}", i.session)).await;
    b.put(
        &format!("/api/invoices/{}", i.invoice),
        json!({ "status": "void" }),
    )
    .await;
    b.post(
        "/api/payments/manual/action",
        json!({ "intake_id": i.intake, "action": "mark_paid_offline" }),
    )
    .await;
    assert_eq!(
        a.get(&format!("/api/clients/{}", i.client)).await.body["name"],
        "Alpha Client"
    );
    assert_eq!(
        a.get(&format!("/api/sessions/{}", i.session)).await.status,
        StatusCode::OK
    );
    assert_eq!(
        a.get(&format!("/api/invoices/{}", i.invoice)).await.body["status"],
        "draft"
    );
    assert_eq!(
        a.get(&format!("/api/payments/status/{}", i.intake))
            .await
            .body["payment_status"],
        "not_requested"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn b_cannot_reference_a_records_in_its_own_writes(pool: PgPool) {
    let (_, b, i) = two_chefs(&pool).await;
    let r = b
        .post("/api/sessions", json!({ "client_id": i.client }))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let own = b
        .post("/api/clients", json!({ "name": "Beta Client" }))
        .await
        .body;
    let r = b
        .post(
            "/api/sessions",
            json!({ "client_id": own["id"], "recipe_ids": [i.recipe] }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let s = b
        .post("/api/sessions", json!({ "client_id": own["id"] }))
        .await
        .body;
    let r = b
        .put(
            &format!("/api/sessions/{}", s["id"]),
            json!({ "client_id": i.client }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = b
        .post("/api/invoices", json!({ "client_id": i.client }))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = b
        .post("/api/invoices", json!({ "session_id": i.session }))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let t = b
        .post(
            "/api/templates",
            json!({ "name": "Sneaky", "recipe_ids": [i.recipe] }),
        )
        .await;
    assert_eq!(t.status, StatusCode::CREATED);
    assert_eq!(t.body["recipe_ids"], json!([]));
    let p = b
        .post(
            "/api/planner/ingredients",
            json!({ "recipe_ids": [i.recipe] }),
        )
        .await;
    assert_eq!(p.body["total_items"], 0);
    let pdf = b
        .get(&format!("/api/planner/pdf?recipe_ids={}", i.recipe))
        .await;
    assert_eq!(pdf.status, StatusCode::OK);
    let text = pdf_extract::extract_text_from_mem(&pdf.bytes).unwrap();
    assert!(text.contains("0 unique ingredients"), "{text}");
    assert!(
        !text.contains("Alpha"),
        "B's planner PDF shows A's recipe: {text}"
    );
    assert!(
        !text.contains("onion"),
        "B's planner PDF shows A's ingredient: {text}"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn lists_and_aggregates_only_show_own_rows(pool: PgPool) {
    let (a, b, i) = two_chefs(&pool).await;
    for url in [
        "/api/clients",
        "/api/recipes",
        "/api/sessions",
        "/api/invoices",
        "/api/intake",
        "/api/templates",
    ] {
        let mine = a.get(url).await.body;
        assert!(!mine.as_array().unwrap().is_empty(), "{url}");
        let theirs = b.get(url).await.body;
        assert_eq!(theirs, json!([]), "{url}: B sees A's rows");
    }
    let costs = b.get("/api/costs/summary").await.body;
    assert_eq!(costs["session_count"], 0);
    assert_eq!(costs["invoiced_total"], 0.0);
    assert_eq!(b.get("/api/costs/by-client").await.body, json!([]));
    assert_eq!(b.get("/api/costs/sessions?days=365").await.body, json!([]));
    let csv = b.get("/api/costs/export-csv").await;
    assert!(!csv.text.contains("Alpha"));

    let export = b.get("/api/settings/export").await.body;
    for table in [
        "clients",
        "recipes",
        "ingredients",
        "sessions",
        "session_dishes",
        "grocery_items",
        "session_templates",
        "client_intakes",
        "invoices",
        "invoice_line_items",
        "payment_events",
    ] {
        assert_eq!(export[table], json!([]), "export.{table}");
    }
    assert_eq!(export["account"]["id"], b.id);
    let _ = i;
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn per_account_uniqueness_and_numbering(pool: PgPool) {
    let (a, b, _) = two_chefs(&pool).await;
    assert_eq!(
        b.post("/api/templates", json!({ "name": "Weeknight" }))
            .await
            .status,
        StatusCode::CREATED
    );
    assert_eq!(
        a.post("/api/templates", json!({ "name": "Weeknight" }))
            .await
            .status,
        StatusCode::CONFLICT
    );
    b.put("/api/settings", json!({ "invoice_prefix": "bk" }))
        .await;
    let own = b
        .post("/api/clients", json!({ "name": "Numbered" }))
        .await
        .body;
    let inv = b
        .post(
            &format!("/api/invoices/from-client/{}", own["id"]),
            json!({}),
        )
        .await
        .body;
    let year = chrono::Utc::now().format("%Y");
    assert_eq!(inv["invoice_number"], format!("BK-{year}-0001"));
    let a_inv = a.get("/api/invoices").await.body;
    assert_eq!(a_inv[0]["invoice_number"], format!("INV-{year}-0001"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn intake_lands_with_the_slug_owner_and_tokens_dont_cross(pool: PgPool) {
    let (a, b, i) = two_chefs(&pool).await;
    // Signed in as another chef without a token: refused, like a stranger.
    assert_eq!(
        b.get(&format!("/api/intake/{}", i.intake)).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        b.put(
            &format!("/api/intake/{}", i.intake),
            json!({ "status": "booked" })
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let beta = post(
        "/api/intake",
        json!({ "slug": "beta-kitchen", "name": "Beta Lead", "terms_accepted": true }),
    )
    .send(&b.app)
    .await
    .body;
    assert!(
        a.get("/api/intake")
            .await
            .body
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["name"] != "Beta Lead")
    );
    assert_eq!(b.get("/api/intake").await.body[0]["name"], "Beta Lead");
    // A's token for B's intake id doesn't verify.
    let a_token = chef_server::intake_token::make_token("test-intake-secret", i.intake);
    let crossed = get(format!("/api/intake/{}?token={a_token}", beta["id"]))
        .send(&b.app)
        .await;
    assert_eq!(crossed.status, StatusCode::FORBIDDEN);
    assert_eq!(
        post(
            "/api/intake",
            json!({ "slug": "nobody-here", "name": "X", "terms_accepted": true })
        )
        .send(&b.app)
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn database_backstop_when_a_query_forgets_its_filter(pool: PgPool) {
    // Routes filter by account explicitly; RLS is the backstop. Run the
    // unfiltered shape of the routes' lookups as B against A's ids.
    let (_, b, i) = two_chefs(&pool).await;
    let mut tx = begin_tenant(&pool, b.id).await.unwrap();
    for (sql, id) in [
        ("SELECT COUNT(*) FROM clients WHERE id = $1", i.client),
        ("SELECT COUNT(*) FROM recipes WHERE id = $1", i.recipe),
        (
            "SELECT COUNT(*) FROM ingredients WHERE recipe_id = $1",
            i.recipe,
        ),
        ("SELECT COUNT(*) FROM sessions WHERE id = $1", i.session),
        (
            "SELECT COUNT(*) FROM session_dishes WHERE session_id = $1",
            i.session,
        ),
        (
            "SELECT COUNT(*) FROM grocery_items WHERE id = $1",
            i.grocery_item,
        ),
        (
            "SELECT COUNT(*) FROM session_templates WHERE id = $1",
            i.template,
        ),
        ("SELECT COUNT(*) FROM invoices WHERE id = $1", i.invoice),
        (
            "SELECT COUNT(*) FROM invoice_line_items WHERE invoice_id = $1",
            i.invoice,
        ),
        (
            "SELECT COUNT(*) FROM client_intakes WHERE id = $1",
            i.intake,
        ),
    ] {
        let n: i64 = sqlx::query_scalar(sql)
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(n, 0, "{sql}");
    }
    // Unfiltered writes touch nothing either.
    for sql in [
        "UPDATE clients SET name = 'x' WHERE id = $1",
        "DELETE FROM sessions WHERE id = $1",
        "UPDATE invoices SET status = 'void' WHERE id = $1",
    ] {
        let id = match sql {
            s if s.contains("clients") => i.client,
            s if s.contains("sessions") => i.session,
            _ => i.invoice,
        };
        let n = sqlx::query(sql)
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap()
            .rows_affected();
        assert_eq!(n, 0, "{sql}");
    }
    // And B can't attach A's recipe to its own session (composite FK + RLS check).
    let own_client: i64 =
        sqlx::query_scalar("INSERT INTO clients (account_id, name) VALUES ($1, 'B') RETURNING id")
            .bind(b.id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    let own_session: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (account_id, client_id) VALUES ($1, $2) RETURNING id",
    )
    .bind(b.id)
    .bind(own_client)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let mut sp = sqlx::Acquire::begin(&mut *tx).await.unwrap();
    let attach = sqlx::query("INSERT INTO session_dishes (session_id, recipe_id) VALUES ($1, $2)")
        .bind(own_session)
        .bind(i.recipe)
        .execute(&mut *sp)
        .await;
    assert!(attach.is_err(), "B attached A's recipe");
    sp.rollback().await.unwrap();
    tx.rollback().await.unwrap();
}
