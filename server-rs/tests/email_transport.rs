// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Email through a real SMTP conversation (a local fake server): acceptance,
//! rejection, transient failure and retry, connection failure, timeout,
//! repeated clicks, and the sign-in/reset flows when the mail server fails.

mod common;

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use chef_server::db::MIGRATOR;
use common::smtp::{FakeSmtp, Mode};
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;

async fn chef_with_work(pool: &PgPool, app: &axum::Router, name: &str) -> (Chef, String, String) {
    let c = Chef::new(pool, app, name).await;
    let client = c
        .post(
            "/api/clients",
            json!({ "name": "Avery Household", "email": "avery@example.com" }),
        )
        .await;
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

fn assert_unsent_invoice(invoice: &Value) {
    assert_eq!(invoice["status"], "draft", "{invoice}");
    assert!(invoice["emailed_at"].is_null(), "{invoice}");
    assert!(invoice["email_failed_at"].is_string(), "{invoice}");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn accepted_mail_is_reported_as_accepted_by_the_mail_server(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::Accept).await;
    let (app, state) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, session_url, invoice_url) = chef_with_work(&pool, &app, "Accepting Kitchen").await;

    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["delivery"], "smtp");
    assert_eq!(res.body["sent_to"], "avery@example.com");
    let res = c.post(&format!("{session_url}/email-pdf"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["delivery"], "smtp");

    let mail = smtp.messages();
    assert_eq!(mail.len(), 2);
    for m in &mail {
        assert!(m.contains("To: avery@example.com"), "{m}");
        assert!(m.contains("application/pdf"), "PDF attached: {m}");
    }
    assert!(mail[0].contains("Subject: Your invoice"), "{}", mail[0]);
    assert!(
        state.outbox.messages().is_empty(),
        "SMTP mode doesn't capture"
    );

    let invoice = c.get(&invoice_url).await.body;
    assert_eq!(invoice["status"], "sent");
    assert!(invoice["emailed_at"].is_string() && invoice["email_failed_at"].is_null());
    assert!(c.get(&session_url).await.body["pdf_emailed_at"].is_string());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_rejected_recipient_fails_without_retry_and_changes_nothing(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::RejectRecipient).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, session_url, invoice_url) = chef_with_work(&pool, &app, "Rejected Kitchen").await;

    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::BAD_GATEWAY, "{}", res.body);
    assert_eq!(res.body["code"], "EMAIL_REJECTED");
    assert!(
        !res.text.contains("5.1.1"),
        "SMTP detail stays in the log: {}",
        res.text
    );
    assert_eq!(smtp.connections(), 1, "a permanent rejection isn't retried");
    assert_unsent_invoice(&c.get(&invoice_url).await.body);

    let res = c.post(&format!("{session_url}/email-pdf"), json!({})).await;
    assert_eq!(res.status, StatusCode::BAD_GATEWAY, "{}", res.body);
    let session = c.get(&session_url).await.body;
    assert!(session["pdf_emailed_at"].is_null() && session["pdf_email_failed_at"].is_string());
    assert!(smtp.messages().is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_transient_failure_is_retried_once(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::TransientThenAccept(1)).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Transient Kitchen").await;

    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(smtp.connections(), 2);
    assert_eq!(smtp.messages().len(), 1);
    assert_eq!(c.get(&invoice_url).await.body["status"], "sent");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_refused_connection_fails_truthfully(pool: PgPool) {
    // Bind then drop a listener: nothing is on that port.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let smtp = FakeSmtp::start(Mode::Accept).await;
    let config = chef_server::config::Config {
        email_port: port,
        ..smtp.config(test_config())
    };
    let (app, _) = app_with(pool.clone(), config);
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Refused Kitchen").await;
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::BAD_GATEWAY, "{}", res.body);
    assert_eq!(res.body["code"], "EMAIL_SEND_FAILED");
    assert_unsent_invoice(&c.get(&invoice_url).await.body);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_silent_mail_server_times_out_instead_of_hanging(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::Silent).await;
    let config = chef_server::config::Config {
        email_timeout: Duration::from_millis(500),
        ..smtp.config(test_config())
    };
    let (app, _) = app_with(pool.clone(), config);
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Silent Kitchen").await;
    let started = Instant::now();
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    // One bounded attempt: a timeout isn't retried (the server may have taken
    // the message and a retry could send it twice).
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(res.status, StatusCode::BAD_GATEWAY, "{}", res.body);
    assert_eq!(res.body["code"], "EMAIL_SEND_FAILED");
    assert_eq!(smtp.connections(), 1, "timeouts aren't retried");
    assert_unsent_invoice(&c.get(&invoice_url).await.body);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn retry_after_the_mail_server_recovers_clears_the_failure(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::RejectRecipient).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Recovering Kitchen").await;
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::BAD_GATEWAY);
    assert_unsent_invoice(&c.get(&invoice_url).await.body);

    smtp.set_mode(Mode::Accept);
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let invoice = c.get(&invoice_url).await.body;
    assert_eq!(invoice["status"], "sent");
    assert!(invoice["emailed_at"].is_string() && invoice["email_failed_at"].is_null());
    assert_eq!(smtp.messages().len(), 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn repeated_clicks_leave_one_consistent_state(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::Accept).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Clicky Kitchen").await;
    let url = format!("{invoice_url}/email");
    let results = c.concurrently(3, || post(url.clone(), json!({}))).await;
    let ok = results
        .iter()
        .filter(|r| r.status == StatusCode::OK)
        .count();
    assert!(
        results
            .iter()
            .all(|r| r.status == StatusCode::OK || r.status == StatusCode::TOO_MANY_REQUESTS),
        "{:?}",
        results.iter().map(|r| r.status).collect::<Vec<_>>()
    );
    // Each accepted request is one message: nothing claimed that wasn't sent.
    assert_eq!(smtp.messages().len(), ok);
    let invoice = c.get(&invoice_url).await.body;
    assert_eq!(invoice["status"], "sent");
    assert!(invoice["email_failed_at"].is_null());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn link_flows_hide_mail_server_failures_and_never_echo_the_link(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::RejectRecipient).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    make_account(&pool, "Link Fail Kitchen").await;
    for path in ["/api/auth/request-link", "/api/auth/forgot-password"] {
        let known = post(path, json!({ "email": "link.fail.kitchen@example.com" }))
            .send(&app)
            .await;
        let unknown = post(path, json!({ "email": "nobody@example.com" }))
            .send(&app)
            .await;
        assert_eq!(known.status, StatusCode::OK);
        assert_eq!(known.body, unknown.body, "{path}: same answer either way");
        assert_eq!(known.body, json!({ "sent": true, "email_available": true }));
    }

    // Let the background sends above (two known-account links) finish.
    for _ in 0..50 {
        if smtp.connections() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        smtp.connections(),
        2,
        "one attempt per real-account request"
    );
    assert!(smtp.messages().is_empty(), "all rejected");

    // With the server accepting, only the real account gets mail.
    smtp.set_mode(Mode::Accept);
    let before = smtp.connections();
    post(
        "/api/auth/forgot-password",
        json!({ "email": "nobody@example.com" }),
    )
    .send(&app)
    .await;
    // Mail goes out after the response: allow time for one to (wrongly) start.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(smtp.connections(), before, "no mail for an unknown address");
    post(
        "/api/auth/forgot-password",
        json!({ "email": "link.fail.kitchen@example.com" }),
    )
    .send(&app)
    .await;
    for _ in 0..50 {
        if !smtp.messages().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mail = smtp.messages();
    assert_eq!(mail.len(), 1);
    assert!(
        mail[0].contains("/reset?token="),
        "the link is in the email only"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn captured_mail_is_labelled_captured_not_sent(pool: PgPool) {
    let (app, state) = app_with(pool.clone(), test_config());
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Capture Kitchen").await;
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["delivery"], "captured");
    assert_eq!(state.outbox.messages().len(), 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_slow_mail_server_doesnt_slow_the_link_response_for_real_accounts(pool: PgPool) {
    // Timing must not reveal which addresses have accounts: the email is
    // sent after the response.
    let smtp = FakeSmtp::start(Mode::Silent).await;
    let config = chef_server::config::Config {
        email_timeout: Duration::from_secs(3),
        ..smtp.config(test_config())
    };
    let (app, _) = app_with(pool.clone(), config);
    make_account(&pool, "Slow Link Kitchen").await;
    for email in ["slow.link.kitchen@example.com", "nobody@example.com"] {
        let started = Instant::now();
        let res = post("/api/auth/forgot-password", json!({ "email": email }))
            .send(&app)
            .await;
        assert_eq!(res.status, StatusCode::OK);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "{email}: {:?}",
            started.elapsed()
        );
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_bad_business_email_doesnt_block_the_clients_mail(pool: PgPool) {
    let smtp = FakeSmtp::start(Mode::Accept).await;
    let (app, _) = app_with(pool.clone(), smtp.config(test_config()));
    let (c, _, invoice_url) = chef_with_work(&pool, &app, "Typo Kitchen").await;
    // Written directly, like old data from before Settings validated it.
    sqlx::query("UPDATE settings SET email = 'not an address' WHERE account_id = $1")
        .bind(c.id)
        .execute(&pool)
        .await
        .unwrap();
    let res = c.post(&format!("{invoice_url}/email"), json!({})).await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let mail = smtp.messages();
    assert_eq!(mail.len(), 1);
    assert!(
        !mail[0].contains("Reply-To"),
        "sent without the bad Reply-To"
    );
}
