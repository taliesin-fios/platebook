// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Email must never report a send that didn't happen: without a transport,
//! chef actions fail truthfully (and leave invoices, sessions and intakes as
//! they were), bookings still save, reminders aren't marked sent, and the
//! sign-in/reset flows stay enumeration-safe without leaking tokens.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use chef_server::config::Config;
use chef_server::db::MIGRATOR;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;

/// A chef with one client (with an email), one session and one draft invoice.
async fn chef_with_work(pool: &PgPool, app: &axum::Router, name: &str) -> (Chef, String, String) {
    let c = Chef::new(pool, app, name).await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Avery Household", "email": "avery@example.com" }),
        )
        .await;
    assert_eq!(client.status, StatusCode::CREATED, "{}", client.body);
    let s = c
        .post(
            "/api/sessions",
            json!({ "client_id": client.body["id"], "session_date": "2026-12-03" }),
        )
        .await;
    let inv = c
        .post(
            &format!("/api/invoices/from-client/{}", client.body["id"]),
            json!({}),
        )
        .await;
    (
        c,
        format!("/api/sessions/{}", s.body["id"]),
        format!("/api/invoices/{}", inv.body["id"]),
    )
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn without_email_chef_sends_fail_and_change_nothing_but_the_failure(pool: PgPool) {
    let (app, state) = app_with(pool.clone(), no_email_config());
    let (c, session_url, invoice_url) = chef_with_work(&pool, &app, "No Mail Kitchen").await;

    let res = c.post(&format!("{session_url}/email-pdf"), json!({})).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{}", res.body);
    assert_eq!(res.body["code"], "EMAIL_NOT_CONFIGURED", "{}", res.body);
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::SERVICE_UNAVAILABLE, "{}", res.body);
    assert_eq!(res.body["code"], "EMAIL_NOT_CONFIGURED", "{}", res.body);

    let session = c.get(&session_url).await.body;
    assert!(session["pdf_emailed_at"].is_null(), "{session}");
    assert!(session["pdf_email_failed_at"].is_string(), "{session}");
    let invoice = c.get(&invoice_url).await.body;
    assert_eq!(invoice["status"], "draft", "not marked sent: {invoice}");
    assert!(invoice["emailed_at"].is_null(), "{invoice}");
    assert!(invoice["email_failed_at"].is_string(), "{invoice}");
    assert!(
        state.outbox.messages().is_empty(),
        "nothing captured as sent"
    );
    // The app knows not to offer a retry that can't work.
    assert_eq!(c.get("/api/auth/me").await.body["email_available"], false);
}

