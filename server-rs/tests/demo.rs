// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! The public demo account (`DEMO_MODE`): "Try the demo", the seeded data,
//! the periodic reset, and what the demo may not do.

mod common;

use axum::Router;
use axum::http::StatusCode;
use chef_server::config::{Config, DemoConfig, SignupMode};
use chef_server::db::MIGRATOR;
use chef_server::demo::{self, DEMO_EMAIL};
use chef_server::state::AppState;
use common::{Chef, TestResponse, app, app_with, get, post, put, test_config};
use serde_json::{Value, json};
use sqlx::PgPool;

fn demo_config() -> Config {
    Config {
        demo: DemoConfig {
            enabled: true,
            reset_hours: 24,
        },
        ..test_config()
    }
}

/// An app with the demo seeded, as `main` starts it.
async fn demo_app(pool: PgPool, config: Config) -> (Router, AppState) {
    let (app, state) = app_with(pool, config);
    demo::start(state.clone()).await;
    (app, state)
}

/// Signs in with "Try the demo"; returns the response and its cookie.
async fn try_demo(app: &Router) -> (TestResponse, String) {
    let res = post("/api/auth/demo", json!({})).send(app).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.text);
    let cookie = res.auth_cookie().unwrap();
    (res, cookie)
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn no_demo_unless_enabled(pool: PgPool) {
    let app = app(pool);
    let status = get("/api/auth/signup").send(&app).await;
    assert_eq!(status.body["demo"], false);
    let res = post("/api/auth/demo", json!({})).send(&app).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(res.body["code"], "DEMO_UNAVAILABLE");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn try_the_demo_opens_a_seeded_account(pool: PgPool) {
    let (app, _) = demo_app(pool, demo_config()).await;
    assert_eq!(get("/api/auth/signup").send(&app).await.body["demo"], true);

    let (res, cookie) = try_demo(&app).await;
    assert_eq!(res.body["email"], DEMO_EMAIL);
    assert_eq!(res.body["onboarding_completed"], true);
    assert_eq!(res.body["demo"]["reset_hours"], 24);
    assert!(res.body["demo"]["next_reset_at"].is_string());
    let me = get("/api/auth/me").cookie(&cookie).send(&app).await;
    assert_eq!(me.body["demo"]["reset_hours"], 24);

    let clients = get("/api/clients").cookie(&cookie).send(&app).await;
    assert_eq!(len(&clients.body), 5, "{}", clients.text);
    let recipes = get("/api/recipes").cookie(&cookie).send(&app).await;
    assert_eq!(len(&recipes.body), 20);

    let sessions = get("/api/sessions").cookie(&cookie).send(&app).await;
    let sessions = sessions.body.as_array().unwrap().clone();
    assert!(sessions.len() > 20);
    for status in ["Completed", "Confirmed", "Pending", "Cancelled"] {
        assert!(
            sessions.iter().any(|s| s["status"] == status),
            "no {status} session"
        );
    }
    let upcoming = sessions
        .iter()
        .find(|s| s["status"] == "Confirmed")
        .unwrap();
    let grocery = get(format!("/api/sessions/{}/grocery", upcoming["id"]))
        .cookie(&cookie)
        .send(&app)
        .await;
    assert_eq!(grocery.status, StatusCode::OK);
    assert!(len(&grocery.body["items"]) > 5, "{}", grocery.text);

    let invoices = get("/api/invoices").cookie(&cookie).send(&app).await;
    let invoices = invoices.body.as_array().unwrap().clone();
    for status in ["paid", "sent", "draft"] {
        assert!(
            invoices.iter().any(|i| i["status"] == status),
            "no {status} invoice"
        );
    }
    let inquiries = get("/api/intake").cookie(&cookie).send(&app).await;
    assert_eq!(len(&inquiries.body), 3, "{}", inquiries.text);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reset_restores_the_demo_and_leaves_other_accounts_alone(pool: PgPool) {
    let (app, _) = demo_app(pool.clone(), demo_config()).await;
    let (first, cookie) = try_demo(&app).await;
    let other = Chef::new(&pool, &app, "Real Kitchen").await;
    assert_eq!(
        other
            .post("/api/clients", json!({ "name": "Real Client" }))
            .await
            .status,
        StatusCode::CREATED
    );

    // A visitor edits the demo.
    let added = post("/api/clients", json!({ "name": "Visitor Was Here" }))
        .cookie(&cookie)
        .send(&app)
        .await;
    assert_eq!(added.status, StatusCode::CREATED);
    let put_settings = put("/api/settings", json!({ "business_name": "Renamed" }))
        .cookie(&cookie)
        .send(&app)
        .await;
    assert_eq!(put_settings.status, StatusCode::OK, "{}", put_settings.text);
    assert_eq!(
        len(&get("/api/clients").cookie(&cookie).send(&app).await.body),
        6
    );

    let id = demo::reset(&pool).await.unwrap();
    assert_eq!(id, first.body["id"].as_i64().unwrap(), "same account");

    // The visitor's cookie still works and sees fresh data.
    let clients = get("/api/clients").cookie(&cookie).send(&app).await;
    assert_eq!(len(&clients.body), 5);
    assert!(!clients.text.contains("Visitor Was Here"));
    let settings = get("/api/settings").cookie(&cookie).send(&app).await;
    assert_eq!(settings.body["business_name"], "Demo Kitchen");

    // Another account is untouched, and can't see the demo's rows.
    let theirs = other.get("/api/clients").await;
    assert_eq!(len(&theirs.body), 1);
    assert_eq!(theirs.body[0]["name"], "Real Client");
    let demo_session: i64 =
        sqlx::query_scalar("SELECT id FROM sessions WHERE account_id = $1 LIMIT 1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let peek = other.get(&format!("/api/sessions/{demo_session}")).await;
    assert_eq!(peek.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn demo_sends_no_email_and_saves_no_ai_key(pool: PgPool) {
    let (app, state) = demo_app(pool, demo_config()).await;
    let (_, cookie) = try_demo(&app).await;

    let invoices = get("/api/invoices").cookie(&cookie).send(&app).await;
    let invoice = &invoices.body[0];
    let res = post(format!("/api/invoices/{}/email", invoice["id"]), json!({}))
        .cookie(&cookie)
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.body["code"], "DEMO_DISABLED");

    let ai = put(
        "/api/ai/settings",
        json!({ "provider": "anthropic", "api_key": "sk-ant-not-a-real-key" }),
    )
    .cookie(&cookie)
    .send(&app)
    .await;
    assert_eq!(ai.status, StatusCode::FORBIDDEN);
    assert_eq!(ai.body["code"], "DEMO_DISABLED");

    // A booking-page inquiry to the demo gets no confirmation email either.
    let slug: String = get("/api/auth/me").cookie(&cookie).send(&app).await.body["slug"]
        .as_str()
        .unwrap()
        .to_string();
    let intake = post(
        "/api/intake",
        json!({
            "slug": slug,
            "name": "Visitor",
            "email": "someone@example.com",
            "terms_accepted": true,
        }),
    )
    .send(&app)
    .await;
    assert_eq!(intake.status, StatusCode::CREATED, "{}", intake.text);
    assert!(state.outbox.messages().is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn demo_account_has_no_password_or_link_sign_in(pool: PgPool) {
    let (app, state) = demo_app(pool, demo_config()).await;
    let login = post(
        "/api/auth/login",
        json!({ "email": DEMO_EMAIL, "password": "" }),
    )
    .send(&app)
    .await;
    assert_eq!(login.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login.body["error"], "Incorrect email or password.");

    for route in ["/api/auth/forgot-password", "/api/auth/request-link"] {
        let res = post(route, json!({ "email": DEMO_EMAIL })).send(&app).await;
        assert_eq!(res.status, StatusCode::OK);
    }
    assert!(state.outbox.messages().is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_first_does_not_count_the_demo(pool: PgPool) {
    let config = Config {
        signup_mode: SignupMode::First,
        ..demo_config()
    };
    let (app, _) = demo_app(pool, config).await;
    assert_eq!(get("/api/auth/signup").send(&app).await.body["open"], true);
    let res = post(
        "/api/auth/signup",
        json!({ "email": "owner@example.com", "password": "correct-horse-battery" }),
    )
    .send(&app)
    .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(get("/api/auth/signup").send(&app).await.body["open"], false);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reset_is_repeatable(pool: PgPool) {
    let a = demo::reset(&pool).await.unwrap();
    let b = demo::reset(&pool).await.unwrap();
    assert_eq!(a, b);
    let (accounts, recipes): (i64, i64) =
        sqlx::query_as("SELECT (SELECT count(*) FROM accounts), (SELECT count(*) FROM recipes)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((accounts, recipes), (1, 20));
}
