// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Intake (public + token), payments (manual, Stripe checkout against a fake
//! API, signed webhooks), the Claude routes against a fake Messages API,
//! email (outbox, Reply-To), recipe URL import guards, security headers and
//! serving the React build. No test talks to a real external service.

mod common;

use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get as get_route, post};
use axum::{Json, Router};
use chef_server::config::{Config, PaymentsMode};
use chef_server::db::MIGRATOR;
use common::{Chef, JWT_SECRET, app, app_with, get, post as post_req, put as put_req};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Starts `router` on an ephemeral localhost port; returns its base URL.
async fn fake_server(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

async fn submit_intake(app: &Router, slug: &str) -> Value {
    let res = post_req(
        "/api/intake",
        json!({ "slug": slug, "name": " Lead ", "email": "lead@example.com", "terms_accepted": true,
                "allergens": ["Soy"], "kitchen_notes": "Gas stove", "notes_for_chef": "Hi" }),
    )
    .send(app)
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.body);
    res.body
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn intake_public_token_and_chef_flows(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Intake Kitchen").await;
    let intake = submit_intake(&app, "intake-kitchen").await;
    assert_eq!(intake["name"], "Lead");
    assert_eq!(intake["status"], "submitted");
    assert_eq!(intake["household_type"], "Couple");
    assert_eq!(intake["client_containers"], 1);
    assert_eq!(intake["chef_slug"], "intake-kitchen");
    let id = intake["id"].as_i64().unwrap();
    let token = intake["edit_token"].as_str().unwrap();
    assert_eq!(intake["edit_url"], format!("/intake/{id}?token={token}"));

    for (body, field) in [
        (
            json!({ "slug": "intake-kitchen", "terms_accepted": true }),
            "name",
        ),
        (
            json!({ "slug": "intake-kitchen", "name": "A" }),
            "terms_accepted",
        ),
        (
            json!({ "slug": "intake-kitchen", "name": "A", "terms_accepted": true, "email": "x" }),
            "email",
        ),
    ] {
        let r = post_req("/api/intake", body).send(&app).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert_eq!(r.body["field"], field);
        assert!(r.body.get("code").is_none());
    }
    let gone = post_req(
        "/api/intake",
        json!({ "slug": "nobody", "name": "A", "terms_accepted": true }),
    )
    .send(&app)
    .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);

    // Token holder: read + client fields only.
    let read = get(format!("/api/intake/{id}?token={token}"))
        .send(&app)
        .await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.body["allergens"], json!(["Soy"]));
    let edit = put_req(
        format!("/api/intake/{id}?token={token}"),
        json!({ "phone": "555-0100", "status": "booked", "internal_notes": "x" }),
    )
    .send(&app)
    .await;
    assert_eq!(edit.body["phone"], "555-0100");
    assert_eq!(edit.body["status"], "submitted");
    assert_eq!(edit.body["internal_notes"], Value::Null);
    assert_eq!(
        get(format!("/api/intake/{id}")).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(format!("/api/intake/{id}?token=bad"))
            .send(&app)
            .await
            .status,
        StatusCode::FORBIDDEN
    );

    // Owning chef: no token needed, admin fields allowed.
    let chef_edit = c
        .put(
            &format!("/api/intake/{id}"),
            json!({ "status": "reviewed_by_chef" }),
        )
        .await;
    assert_eq!(chef_edit.body["status"], "reviewed_by_chef");
    assert_eq!(
        c.get("/api/intake?status=reviewed_by_chef")
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let converted = c
        .post(&format!("/api/intake/{id}/convert"), json!({}))
        .await;
    assert_eq!(converted.status, StatusCode::CREATED);
    assert_eq!(converted.body["client"]["name"], "Lead");
    assert_eq!(converted.body["client"]["chef_notes"], "Hi\n\nGas stove");
    assert_eq!(converted.body["client"]["allergens"], json!(["Soy"]));
    let again = c
        .post(&format!("/api/intake/{id}/convert"), json!({}))
        .await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body["already_converted"], true);
    assert_eq!(again.body["client"]["id"], converted.body["client"]["id"]);
    assert_eq!(
        c.get(&format!("/api/intake/{id}")).await.body["status"],
        "booked"
    );

    assert_eq!(
        c.delete(&format!("/api/intake/{id}")).await.status,
        StatusCode::OK
    );
    assert_eq!(
        c.delete(&format!("/api/intake/{id}")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn intake_tokens_from_node_still_work(pool: PgPool) {
    let app = app(pool.clone());
    Chef::new(&pool, &app, "Legacy Kitchen").await;
    let intake = submit_intake(&app, "legacy-kitchen").await;
    let id = intake["id"].as_i64().unwrap();
    // A token minted the way utils/intakeToken.js does it, with the same secret.
    let now = chrono::Utc::now().timestamp();
    let token = chef_server::intake_token::make_token_at(
        "test-intake-secret",
        &id.to_string(),
        now - 86_400,
    );
    assert_eq!(
        get(format!("/api/intake/{id}?token={token}"))
            .send(&app)
            .await
            .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn emails_go_to_the_outbox_with_the_chefs_reply_to(pool: PgPool) {
    let (app, state) = app_with(pool.clone(), common::test_config());
    let c = Chef::new(&pool, &app, "Mail Kitchen").await;
    c.put(
        "/api/settings",
        json!({ "email": "hello@mail-kitchen.example" }),
    )
    .await;
    submit_intake(&app, "mail-kitchen").await;
    // Intake emails are sent in the background.
    for _ in 0..50 {
        if state.outbox.messages().len() >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let mail = state.outbox.messages();
    let to_client = mail
        .iter()
        .find(|m| m.to == "lead@example.com")
        .expect("client mail");
    assert_eq!(
        to_client.reply_to.as_deref(),
        Some("hello@mail-kitchen.example")
    );
    assert!(to_client.subject.contains("Mail Kitchen"));
    assert!(to_client.html.contains("Hi Lead,"));
    let to_chef = mail
        .iter()
        .find(|m| m.to == "hello@mail-kitchen.example")
        .expect("chef notification");
    assert_eq!(to_chef.subject, "New client inquiry from Lead");

    // Meal plan PDF to a client
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "email": "jo@example.com" }),
        )
        .await;
    let s = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-03-15" }),
        )
        .await;
    // Bytes in the request are ignored: the server renders the attachment
    // (R1: otherwise any account could mail arbitrary files to any address).
    let sent = c
        .post(
            &format!("/api/sessions/{}/email-pdf", s.body["id"]),
            json!({ "pdf_base64": "data:application/pdf;base64,TVqQAAMAAAAEAAAA" }),
        )
        .await;
    assert_eq!(
        sent.body,
        json!({ "success": true, "sent_to": "jo@example.com" })
    );
    let plan = state.outbox.messages().into_iter().last().unwrap();
    assert_eq!(plan.subject, "Your meal plan — Sunday, March 15");
    assert_eq!(plan.attachments[0].0, "meal-plan-2026-03-15.pdf");
    assert!(plan.attachments[0].1.starts_with(b"%PDF-"));
    assert!(plan.attachments[0].1.len() > 1000, "a rendered meal plan");
    assert_eq!(plan.reply_to.as_deref(), Some("hello@mail-kitchen.example"));
    assert!(
        c.get(&format!("/api/sessions/{}", s.body["id"])).await.body["pdf_emailed_at"].is_string()
    );

    let no_body = c
        .post(
            &format!("/api/sessions/{}/email-pdf", s.body["id"]),
            json!({}),
        )
        .await;
    assert_eq!(no_body.status, StatusCode::OK, "{}", no_body.body);

    // Invoice email advances draft → sent
    let inv = c
        .post(
            &format!("/api/invoices/from-client/{}", client.body["id"]),
            json!({}),
        )
        .await;
    let emailed = c
        .post(
            &format!("/api/invoices/{}/email", inv.body["id"]),
            json!({ "pdf_base64": "TVqQAAMAAAAEAAAA" }),
        )
        .await;
    assert_eq!(emailed.status, StatusCode::OK, "{}", emailed.body);
    let invoice_mail = state.outbox.messages().into_iter().last().unwrap();
    assert!(invoice_mail.attachments[0].1.starts_with(b"%PDF-"));
    assert!(
        invoice_mail
            .subject
            .starts_with("Your invoice from Mail Kitchen — INV-")
    );
    assert_eq!(
        c.get(&format!("/api/invoices/{}", inv.body["id"]))
            .await
            .body["status"],
        "sent"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn client_emails_are_rate_limited_per_account(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Busy Kitchen").await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "email": "jo@example.com" }),
        )
        .await;
    let inv = c
        .post(
            &format!("/api/invoices/from-client/{}", client.body["id"]),
            json!({}),
        )
        .await;
    let url = format!("/api/invoices/{}/email", inv.body["id"]);
    for _ in 0..30 {
        assert_eq!(c.post(&url, json!({})).await.status, StatusCode::OK);
    }
    let limited = c.post(&url, json!({})).await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.body["code"], "RATE_LIMITED");

    // Another account has its own budget.
    let other = Chef::new(&pool, &app, "Quiet Kitchen").await;
    let client = other
        .post(
            "/api/clients",
            json!({ "name": "Al", "email": "al@example.com" }),
        )
        .await;
    let inv = other
        .post(
            &format!("/api/invoices/from-client/{}", client.body["id"]),
            json!({}),
        )
        .await;
    let ok = other
        .post(
            &format!("/api/invoices/{}/email", inv.body["id"]),
            json!({}),
        )
        .await;
    assert_eq!(ok.status, StatusCode::OK);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn intake_submissions_are_rate_limited_per_ip(pool: PgPool) {
    let app = app(pool.clone());
    Chef::new(&pool, &app, "Popular Kitchen").await;
    let body = json!({ "slug": "popular-kitchen", "name": "Lead", "terms_accepted": true });
    for _ in 0..20 {
        let res = post_req("/api/intake", body.clone())
            .ip("203.0.113.9")
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::CREATED);
    }
    let limited = post_req("/api/intake", body.clone())
        .ip("203.0.113.9")
        .send(&app)
        .await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.body["code"], "RATE_LIMITED");
    let other_ip = post_req("/api/intake", body)
        .ip("203.0.113.10")
        .send(&app)
        .await;
    assert_eq!(other_ip.status, StatusCode::CREATED);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn day_before_reminders_are_opt_in_timezone_aware_and_sent_once(pool: PgPool) {
    use chef_server::reminders::{RunStats, run_once};
    let (app, state) = app_with(pool.clone(), Config::for_tests(JWT_SECRET));
    let c = Chef::new(&pool, &app, "Reminder Kitchen").await;
    let at = |s: &str| chrono::DateTime::parse_from_rfc3339(s).unwrap().to_utc();
    let sent = |n| RunStats { sent: n, failed: 0 };

    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Rae", "email": "rae@example.com", "address": "1 Elm St",
                    "parking_notes": "Driveway on the left", "allergens": ["Peanuts"] }),
        )
        .await;
    let recipe = c
        .post("/api/recipes", json!({ "name": "Lentil Soup" }))
        .await;
    // 10:00 in Chicago on Nov 3 (CST, UTC-6) is 16:00 UTC.
    let session = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-03",
                    "session_time": "10:00", "recipe_ids": [recipe.body["id"]] }),
        )
        .await;
    let sid = session.body["id"].as_i64().unwrap();

    // Off by default: nothing goes out.
    assert_eq!(
        run_once(&state, at("2026-11-02T16:30:00Z")).await.unwrap(),
        sent(0)
    );

    let bad = c
        .put("/api/settings", json!({ "timezone": "Mars/Olympus" }))
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let on = c
        .put(
            "/api/settings",
            json!({ "remind_client": true, "remind_chef": 1,
                    "timezone": "America/Chicago", "email": "hello@reminder.example" }),
        )
        .await;
    assert_eq!(on.status, StatusCode::OK, "{}", on.body);
    assert_eq!(on.body["remind_client"], 1);
    assert_eq!(on.body["remind_chef"], 1);
    assert_eq!(on.body["timezone"], "America/Chicago");

    // 25h before the Chicago start: not yet (it would be, read as UTC).
    assert_eq!(
        run_once(&state, at("2026-11-02T15:00:00Z")).await.unwrap(),
        sent(0)
    );
    // 23.5h before: both reminders, once.
    let before = state.outbox.messages().len();
    assert_eq!(
        run_once(&state, at("2026-11-02T16:30:00Z")).await.unwrap(),
        sent(2)
    );
    assert_eq!(
        run_once(&state, at("2026-11-02T16:35:00Z")).await.unwrap(),
        sent(0)
    );
    let mail = state.outbox.messages()[before..].to_vec();
    let to_client = mail.iter().find(|m| m.to == "rae@example.com").unwrap();
    assert_eq!(
        to_client.subject,
        "Reminder: your meal prep session Tuesday, November 3 at 10:00 AM"
    );
    assert_eq!(
        to_client.reply_to.as_deref(),
        Some("hello@reminder.example")
    );
    assert!(to_client.html.contains("Lentil Soup"));
    let to_chef = mail
        .iter()
        .find(|m| m.to == "hello@reminder.example")
        .unwrap();
    assert!(to_chef.subject.starts_with("Coming up: Rae"));
    assert!(to_chef.html.contains("Peanuts"));
    assert!(to_chef.html.contains("Driveway on the left"));
    assert!(to_chef.text.contains(&format!("/sessions/{sid}")));
    let detail = c.get(&format!("/api/sessions/{sid}")).await.body;
    assert!(detail["client_reminder_sent_at"].is_string());
    assert!(detail["chef_reminder_sent_at"].is_string());

    // Re-saving the same time keeps the markers; moving it resends.
    c.put(
        &format!("/api/sessions/{sid}"),
        json!({ "session_time": "10:00" }),
    )
    .await;
    assert!(
        c.get(&format!("/api/sessions/{sid}")).await.body["client_reminder_sent_at"].is_string()
    );
    c.put(
        &format!("/api/sessions/{sid}"),
        json!({ "session_time": "11:00" }),
    )
    .await;
    assert!(c.get(&format!("/api/sessions/{sid}")).await.body["client_reminder_sent_at"].is_null());
    assert_eq!(
        run_once(&state, at("2026-11-02T17:30:00Z")).await.unwrap(),
        sent(2)
    );

    // Cancelled sessions and clients without email.
    let quiet = c.post("/api/clients", json!({ "name": "No Email" })).await;
    let cancelled = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-04",
                    "session_time": "09:00", "status": "Pending" }),
        )
        .await;
    c.put(
        &format!("/api/sessions/{}", cancelled.body["id"]),
        json!({ "status": "Cancelled" }),
    )
    .await;
    c.post(
        "/api/sessions",
        json!({ "client_id": quiet.body["id"], "session_date": "2026-11-04", "session_time": "09:00" }),
    )
    .await;
    // Only the chef's prep sheet for the client without an email.
    assert_eq!(
        run_once(&state, at("2026-11-03T16:00:00Z")).await.unwrap(),
        sent(1)
    );
    assert_eq!(
        state.outbox.messages().last().unwrap().to,
        "hello@reminder.example"
    );

    // Another account's opted-out sessions are untouched.
    let other = Chef::new(&pool, &app, "Quiet Kitchen").await;
    let oc = other
        .post(
            "/api/clients",
            json!({ "name": "Oz", "email": "oz@example.com" }),
        )
        .await;
    other
        .post(
            "/api/sessions",
            json!({ "client_id": oc.body["id"], "session_date": "2026-11-05", "session_time": "09:00" }),
        )
        .await;
    assert_eq!(
        run_once(&state, at("2026-11-04T12:00:00Z")).await.unwrap(),
        sent(0)
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_failed_reminder_is_released_for_the_next_run(pool: PgPool) {
    use chef_server::reminders::{RunStats, run_once};
    // SMTP pointed at a closed local port: every send fails fast.
    let config = Config {
        email_host: "127.0.0.1".into(),
        email_port: 9,
        email_user: "user".into(),
        email_pass: "pass".into(),
        email_from: "Platebook <noreply@example.com>".into(),
        ..Config::for_tests(JWT_SECRET)
    };
    let (app, state) = app_with(pool.clone(), config);
    let c = Chef::new(&pool, &app, "Flaky Kitchen").await;
    c.put("/api/settings", json!({ "remind_client": true }))
        .await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Flo", "email": "flo@example.com" }),
        )
        .await;
    let s = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-03", "session_time": "10:00" }),
        )
        .await;
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-02T12:00:00Z")
        .unwrap()
        .to_utc();
    assert_eq!(
        run_once(&state, now).await.unwrap(),
        RunStats { sent: 0, failed: 1 }
    );
    // Rolled back, so the next run tries again.
    assert!(
        c.get(&format!("/api/sessions/{}", s.body["id"])).await.body["client_reminder_sent_at"]
            .is_null()
    );
    assert_eq!(
        run_once(&state, now).await.unwrap(),
        RunStats { sent: 0, failed: 1 }
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_reminder_locked_elsewhere_is_skipped_then_sent(pool: PgPool) {
    use chef_server::reminders::{RunStats, run_once};
    let (app, state) = app_with(pool.clone(), Config::for_tests(JWT_SECRET));
    let c = Chef::new(&pool, &app, "Busy Kitchen").await;
    c.put("/api/settings", json!({ "remind_client": true }))
        .await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Lu", "email": "lu@example.com" }),
        )
        .await;
    let s = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-03", "session_time": "10:00" }),
        )
        .await;
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-02T12:00:00Z")
        .unwrap()
        .to_utc();

    // Another instance (or a chef's edit) holds the row: skipped, not waited on.
    let mut other = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM sessions WHERE id = $1 FOR UPDATE")
        .bind(s.body["id"].as_i64().unwrap())
        .execute(&mut *other)
        .await
        .unwrap();
    assert_eq!(
        run_once(&state, now).await.unwrap(),
        RunStats { sent: 0, failed: 0 }
    );
    other.rollback().await.unwrap();
    assert_eq!(
        run_once(&state, now).await.unwrap(),
        RunStats { sent: 1, failed: 0 }
    );

    // Cancelled before its turn: never sent.
    let later = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-11-03", "session_time": "11:00" }),
        )
        .await;
    c.put(
        &format!("/api/sessions/{}", later.body["id"]),
        json!({ "status": "Cancelled" }),
    )
    .await;
    assert_eq!(
        run_once(&state, now).await.unwrap(),
        RunStats { sent: 0, failed: 0 }
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn manual_payment_actions_update_and_audit(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Pay Kitchen").await;
    let id = submit_intake(&app, "pay-kitchen").await["id"]
        .as_i64()
        .unwrap();

    let mode = c.get("/api/payments/mode").await.body;
    assert_eq!(mode["mode"], "manual");
    assert_eq!(mode["online_enabled"], false);
    let keys: Vec<&String> = mode["statuses"].as_object().unwrap().keys().collect();
    assert_eq!(keys[0], "not_requested", "status order is preserved");
    assert_eq!(
        mode["actions"]["mark_paid_offline"]["intake_status"],
        "deposit_paid"
    );

    let paid = c
        .post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "mark_paid_offline", "amount": "75", "notes": " cash " }),
        )
        .await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.body);
    assert_eq!(paid.body["payment_status"], "paid_offline");
    assert_eq!(paid.body["status"], "deposit_paid");
    assert_eq!(paid.body["payment_notes"], "cash");
    assert_eq!(paid.body["booking_fee_amount"], 75.0);
    assert!(paid.body["booking_fee_paid_at"].is_string());

    let status = c.get(&format!("/api/payments/status/{id}")).await.body;
    assert_eq!(status["mode"], "manual");
    assert_eq!(status["payment_status"], "paid_offline");

    let reset = c
        .post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "reset" }),
        )
        .await;
    assert_eq!(reset.body["booking_fee_paid_at"], Value::Null);

    let bad = c
        .post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "nope" }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.body["code"], "INVALID_ACTION");
    assert_eq!(bad.body["valid_actions"].as_array().unwrap().len(), 6);
    let neg = c
        .post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "waive", "amount": -1 }),
        )
        .await;
    assert_eq!(neg.status, StatusCode::BAD_REQUEST);

    let events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM payment_events WHERE intake_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 2, "one audit row per successful action");

    let disabled = c
        .post("/api/payments/checkout", json!({ "intake_id": id }))
        .await;
    assert_eq!(disabled.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(disabled.body["code"], "PAYMENTS_DISABLED");
    let webhook = post_req("/api/payments/webhook", json!({}))
        .send(&app)
        .await;
    assert_eq!(webhook.body["received"], true);
}

