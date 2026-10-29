// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Auth routes, ported from server/test/auth.test.js plus compatibility checks
//! against hashes and cookies produced by the Node server's libraries.

mod common;

use std::time::Duration;

use axum::http::{StatusCode, header};
use chef_server::auth::{Account, sign_one_time};
use chef_server::config::SignupMode;
use chef_server::db::MIGRATOR;
use common::{JWT_SECRET, app, app_with, get, make_account, post, test_config};
use serde_json::json;
use sqlx::PgPool;

/// `bcryptjs.hashSync('correct-horse-battery', 12)` from server/node_modules.
const BCRYPTJS_HASH: &str = "$2a$12$8NhMFiCiP6HwxQFQ3Fk5Iuo3QZPGk8fJUnaI2uimrfp4YKwvefu4.";
/// node-jsonwebtoken: `jwt.sign({ accountId: 1, email: 'legacy@example.com' }, 'test-secret', { expiresIn: '36500d' })`
const NODE_SESSION: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJhY2NvdW50SWQiOjEsImVtYWlsIjoibGVnYWN5QGV4YW1wbGUuY29tIiwiaWF0IjoxNzkxOTk3ODkyLCJleHAiOjQ5NDU1OTc4OTJ9.SnyaGf1K00sdUZuF3VR4q-9YZpUOZK4JUK8HfbsEE00";
/// Same, but a pre-multi-tenant cookie with no accountId.
const NODE_LEGACY_SESSION: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJlbWFpbCI6ImxlZ2FjeUBleGFtcGxlLmNvbSIsImlhdCI6MTc5MTk5Nzg5MiwiZXhwIjo0OTQ1NTk3ODkyfQ.gpcjXnkYMhVGbUohIkEJyrL2GdslWP1NmTYf_sDjnhQ";