/// Polls an intake until `done` holds (booking emails run in the background).
async fn wait_for(c: &Chef, url: &str, done: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..100 {
        let v = c.get(url).await.body;
        if done(&v) {
            return v;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!(
        "never reached the expected state: {}",
        c.get(url).await.body
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn without_email_a_booking_still_saves_and_records_the_unsent_confirmation(pool: PgPool) {
    let (app, state) = app_with(pool.clone(), no_email_config());
    let c = Chef::new(&pool, &app, "No Mail Bookings").await;
    let res = post(
        "/api/intake",
        json!({ "slug": "no-mail-bookings", "name": "Rivera Family",
                "email": "rivera@example.com", "terms_accepted": true }),
    )
    .send(&app)
    .await;
    assert_eq!(res.status, StatusCode::CREATED, "{}", res.body);
    let url = format!("/api/intake/{}", res.body["id"]);

    let intake = wait_for(&c, &url, |i| {
        i["confirmation_email_failed_at"].is_string() && i["chef_alert_failed_at"].is_string()
    })
    .await;
    assert_eq!(intake["name"], "Rivera Family");
    assert!(intake["confirmation_emailed_at"].is_null(), "{intake}");

    let resend = c
        .post(&format!("{url}/resend-confirmation"), json!({}))
        .await;
    assert_eq!(
        resend.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        resend.body
    );
    assert_eq!(resend.body["code"], "EMAIL_NOT_CONFIGURED");
    assert!(c.get(&url).await.body["confirmation_emailed_at"].is_null());
    assert!(state.outbox.messages().is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn without_email_reminders_are_not_marked_sent(pool: PgPool) {
    use chef_server::reminders::run_once;
    let (app, state) = app_with(pool.clone(), no_email_config());
    let c = Chef::new(&pool, &app, "No Mail Reminders").await;
    c.put(
        "/api/settings",
        json!({ "remind_client": true, "remind_chef": true }),
    )
    .await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Kim", "email": "kim@example.com" }),
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
    let stats = run_once(&state, now).await.unwrap();
    assert_eq!(stats.sent, 0, "{stats:?}");
    let session = c.get(&format!("/api/sessions/{}", s.body["id"])).await.body;
    assert!(session["client_reminder_sent_at"].is_null(), "{session}");
    assert!(session["chef_reminder_sent_at"].is_null(), "{session}");
    assert!(state.outbox.messages().is_empty());
}

/// A `tracing` writer that keeps everything in memory.
#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuf {
    type Writer = LogBuf;
    fn make_writer(&'a self) -> LogBuf {
        self.clone()
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn without_email_link_flows_say_so_without_revealing_accounts_or_tokens(pool: PgPool) {
    let logs = LogBuf::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(logs.clone())
        .with_max_level(tracing::Level::TRACE)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let (app, state) = app_with(pool.clone(), no_email_config());
    make_account(&pool, "Link Kitchen").await; // link.kitchen@example.com
    for path in ["/api/auth/request-link", "/api/auth/forgot-password"] {
        let known = post(path, json!({ "email": "link.kitchen@example.com" }))
            .send(&app)
            .await;
        let unknown = post(path, json!({ "email": "nobody@example.com" }))
            .send(&app)
            .await;
        assert_eq!(known.status, StatusCode::OK, "{path}");
        assert_eq!(known.status, unknown.status, "{path}");
        assert_eq!(known.body, unknown.body, "{path}: same answer either way");
        assert_eq!(
            known.body["email_available"], false,
            "{path}: {}",
            known.body
        );
        assert!(!known.text.contains("token"), "{path}: {}", known.text);
    }
    assert!(state.outbox.messages().is_empty(), "no link was 'sent'");
    let logged = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(
        !logged.contains("token="),
        "a link token was logged:\n{logged}"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_captured_link_email_never_logs_the_link(pool: PgPool) {
    let logs = LogBuf::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(logs.clone())
        .with_max_level(tracing::Level::TRACE)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let (app, state) = app_with(pool.clone(), test_config());
    make_account(&pool, "Log Kitchen").await;
    for path in ["/api/auth/request-link", "/api/auth/forgot-password"] {
        let res = post(path, json!({ "email": "log.kitchen@example.com" }))
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::OK);
        assert!(!res.text.contains("token"), "{}", res.text);
    }
    // Link mail goes out after the response.
    for _ in 0..50 {
        if state.outbox.messages().len() >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(state.outbox.messages().len(), 2, "both captured");
    let logged = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(
        !logged.contains("token="),
        "a link token was logged:\n{logged}"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_booking_response_says_whether_a_confirmation_will_be_emailed(pool: PgPool) {
    for (config, expected) in [(test_config(), true), (no_email_config(), false)] {
        let (app, _) = app_with(pool.clone(), config);
        let name = if expected {
            "Mail Kitchen"
        } else {
            "Mailless Kitchen"
        };
        Chef::new(&pool, &app, name).await;
        let slug = chef_server::accounts::slugify(name);
        let with_email = post(
            "/api/intake",
            json!({ "slug": slug, "name": "A", "email": "a@example.com", "terms_accepted": true }),
        )
        .send(&app)
        .await;
        assert_eq!(
            with_email.status,
            StatusCode::CREATED,
            "{}",
            with_email.body
        );
        assert_eq!(with_email.body["confirmation_email"], expected, "{name}");
        let without = post(
            "/api/intake",
            json!({ "slug": slug, "name": "B", "phone": "555-0100", "terms_accepted": true }),
        )
        .send(&app)
        .await;
        assert_eq!(without.status, StatusCode::CREATED, "{}", without.body);
        assert_eq!(without.body["confirmation_email"], false);
    }
}

/// Settings that would make every send fail count as "no email" up front:
/// the chef gets the not-configured message and the link pages say so.
#[sqlx::test(migrator = "MIGRATOR")]
async fn unusable_smtp_settings_count_as_no_email(pool: PgPool) {
    let partial = Config {
        email_host: "smtp.example.com".into(),
        email_user: "user".into(),
        email_pass: String::new(),
        email_capture: false,
        ..test_config()
    };
    let bad_from = Config {
        email_host: "smtp.example.com".into(),
        email_user: "apikey".into(),
        email_pass: "secret".into(),
        email_from: "apikey".into(),
        email_capture: false,
        ..test_config()
    };
    for (label, config) in [("partial", partial), ("bad from", bad_from)] {
        let (app, _) = app_with(pool.clone(), config);
        let (c, _, invoice_url) = chef_with_work(&pool, &app, &format!("Unusable {label}")).await;
        let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
        assert_eq!(
            res.status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{label}: {}",
            res.body
        );
        assert_eq!(res.body["code"], "EMAIL_NOT_CONFIGURED", "{label}");
        assert_eq!(c.get(&invoice_url).await.body["status"], "draft", "{label}");
        let link = post(
            "/api/auth/forgot-password",
            json!({ "email": "x@example.com" }),
        )
        .send(&app)
        .await;
        assert_eq!(link.body["email_available"], false, "{label}");
    }
}