const WHSEC: &str = "whsec_test";

fn stripe_config(api_base: &str) -> Config {
    Config {
        payments_mode: PaymentsMode::Stripe,
        stripe_secret_key: Some("sk_test_123".into()),
        stripe_webhook_secret: Some(WHSEC.into()),
        stripe_api_base: api_base.into(),
        ..Config::for_tests(JWT_SECRET)
    }
}

async fn send_webhook(app: &Router, body: &Value, secret: &str) -> common::TestResponse {
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let raw = serde_json::to_vec(body).unwrap();
    let t = chrono::Utc::now().timestamp();
    let sig = chef_server::stripe::sign(secret, t, &raw);
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::post("/api/payments/webhook")
                .header("content-type", "application/json")
                .header("stripe-signature", format!("t={t},v1={sig}"))
                .body(Body::from(raw))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    common::TestResponse {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text: String::from_utf8_lossy(&bytes).into_owned(),
        bytes: bytes.to_vec(),
    }
}

/// A fake Stripe API: records each checkout created (auth header, form
/// body), answers a retrieve with `status`, and records expired session ids.
struct FakeStripe {
    url: String,
    created: Arc<Mutex<Vec<(String, String)>>>,
    expired: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<String>>,
}

async fn fake_stripe() -> FakeStripe {
    let created: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let expired: Arc<Mutex<Vec<String>>> = Arc::default();
    let status = Arc::new(Mutex::new("open".to_string()));
    let (log, gone, st) = (created.clone(), expired.clone(), status.clone());
    let url = fake_server(
        Router::new()
            .route(
                "/v1/checkout/sessions",
                post(move |headers: HeaderMap, body: Bytes| {
                    let log = log.clone();
                    async move {
                        let mut log = log.lock().unwrap();
                        log.push((
                            headers["authorization"].to_str().unwrap().to_string(),
                            String::from_utf8(body.to_vec()).unwrap(),
                        ));
                        let id = format!("cs_test_{}", log.len());
                        Json(json!({ "id": id, "url": format!("https://checkout.stripe.test/{id}") }))
                    }
                }),
            )
            .route(
                "/v1/checkout/sessions/{id}",
                get_route(move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let st = st.clone();
                    async move { Json(json!({ "id": id, "status": *st.lock().unwrap() })) }
                }),
            )
            .route(
                "/v1/checkout/sessions/{id}/expire",
                post(move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let gone = gone.clone();
                    async move {
                        gone.lock().unwrap().push(id.clone());
                        Json(json!({ "id": id, "status": "expired" }))
                    }
                }),
            ),
    )
    .await;
    FakeStripe {
        url,
        created,
        expired,
        status,
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn stripe_checkout_and_signed_webhooks(pool: PgPool) {
    let fake = fake_stripe().await;
    let (seen, stripe) = (fake.created.clone(), fake.url.clone());
    let (app, _) = app_with(pool.clone(), stripe_config(&stripe));
    let c = Chef::new(&pool, &app, "Stripe Kitchen").await;
    c.put("/api/settings", json!({ "deposit_cents": 6000 }))
        .await;
    let id = submit_intake(&app, "stripe-kitchen").await["id"]
        .as_i64()
        .unwrap();

    let checkout = c
        .post("/api/payments/checkout", json!({ "intake_id": id }))
        .await;
    assert_eq!(checkout.status, StatusCode::OK, "{}", checkout.body);
    assert_eq!(
        checkout.body["checkout_url"],
        "https://checkout.stripe.test/cs_test_1"
    );
    let (auth, form) = seen.lock().unwrap()[0].clone();
    assert_eq!(auth, "Bearer sk_test_123");
    assert!(form.contains("line_items%5B0%5D%5Bprice_data%5D%5Bunit_amount%5D=6000"));
    assert!(form.contains(&format!("metadata%5Bintake_id%5D={id}")));
    // R4: refunds and failures arrive as PaymentIntent/Charge events, which
    // only carry the PaymentIntent's metadata.
    assert!(form.contains(&format!(
        "payment_intent_data%5Bmetadata%5D%5Bintake_id%5D={id}"
    )));
    assert!(form.contains("customer_email=lead%40example.com"));
    let pending = c.get(&format!("/api/payments/status/{id}")).await.body;
    assert_eq!(pending["payment_status"], "payment_pending");
    assert_eq!(pending["booking_fee_amount"], 60.0);
    // Stripe sends the client back to their own booking page, with its
    // edit token, and says how it went.
    assert!(
        form.contains(&format!(
            "success_url=http%3A%2F%2Flocalhost%3A5174%2Fintake%2F{id}%3Ftoken%3D"
        )),
        "{form}"
    );
    assert!(form.contains("%26paid%3D1"), "{form}");
    assert!(form.contains("%26cancelled%3D1"), "{form}");
    // A second checkout (the client came back later) expires the open one
    // first, so only the newest can be paid.
    let twice = c
        .post("/api/payments/checkout", json!({ "intake_id": id }))
        .await;
    assert_eq!(twice.status, StatusCode::OK, "{}", twice.body);
    assert_eq!(twice.body["session_id"], "cs_test_2");
    assert_eq!(*fake.expired.lock().unwrap(), ["cs_test_1"]);

    let event = json!({
        "id": "evt_1", "type": "checkout.session.completed",
        "data": { "object": { "payment_intent": "pi_1", "amount_total": 6000,
                              "payment_status": "paid", "metadata": { "intake_id": id.to_string() } } }
    });
    let forged = send_webhook(&app, &event, "whsec_wrong").await;
    assert_eq!(forged.status, StatusCode::BAD_REQUEST);
    assert!(forged.text.starts_with("Webhook Error: "));
    assert_eq!(
        c.get(&format!("/api/payments/status/{id}")).await.body["payment_status"],
        "payment_pending"
    );

    // The chef deleted the inquiry before the client paid: the payment
    // restores it, so the money doesn't land on a hidden intake.
    assert_eq!(
        c.delete(&format!("/api/intake/{id}")).await.status,
        StatusCode::OK
    );
    let ok = send_webhook(&app, &event, WHSEC).await;
    assert_eq!(ok.body, json!({ "received": true }));
    let paid = c.get(&format!("/api/payments/status/{id}")).await.body;
    assert_eq!(paid["payment_status"], "paid");
    assert_eq!(paid["status"], "deposit_paid");
    assert!(c.get(&format!("/api/intake/{id}")).await.body["deleted_at"].is_null());
    let replay = send_webhook(&app, &event, WHSEC).await;
    assert_eq!(
        replay.body,
        json!({ "received": true, "skipped": "already processed" })
    );
    // A refund names the intake through the PaymentIntent metadata.
    let refund = json!({
        "id": "evt_2", "type": "charge.refunded",
        "data": { "object": { "status": "succeeded", "metadata": { "intake_id": id.to_string() } } }
    });
    assert_eq!(
        send_webhook(&app, &refund, WHSEC).await.body,
        json!({ "received": true })
    );
    assert_eq!(
        c.get(&format!("/api/payments/status/{id}")).await.body["payment_status"],
        "refunded"
    );
    let timeline = c.get(&format!("/api/intake/{id}/payments")).await.body;
    let labels: Vec<&str> = timeline
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Paid online", "Refunded"], "{timeline}");
    assert_eq!(timeline[0]["amount"], 60.0);

    let (account, amount): (i64, rust_decimal::Decimal) = sqlx::query_as(
        "SELECT account_id, amount FROM payment_events WHERE stripe_event_id = 'evt_1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(account, c.id);
    assert_eq!(amount, rust_decimal::Decimal::new(6000, 2));

    let no_deposit = c.put("/api/settings", json!({ "deposit_cents": 0 })).await;
    assert_eq!(no_deposit.status, StatusCode::OK);
    let other = submit_intake(&app, "stripe-kitchen").await["id"]
        .as_i64()
        .unwrap();
    let refused = c
        .post("/api/payments/checkout", json!({ "intake_id": other }))
        .await;
    assert_eq!(refused.body["code"], "NO_DEPOSIT");
}

fn ai_config(api_base: &str) -> Config {
    Config {
        anthropic_api_key: Some("sk-ant-test".into()),
        anthropic_api_base: api_base.into(),
        ..Config::for_tests(JWT_SECRET)
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn ai_routes_call_the_messages_api(pool: PgPool) {
    let captured: Arc<Mutex<Vec<Value>>> = Arc::default();
    let log = captured.clone();
    let recipe_ids: Arc<Mutex<Vec<i64>>> = Arc::default();
    let ids = recipe_ids.clone();
    let claude = fake_server(Router::new().route(
        "/v1/messages",
        post(move |headers: HeaderMap, Json(body): Json<Value>| {
            let log = log.clone();
            let ids = ids.clone();
            async move {
                assert_eq!(headers["x-api-key"], "sk-ant-test");
                assert_eq!(headers["anthropic-version"], "2023-06-01");
                log.lock().unwrap().push(body.clone());
                let text = if body["max_tokens"] == 10 {
                    "pong".to_string()
                } else if body["max_tokens"] == 4096 {
                    json!({ "name": "Toast", "ingredients": [{ "name": "bread" }] }).to_string()
                } else {
                    let mut suggested: Vec<Value> =
                        ids.lock().unwrap().iter().map(|i| json!(i)).collect();
                    suggested.push(json!(999_999));
                    suggested.push(json!("1"));
                    json!({ "suggested_recipe_ids": suggested, "reasoning": "overlap" }).to_string()
                };
                // A leading thinking block must not hide the text block.
                Json(json!({ "content": [
                    { "type": "thinking", "thinking": "" },
                    { "type": "text", "text": text },
                ] }))
            }
        }),
    ))
    .await;
    let (app, _) = app_with(pool.clone(), ai_config(&claude));
    let c = Chef::new(&pool, &app, "AI Kitchen").await;
    c.put("/api/settings", json!({ "location": "Austin" }))
        .await;
    assert_eq!(
        c.get("/api/ai/status").await.body,
        json!({ "configured": true })
    );
    assert_eq!(c.get("/api/settings").await.body["ai_configured"], true);

    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "allergens": ["Peanuts"], "household_type": "Family" }),
        )
        .await;
    let recipe = c.post("/api/recipes", json!({ "name": "Stew" })).await;
    recipe_ids
        .lock()
        .unwrap()
        .push(recipe.body["id"].as_i64().unwrap());
    let menu = c
        .post(
            "/api/ai/suggest-menu",
            json!({ "client_id": client.body["id"], "dish_count": 2 }),
        )
        .await;
    assert_eq!(menu.status, StatusCode::OK, "{}", menu.body);
    assert_eq!(
        menu.body["suggested_recipe_ids"],
        json!([recipe.body["id"]])
    );
    assert_eq!(menu.body["reasoning"], "overlap");
    let req = captured.lock().unwrap().last().unwrap().clone();
    assert_eq!(req["model"], "claude-sonnet-5-5");
    assert_eq!(req["thinking"], json!({ "type": "between_tools" }));
    assert!(req["system"].as_str().unwrap().contains("AI Kitchen"));
    assert!(req["system"].as_str().unwrap().contains(" in Austin"));
    let user = req["messages"][0]["content"].as_str().unwrap();
    assert!(user.contains("Allergens to EXCLUDE completely: Peanuts."));
    assert!(user.contains("Suggest 2 dishes"));
    assert!(user.contains("Session window: 3 hours."));

    let parsed = c
        .post(
            "/api/recipes/import/text",
            json!({ "raw_text": "Toast: toast some bread, then butter it." }),
        )
        .await;
    assert_eq!(parsed.status, StatusCode::OK, "{}", parsed.body);
    assert_eq!(parsed.body["parsed"]["name"], "Toast");
    assert_eq!(parsed.body["confidence"], 100 - 30 - 15 - 10);
    assert_eq!(parsed.body["confidence_level"], "low");

    let ping = c.post("/api/ai/test", json!({})).await;
    assert_eq!(ping.body, json!({ "configured": true, "working": true }));
    let ping_req = captured.lock().unwrap().last().unwrap().clone();
    assert_eq!(ping_req["model"], "claude-haiku-5-5");
    assert_eq!(ping_req["thinking"], json!({ "type": "disabled" }));

    let missing = c
        .post("/api/ai/suggest-menu", json!({ "client_id": 999_999 }))
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn ai_and_import_guards_without_a_key(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "No Key Kitchen").await;
    let r = c
        .post("/api/ai/suggest-menu", json!({ "client_id": 1 }))
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(r.body["code"], "API_KEY_MISSING");
    let t = c.post("/api/ai/test", json!({})).await;
    assert_eq!(t.body["configured"], false);
    let short = c
        .post(
            "/api/recipes/import/text",
            json!({ "raw_text": "too short" }),
        )
        .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    let long = c
        .post(
            "/api/recipes/import/text",
            json!({ "raw_text": "x".repeat(50_001) }),
        )
        .await;
    assert_eq!(
        long.body["error"],
        "Text too long — please trim to under 50 KB"
    );

    for url in [
        "ftp://example.com",
        "",
        "http://127.0.0.1:1/",
        "http://169.254.169.254/latest",
        "http://[::1]/",
    ] {
        let r = c
            .post("/api/recipes/import/url", json!({ "url": url }))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{url}");
    }
    // 20 imports per 15 minutes per account (counting the requests above)
    let mut last = StatusCode::OK;
    for _ in 0..20 {
        last = c
            .post(
                "/api/recipes/import/url",
                json!({ "url": "http://localhost/" }),
            )
            .await
            .status;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    // ...and another account has its own budget
    let other = Chef::new(&pool, &app, "Other Kitchen").await;
    let r = other
        .post(
            "/api/recipes/import/url",
            json!({ "url": "http://localhost/" }),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn security_headers_and_spa_serving(pool: PgPool) {
    let dist = std::env::temp_dir().join(format!("chef-dist-{}", std::process::id()));
    std::fs::create_dir_all(dist.join("assets")).unwrap();
    std::fs::write(
        dist.join("index.html"),
        "<!doctype html><title>Chef</title>",
    )
    .unwrap();
    std::fs::write(dist.join("assets/app.js"), "console.log(1)").unwrap();
    let (app, _) = app_with(
        pool.clone(),
        Config {
            client_dist: Some(dist.to_string_lossy().into_owned()),
            ..Config::for_tests(JWT_SECRET)
        },
    );

    let api = get("/api/clients").send(&app).await;
    assert_eq!(api.status, StatusCode::UNAUTHORIZED);
    let csp = api.headers["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("style-src 'self' 'unsafe-inline' https://fonts.googleapis.com"));
    assert!(csp.contains("font-src 'self' data: https://fonts.gstatic.com"));
    assert_eq!(api.headers["x-frame-options"], "SAMEORIGIN");
    assert_eq!(api.headers["x-content-type-options"], "nosniff");

    let page = get("/clients/42").send(&app).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.text.contains("<title>Chef</title>"),
        "client routes get index.html"
    );
    let asset = get("/assets/app.js").send(&app).await;
    assert_eq!(asset.text, "console.log(1)");
    let (_, cookie) = common::make_account(&pool, "SPA Kitchen").await;
    let unknown = get("/api/nope").cookie(cookie).send(&app).await;
    assert_eq!(
        unknown.body,
        json!({ "error": "Not found", "code": "NOT_FOUND" })
    );
    std::fs::remove_dir_all(&dist).unwrap();
}

/// A failed meal-plan or invoice email is remembered (so the screens can say
/// so after the chef leaves) and cleared by the next successful send. SMTP
/// here points at a closed local port: the connection is refused, retried
/// once, and the request fails.
#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_emails_are_recorded_and_cleared_by_a_retry(pool: PgPool) {
    let mut broken = common::test_config();
    broken.email_host = "127.0.0.1".into();
    broken.email_port = 1;
    broken.email_user = "chef".into();
    broken.email_pass = "secret".into();
    let (broken_app, _) = app_with(pool.clone(), broken);
    let (working_app, state) = app_with(pool.clone(), common::test_config());

    let c = Chef::new(&pool, &broken_app, "Flaky Mail Kitchen").await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Jo", "email": "jo@example.com" }),
        )
        .await;
    let s = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-03-15" }),
        )
        .await;
    let session_url = format!("/api/sessions/{}", s.body["id"]);
    let inv = c
        .post(
            &format!("/api/invoices/from-client/{}", client.body["id"]),
            json!({}),
        )
        .await;
    let invoice_url = format!("/api/invoices/{}", inv.body["id"]);

    // Fresh rows: nothing sent, nothing failed.
    let fresh = c.get(&session_url).await.body;
    assert!(fresh["pdf_email_failed_at"].is_null() && fresh["pdf_emailed_at"].is_null());
    let fresh = c.get(&invoice_url).await.body;
    assert!(fresh["email_failed_at"].is_null() && fresh["emailed_at"].is_null());

    // SMTP down: both sends fail, and the failure is recorded.
    let failed = c.post(&format!("{session_url}/email-pdf"), json!({})).await;
    assert_eq!(
        failed.status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{}",
        failed.body
    );
    let failed = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(
        failed.status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{}",
        failed.body
    );
    let session = c.get(&session_url).await.body;
    assert!(session["pdf_email_failed_at"].is_string(), "{session}");
    assert!(session["pdf_emailed_at"].is_null());
    let invoice = c.get(&invoice_url).await.body;
    assert!(invoice["email_failed_at"].is_string(), "{invoice}");
    assert!(invoice["emailed_at"].is_null());
    assert_eq!(
        invoice["status"], "draft",
        "a failed send doesn't mark it sent"
    );

    // Retry with mail working: the failure clears and the send is recorded.
    let c = Chef {
        app: working_app,
        ..c
    };
    let ok = c.post(&format!("{session_url}/email-pdf"), json!({})).await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    let ok = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    let session = c.get(&session_url).await.body;
    assert!(session["pdf_email_failed_at"].is_null(), "{session}");
    assert!(session["pdf_emailed_at"].is_string());
    let invoice = c.get(&invoice_url).await.body;
    assert!(invoice["email_failed_at"].is_null(), "{invoice}");
    assert!(invoice["emailed_at"].is_string());
    assert_eq!(invoice["status"], "sent");
    assert_eq!(state.outbox.messages().len(), 2);
}

/// Polls the chef's view of an intake until `done` holds (booking emails are
/// sent in the background, and a failed one is retried after 2 seconds).
async fn intake_once(c: &Chef, url: &str, done: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..100 {
        let intake = c.get(url).await.body;
        if done(&intake) {
            return intake;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!(
        "intake never reached the expected state: {}",
        c.get(url).await.body
    );
}

/// A failed booking confirmation (to the client) or new-booking alert (to the
/// chef) is recorded on the intake, and the chef's "Try again" resends the
/// confirmation and clears the failure.
#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_booking_emails_are_recorded_and_the_confirmation_can_be_resent(pool: PgPool) {
    let mut broken = common::test_config();
    broken.email_host = "127.0.0.1".into();
    broken.email_port = 1;
    broken.email_user = "chef".into();
    broken.email_pass = "secret".into();
    let (broken_app, _) = app_with(pool.clone(), broken);
    let (working_app, state) = app_with(pool.clone(), common::test_config());

    let c = Chef::new(&pool, &broken_app, "Flaky Booking Kitchen").await;
    let intake = submit_intake(&broken_app, "flaky-booking-kitchen").await;
    assert!(intake["confirmation_email_failed_at"].is_null());
    let url = format!("/api/intake/{}", intake["id"]);

    // SMTP down: both background sends fail, and both failures are recorded.
    let failed = intake_once(&c, &url, |i| {
        i["confirmation_email_failed_at"].is_string() && i["chef_alert_failed_at"].is_string()
    })
    .await;
    assert!(failed["confirmation_emailed_at"].is_null(), "{failed}");
    // The list shows it too.
    let list = c.get("/api/intake").await.body;
    assert!(
        list[0]["confirmation_email_failed_at"].is_string(),
        "{list}"
    );

    // "Try again" while SMTP is still down fails and keeps the failure.
    let retry = c
        .post(&format!("{url}/resend-confirmation"), json!({}))
        .await;
    assert_eq!(
        retry.status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{}",
        retry.body
    );
    assert!(c.get(&url).await.body["confirmation_email_failed_at"].is_string());

    // With mail working, the resend goes to the client with their edit link
    // and clears the failure. The chef alert failure stays: it isn't resent.
    let c = Chef {
        app: working_app.clone(),
        ..c
    };
    let ok = c
        .post(&format!("{url}/resend-confirmation"), json!({}))
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    assert_eq!(ok.body["sent_to"], "lead@example.com");
    let mail = state.outbox.messages();
    assert_eq!(mail.len(), 1);
    assert_eq!(mail[0].to, "lead@example.com");
    assert!(
        mail[0]
            .text
            .contains(&format!("/intake/{}?token=", intake["id"])),
        "{}",
        mail[0].text
    );
    let resent = c.get(&url).await.body;
    assert!(resent["confirmation_email_failed_at"].is_null(), "{resent}");
    assert!(resent["confirmation_emailed_at"].is_string(), "{resent}");
    assert!(resent["chef_alert_failed_at"].is_string(), "{resent}");

    // Signed out: refused.
    assert_eq!(
        post_req(format!("{url}/resend-confirmation"), json!({}))
            .send(&working_app)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    // An inquiry without an email address has nothing to resend.
    let no_email = post_req(
        "/api/intake",
        json!({ "slug": "flaky-booking-kitchen", "name": "Phone Only", "terms_accepted": true }),
    )
    .send(&working_app)
    .await;
    let res = c
        .post(
            &format!("/api/intake/{}/resend-confirmation", no_email.body["id"]),
            json!({}),
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST, "{}", res.body);
    assert_eq!(res.body["code"], "NO_EMAIL");
}

/// With mail working, a booking records the confirmation as sent and no
/// failures.
#[sqlx::test(migrator = "MIGRATOR")]
async fn delivered_booking_emails_are_recorded(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Steady Booking Kitchen").await;
    let intake = submit_intake(&app, "steady-booking-kitchen").await;
    let url = format!("/api/intake/{}", intake["id"]);
    let sent = intake_once(&c, &url, |i| i["confirmation_emailed_at"].is_string()).await;
    assert!(sent["confirmation_email_failed_at"].is_null(), "{sent}");
    assert!(sent["chef_alert_failed_at"].is_null(), "{sent}");
}

/// The client's edit link returns what they filled in, never the chef's
/// private notes, payment details or bookkeeping; the chef still sees all.
#[sqlx::test(migrator = "MIGRATOR")]
async fn the_clients_edit_link_never_returns_chef_only_fields(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Private Notes Kitchen").await;
    let intake = submit_intake(&app, "private-notes-kitchen").await;
    let (id, token) = (&intake["id"], intake["edit_token"].as_str().unwrap());
    let chef_view = c
        .put(
            &format!("/api/intake/{id}"),
            json!({ "internal_notes": "Haggles on price", "payment_notes": "Owes $40",
                    "status": "reviewed_by_chef" }),
        )
        .await;
    assert_eq!(chef_view.body["internal_notes"], "Haggles on price");

    let url = format!("/api/intake/{id}?token={token}");
    let read = get(url.clone()).send(&app).await;
    let edit = put_req(url, json!({ "phone": "555-0102" }))
        .send(&app)
        .await;
    for res in [&read, &edit] {
        assert_eq!(res.status, StatusCode::OK, "{}", res.body);
        let body = res.body.as_object().unwrap();
        for key in [
            "internal_notes",
            "payment_notes",
            "stripe_session_id",
            "stripe_payment_intent",
            "converted_client_id",
            "pdf_path",
            "account_id",
        ] {
            assert!(!body.contains_key(key), "{key} leaked: {}", res.body);
        }
        // What the booking form needs is all there.
        assert_eq!(body["name"], "Lead");
        assert_eq!(body["allergens"], json!(["Soy"]));
        assert_eq!(body["kitchen_notes"], "Gas stove");
        assert_eq!(body["chef_slug"], "private-notes-kitchen");
        assert_eq!(body["status"], "reviewed_by_chef");
    }
    assert_eq!(edit.body["phone"], "555-0102");

    // The chef's own view is unchanged.
    let chef_read = c.get(&format!("/api/intake/{id}")).await.body;
    assert_eq!(chef_read["internal_notes"], "Haggles on price");
    assert_eq!(chef_read["payment_notes"], "Owes $40");
    assert_eq!(chef_read["phone"], "555-0102");
}

/// Deleting an intake hides it but keeps it and its payment history: it
/// leaves the inbox, the client's link and every action, shows under
/// ?deleted=1 and comes back with POST /restore.
#[sqlx::test(migrator = "MIGRATOR")]
async fn deleted_intakes_are_kept_and_can_be_restored(pool: PgPool) {
    let app = app(pool.clone());
    let c = Chef::new(&pool, &app, "Tidy Kitchen").await;
    let intake = submit_intake(&app, "tidy-kitchen").await;
    let (id, token) = (&intake["id"], intake["edit_token"].as_str().unwrap());
    let url = format!("/api/intake/{id}");
    let client_url = format!("{url}?token={token}");
    let paid = c
        .post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "mark_paid_offline", "amount": 50 }),
        )
        .await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.body);

    assert_eq!(c.delete(&url).await.status, StatusCode::OK);
    assert_eq!(
        c.delete(&url).await.status,
        StatusCode::NOT_FOUND,
        "already deleted"
    );

    // Out of the inbox, into the Deleted list; the chef can still open it.
    assert_eq!(c.get("/api/intake").await.body, json!([]));
    let deleted = c.get("/api/intake?deleted=1").await.body;
    assert_eq!(deleted.as_array().unwrap().len(), 1, "{deleted}");
    assert!(deleted[0]["deleted_at"].is_string());
    let chef_view = c.get(&url).await;
    assert_eq!(chef_view.status, StatusCode::OK);
    assert!(chef_view.body["deleted_at"].is_string());

    // The history is kept: the event still points at the intake.
    let timeline = c.get(&format!("{url}/payments")).await.body;
    assert_eq!(timeline.as_array().unwrap().len(), 1, "{timeline}");
    assert_eq!(timeline[0]["label"], "Mark Deposit Paid");
    assert_eq!(timeline[0]["event_type"], "manual.mark_paid_offline");
    assert_eq!(timeline[0]["amount"], 50.0);

    // Every action and the client's link answer 404 until it's restored.
    assert_eq!(
        get(client_url.clone()).send(&app).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        put_req(client_url.clone(), json!({ "phone": "1" }))
            .send(&app)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    for res in [
        c.put(&url, json!({ "internal_notes": "x" })).await,
        c.post(&format!("{url}/convert"), json!({})).await,
        c.post(&format!("{url}/resend-confirmation"), json!({}))
            .await,
        c.post(
            "/api/payments/manual/action",
            json!({ "intake_id": id, "action": "waive" }),
        )
        .await,
    ] {
        assert_eq!(res.status, StatusCode::NOT_FOUND, "{}", res.body);
    }

    let restored = c.post(&format!("{url}/restore"), json!({})).await;
    assert_eq!(restored.status, StatusCode::OK, "{}", restored.body);
    assert!(restored.body["deleted_at"].is_null());
    assert_eq!(restored.body["payment_status"], "paid_offline");
    assert_eq!(
        c.post(&format!("{url}/restore"), json!({})).await.status,
        StatusCode::NOT_FOUND,
        "not deleted"
    );
    assert_eq!(c.get("/api/intake").await.body[0]["id"], *id);
    assert_eq!(
        get(client_url).send(&app).await.status,
        StatusCode::OK,
        "the client's link works again"
    );
}