fn signup_body(email: &str) -> serde_json::Value {
    json!({ "email": email, "password": "correct-horse-battery", "business_name": "Sunday Suppers" })
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn health_is_public(pool: PgPool) {
    let res = get("/health").send(&app(pool)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, json!({ "ok": true }));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_creates_account_settings_and_cookie(pool: PgPool) {
    let app = app(pool.clone());
    let res = post("/api/auth/signup", signup_body("New.Chef@Example.com"))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["email"], "new.chef@example.com");
    assert_eq!(res.body["slug"], "sunday-suppers");
    assert_eq!(res.body["onboarding_completed"], false);
    let set_cookie = res.set_cookie().unwrap();
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    assert!(set_cookie.contains("Path=/"));

    let (name, prefix): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT business_name, invoice_prefix FROM settings WHERE account_id = $1")
            .bind(res.body["id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name.as_deref(), Some("Sunday Suppers"));
    assert_eq!(prefix.as_deref(), Some("INV"));

    // The cookie works on a protected route.
    let me = get("/api/auth/me")
        .cookie(res.auth_cookie().unwrap())
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["email"], "new.chef@example.com");

    // Duplicate email, different case.
    let dup = post("/api/auth/signup", signup_body("NEW.CHEF@example.com"))
        .send(&app)
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(dup.body["code"], "EMAIL_TAKEN");

    // Same business name gets the next free slug.
    let second = post("/api/auth/signup", signup_body("second@example.com"))
        .send(&app)
        .await;
    assert_eq!(second.body["slug"], "sunday-suppers-2");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_mode_first_allows_only_the_first_account(pool: PgPool) {
    let (app, _) = app_with(
        pool,
        chef_server::config::Config {
            signup_mode: SignupMode::First,
            ..test_config()
        },
    );
    let status = get("/api/auth/signup").send(&app).await;
    assert_eq!(status.status, StatusCode::OK);
    assert_eq!(status.body, json!({ "open": true }));

    let first = post("/api/auth/signup", signup_body("owner@example.com"))
        .send(&app)
        .await;
    assert_eq!(first.status, StatusCode::CREATED);

    assert_eq!(
        get("/api/auth/signup").send(&app).await.body,
        json!({ "open": false })
    );
    let second = post("/api/auth/signup", signup_body("second@example.com"))
        .send(&app)
        .await;
    assert_eq!(second.status, StatusCode::FORBIDDEN);
    assert_eq!(second.body["code"], "SIGNUP_CLOSED");
    // The first account still signs in.
    let login = post(
        "/api/auth/login",
        json!({ "email": "owner@example.com", "password": "correct-horse-battery" }),
    )
    .send(&app)
    .await;
    assert_eq!(login.status, StatusCode::OK);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_mode_closed_refuses_every_signup(pool: PgPool) {
    let (app, _) = app_with(
        pool,
        chef_server::config::Config {
            signup_mode: SignupMode::Closed,
            ..test_config()
        },
    );
    assert_eq!(
        get("/api/auth/signup").send(&app).await.body,
        json!({ "open": false })
    );
    let res = post("/api/auth/signup", signup_body("new@example.com"))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FORBIDDEN);
    assert_eq!(res.body["code"], "SIGNUP_CLOSED");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_status_is_open_by_default(pool: PgPool) {
    let res = get("/api/auth/signup").send(&app(pool)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, json!({ "open": true }));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_validates_input(pool: PgPool) {
    let app = app(pool);
    let short = post(
        "/api/auth/signup",
        json!({ "email": "short@example.com", "password": "abc" }),
    )
    .send(&app)
    .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    assert_eq!(short.body["code"], "VALIDATION_ERROR");
    assert_eq!(
        short.body["error"],
        "Password must be at least 10 characters."
    );

    let bad_email = post(
        "/api/auth/signup",
        json!({ "email": "nope", "password": "long-enough-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(bad_email.status, StatusCode::BAD_REQUEST);

    let long = post(
        "/api/auth/signup",
        json!({ "email": "l@example.com", "password": "x".repeat(201) }),
    )
    .send(&app)
    .await;
    assert_eq!(long.body["error"], "Password is too long.");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reserved_business_names_get_a_suffix(pool: PgPool) {
    let res = post(
        "/api/auth/signup",
        json!({ "email": "a@example.com", "password": "long-enough-password", "business_name": "Admin" }),
    )
    .send(&app(pool))
    .await;
    assert_eq!(res.body["slug"], "admin-kitchen");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login(pool: PgPool) {
    let app = app(pool);
    post("/api/auth/signup", signup_body("chef@example.com"))
        .send(&app)
        .await;

    let ok = post(
        "/api/auth/login",
        json!({ "email": "Chef@Example.com ", "password": "correct-horse-battery" }),
    )
    .send(&app)
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert!(ok.auth_cookie().is_some());

    let wrong = post(
        "/api/auth/login",
        json!({ "email": "chef@example.com", "password": "wrong-password-here" }),
    )
    .send(&app)
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.body["code"], "INVALID_CREDENTIALS");

    let ghost = post(
        "/api/auth/login",
        json!({ "email": "ghost@example.com", "password": "whatever-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(ghost.status, StatusCode::UNAUTHORIZED);
    assert_eq!(ghost.body["error"], "Incorrect email or password.");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_accepts_existing_bcryptjs_hashes(pool: PgPool) {
    sqlx::query(
        "INSERT INTO accounts (email, password_hash, slug) VALUES ('old@example.com', $1, 'old')",
    )
    .bind(BCRYPTJS_HASH)
    .execute(&pool)
    .await
    .unwrap();
    let res = post(
        "/api/auth/login",
        json!({ "email": "old@example.com", "password": "correct-horse-battery" }),
    )
    .send(&app(pool))
    .await;
    assert_eq!(res.status, StatusCode::OK);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn passwordless_account_gets_a_hint(pool: PgPool) {
    make_account(&pool, "No Password").await;
    let res = post(
        "/api/auth/login",
        json!({ "email": "no.password@example.com", "password": "anything-long" }),
    )
    .send(&app(pool))
    .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert!(
        res.body["error"]
            .as_str()
            .unwrap()
            .contains("doesn't have a password yet")
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn node_issued_session_cookies_are_accepted(pool: PgPool) {
    sqlx::query(
        "INSERT INTO accounts (id, email, slug) VALUES (1, 'legacy@example.com', 'legacy')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = app(pool);
    let me = get("/api/auth/me")
        .cookie(format!("authToken={NODE_SESSION}"))
        .send(&app)
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["slug"], "legacy");

    // A pre-accounts cookie (no accountId) is rejected and cleared.
    let legacy = get("/api/auth/me")
        .cookie(format!("authToken={NODE_LEGACY_SESSION}"))
        .send(&app)
        .await;
    assert_eq!(legacy.status, StatusCode::UNAUTHORIZED);
    assert_eq!(legacy.body["code"], "INVALID_TOKEN");
    assert!(legacy.set_cookie().unwrap().starts_with("authToken=;"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn protected_routes_need_a_session(pool: PgPool) {
    let app = app(pool.clone());
    let res = get("/api/auth/me").send(&app).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        res.body,
        json!({ "error": "Authentication required", "code": "UNAUTHORIZED" })
    );

    // Unknown /api paths are still protected, then a JSON 404 once signed in.
    assert_eq!(
        get("/api/no-such-route").send(&app).await.status,
        StatusCode::UNAUTHORIZED
    );
    let (_, cookie) = make_account(&pool, "Kitchen").await;
    let missing = get("/api/no-such-route").cookie(cookie).send(&app).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["code"], "NOT_FOUND");

    // A cookie for a deleted account.
    let ghost = Account {
        id: 999_999,
        email: "ghost@example.com".into(),
    };
    let token = chef_server::auth::sign_session(&ghost, JWT_SECRET);
    let res = get("/api/auth/me")
        .cookie(format!("authToken={token}"))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.body["error"], "Account not found");
    // Refused by the middleware on every route, not just /me.
    let res = get("/api/clients")
        .cookie(format!("authToken={token}"))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.body["error"], "Account not found");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn forgot_password_and_request_link_respond_uniformly(pool: PgPool) {
    let app = app(pool.clone());
    make_account(&pool, "Real Kitchen").await;
    for path in ["/api/auth/forgot-password", "/api/auth/request-link"] {
        for email in ["ghost@example.com", "real.kitchen@example.com"] {
            let res = post(path, json!({ "email": email })).send(&app).await;
            assert_eq!(res.status, StatusCode::OK);
            assert_eq!(res.body, json!({ "sent": true }), "{path} {email}");
        }
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reset_token_sets_a_new_password_exactly_once(pool: PgPool) {
    let app = app(pool.clone());
    let (id, _) = make_account(&pool, "Reset Kitchen").await;
    let acct = Account {
        id,
        email: "reset.kitchen@example.com".into(),
    };
    let token = sign_one_time(
        &acct,
        "password-reset",
        Duration::from_secs(1800),
        JWT_SECRET,
    );

    let res = post(
        "/api/auth/reset-password",
        json!({ "token": token, "password": "brand-new-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(res.status, StatusCode::OK);
    assert!(res.auth_cookie().is_some());

    let login = post(
        "/api/auth/login",
        json!({ "email": acct.email, "password": "brand-new-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(login.status, StatusCode::OK);

    let reuse = post(
        "/api/auth/reset-password",
        json!({ "token": token, "password": "another-new-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(reuse.status, StatusCode::BAD_REQUEST);
    assert_eq!(reuse.body["code"], "TOKEN_USED");

    // A magic-link token can't be used as a reset token.
    let magic = sign_one_time(&acct, "magic-login", Duration::from_secs(900), JWT_SECRET);
    let res = post(
        "/api/auth/reset-password",
        json!({ "token": magic, "password": "brand-new-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body["code"], "INVALID_TOKEN");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn password_reset_ends_other_sessions(pool: PgPool) {
    let app = app(pool.clone());
    let (id, old_cookie) = make_account(&pool, "Stolen Kitchen").await;
    assert_eq!(
        get("/api/auth/me")
            .cookie(&old_cookie)
            .send(&app)
            .await
            .status,
        StatusCode::OK
    );
    // Session tokens carry whole-second `iat`s; a cookie from the same
    // second as the reset is (deliberately) still accepted.
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let acct = Account {
        id,
        email: "stolen.kitchen@example.com".into(),
    };
    let token = sign_one_time(
        &acct,
        "password-reset",
        Duration::from_secs(1800),
        JWT_SECRET,
    );
    let reset = post(
        "/api/auth/reset-password",
        json!({ "token": token, "password": "brand-new-password" }),
    )
    .send(&app)
    .await;
    assert_eq!(reset.status, StatusCode::OK);

    // The cookie from before the reset no longer works anywhere...
    let me = get("/api/auth/me").cookie(&old_cookie).send(&app).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(me.body["code"], "SESSION_EXPIRED");
    let data = get("/api/clients").cookie(&old_cookie).send(&app).await;
    assert_eq!(data.status, StatusCode::UNAUTHORIZED);

    // ...while the one the reset issued does.
    let fresh = reset.auth_cookie().expect("reset signs the user in");
    let me = get("/api/auth/me").cookie(&fresh).send(&app).await;
    assert_eq!(me.status, StatusCode::OK);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn magic_link_verify_sets_cookie_and_is_single_use(pool: PgPool) {
    let app = app(pool.clone());
    let (id, _) = make_account(&pool, "Magic Kitchen").await;
    let acct = Account {
        id,
        email: "magic.kitchen@example.com".into(),
    };
    let token = sign_one_time(&acct, "magic-login", Duration::from_secs(900), JWT_SECRET);

    let res = get(format!("/api/auth/verify?token={token}"))
        .send(&app)
        .await;
    assert_eq!(res.status, StatusCode::FOUND);
    assert_eq!(res.headers[header::LOCATION], "/");
    assert!(res.auth_cookie().is_some());

    let again = get(format!("/api/auth/verify?token={token}"))
        .send(&app)
        .await;
    assert_eq!(
        again.headers[header::LOCATION],
        "/login?error=link_already_used"
    );

    let missing = get("/api/auth/verify").send(&app).await;
    assert_eq!(
        missing.headers[header::LOCATION],
        "/login?error=missing_token"
    );

    let reset = sign_one_time(
        &acct,
        "password-reset",
        Duration::from_secs(900),
        JWT_SECRET,
    );
    let wrong = get(format!("/api/auth/verify?token={reset}"))
        .send(&app)
        .await;
    assert_eq!(
        wrong.headers[header::LOCATION],
        "/login?error=invalid_token"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn logout_clears_the_cookie(pool: PgPool) {
    let res = post("/api/auth/logout", json!({})).send(&app(pool)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, json!({ "success": true }));
    assert!(res.set_cookie().unwrap().starts_with("authToken=;"));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn signup_is_rate_limited_per_ip(pool: PgPool) {
    let app = app(pool);
    for i in 0..5 {
        let res = post(
            "/api/auth/signup",
            json!({ "email": format!("bad{i}"), "password": "x" }),
        )
        .ip("192.0.2.9")
        .send(&app)
        .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
    }
    let blocked = post("/api/auth/signup", signup_body("late@example.com"))
        .ip("192.0.2.9")
        .send(&app)
        .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        blocked.body["error"],
        "Too many sign-ups from this network — please try again later."
    );
    // Another client is unaffected.
    let other = post("/api/auth/signup", signup_body("late@example.com"))
        .ip("192.0.2.10")
        .send(&app)
        .await;
    assert_eq!(other.status, StatusCode::CREATED);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn without_a_proxy_a_forged_forwarded_for_does_not_reset_the_limit(pool: PgPool) {
    // TRUST_PROXY_HOPS=0: clients connect directly, so the limit keys on the
    // socket address and a different X-Forwarded-For on each try changes nothing.
    let config = chef_server::config::Config {
        trust_proxy_hops: 0,
        ..test_config()
    };
    let (app, _) = app_with(pool, config);
    for i in 0..5 {
        let res = post(
            "/api/auth/signup",
            json!({ "email": format!("bad{i}"), "password": "x" }),
        )
        .ip(format!("203.0.113.{i}"))
        .peer("192.0.2.9")
        .send(&app)
        .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
    }
    let blocked = post("/api/auth/signup", signup_body("late@example.com"))
        .ip("203.0.113.99")
        .peer("192.0.2.9")
        .send(&app)
        .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    // A different socket address is a different client.
    let other = post("/api/auth/signup", signup_body("late@example.com"))
        .peer("192.0.2.10")
        .send(&app)
        .await;
    assert_eq!(other.status, StatusCode::CREATED);
}