/// The client pays from their own booking page with the edit token: no token
/// or a bad one is refused, a waived or paid deposit can't be charged, a
/// checkout already paid gives 409, and the chef can copy that page's link.
#[sqlx::test(migrator = "MIGRATOR")]
async fn clients_pay_the_deposit_from_their_booking_page(pool: PgPool) {
    let fake = fake_stripe().await;
    let (app, _) = app_with(pool.clone(), stripe_config(&fake.url));
    let c = Chef::new(&pool, &app, "Deposit Kitchen").await;
    c.put("/api/settings", json!({ "deposit_cents": 5000 }))
        .await;
    let intake = submit_intake(&app, "deposit-kitchen").await;
    let (id, token) = (&intake["id"], intake["edit_token"].as_str().unwrap());
    let url = format!("/api/intake/{id}/checkout");

    assert_eq!(
        post_req(url.clone(), json!({})).send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_req(format!("{url}?token=bad"), json!({}))
            .send(&app)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let paid = post_req(format!("{url}?token={token}"), json!({}))
        .send(&app)
        .await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.body);
    assert_eq!(
        paid.body["checkout_url"],
        "https://checkout.stripe.test/cs_test_1"
    );

    // Their page now shows the deposit as in progress (and still hides the
    // chef's notes).
    let mine = get(format!("/api/intake/{id}?token={token}"))
        .send(&app)
        .await
        .body;
    assert_eq!(mine["payment_status"], "payment_pending", "{mine}");
    assert_eq!(mine["booking_fee_amount"], 50.0);
    assert!(mine.get("payment_notes").is_none());

    // The first checkout was paid but the webhook hasn't arrived yet: no
    // second charge.
    *fake.status.lock().unwrap() = "complete".into();
    let again = post_req(format!("{url}?token={token}"), json!({}))
        .send(&app)
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT, "{}", again.body);
    assert_eq!(again.body["code"], "ALREADY_PAID");
    assert_eq!(fake.created.lock().unwrap().len(), 1);

    // A waived deposit can't be charged.
    c.post(
        "/api/payments/manual/action",
        json!({ "intake_id": id, "action": "waive" }),
    )
    .await;
    let waived = post_req(format!("{url}?token={token}"), json!({}))
        .send(&app)
        .await;
    assert_eq!(waived.status, StatusCode::BAD_REQUEST, "{}", waived.body);
    assert_eq!(waived.body["code"], "DEPOSIT_SETTLED");

    // The chef's copy of the client's link opens their page.
    let link = c.get(&format!("/api/intake/{id}/client-link")).await;
    assert_eq!(link.status, StatusCode::OK, "{}", link.body);
    // Tokens carry the time they were minted, so compare what it does, not
    // the string from booking time.
    let link = link.body["url"].as_str().unwrap();
    let link_token = link
        .strip_prefix(&format!("http://localhost:5174/intake/{id}?token="))
        .unwrap_or_else(|| panic!("unexpected link: {link}"));
    let opened = get(format!("/api/intake/{id}?token={link_token}"))
        .send(&app)
        .await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.body);
    assert_eq!(opened.body["name"], "Lead");

    // Deleted: no checkout, no link.
    c.delete(&format!("/api/intake/{id}")).await;
    c.post(
        "/api/payments/manual/action",
        json!({ "intake_id": id, "action": "reset" }),
    )
    .await;
    assert_eq!(
        post_req(format!("{url}?token={token}"), json!({}))
            .send(&app)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        c.get(&format!("/api/intake/{id}/client-link")).await.status,
        StatusCode::NOT_FOUND
    );

    // Online payments off (the default manual mode): 503.
    let manual = common::app(pool.clone());
    let m = Chef::new(&pool, &manual, "Manual Kitchen").await;
    let mi = submit_intake(&manual, "manual-kitchen").await;
    let off = post_req(
        format!(
            "/api/intake/{}/checkout?token={}",
            mi["id"],
            mi["edit_token"].as_str().unwrap()
        ),
        json!({}),
    )
    .send(&manual)
    .await;
    assert_eq!(off.status, StatusCode::SERVICE_UNAVAILABLE, "{}", off.body);
    drop(m);
}
