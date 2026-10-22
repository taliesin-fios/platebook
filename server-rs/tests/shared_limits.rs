// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Single-use token ids and rate-limit counters live in Postgres
//! (migration 0003), so two server instances on one database share them.
//! "Two instances" here are two independently built apps (separate
//! `AppState`s) on the same pool.

mod common;

use std::time::Duration;

use axum::http::{StatusCode, header};
use chef_server::auth::{Account, consume_jti, prune_used_jtis, sign_one_time};
use chef_server::db::MIGRATOR;
use chef_server::rate_limit::{self, RateLimited, RateLimiter};
use common::{JWT_SECRET, app, get, make_account, post};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_jti_is_consumed_once(pool: PgPool) {
    let ttl = Duration::from_secs(60);
    assert!(consume_jti(&pool, "a", ttl).await.unwrap());
    assert!(!consume_jti(&pool, "a", ttl).await.unwrap());
    assert!(consume_jti(&pool, "b", ttl).await.unwrap());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_magic_link_used_on_one_instance_is_refused_on_another(pool: PgPool) {
    let one = app(pool.clone());
    let two = app(pool.clone());
    let (id, _) = make_account(&pool, "Twin Kitchen").await;
    let acct = Account {
        id,
        email: "twin.kitchen@example.com".into(),
    };
    let token = sign_one_time(&acct, "magic-login", Duration::from_secs(900), JWT_SECRET);

    let first = get(format!("/api/auth/verify?token={token}"))
        .send(&one)
        .await;
    assert_eq!(first.headers[header::LOCATION], "/");
    let second = get(format!("/api/auth/verify?token={token}"))
        .send(&two)
        .await;
    assert_eq!(
        second.headers[header::LOCATION],
        "/login?error=link_already_used"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_reset_token_used_on_one_instance_is_refused_on_another(pool: PgPool) {
    let one = app(pool.clone());
    let two = app(pool.clone());
    let (id, _) = make_account(&pool, "Reset Twins").await;
    let acct = Account {
        id,
        email: "reset.twins@example.com".into(),
    };
    let token = sign_one_time(
        &acct,
        "password-reset",
        Duration::from_secs(1800),
        JWT_SECRET,
    );
    let body = json!({ "token": token, "password": "brand-new-password" });

    let first = post("/api/auth/reset-password", body.clone())
        .send(&one)
        .await;
    assert_eq!(first.status, StatusCode::OK);
    let second = post("/api/auth/reset-password", body).send(&two).await;
    assert_eq!(second.status, StatusCode::BAD_REQUEST);
    assert_eq!(second.body["code"], "TOKEN_USED");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn login_attempts_count_across_instances(pool: PgPool) {
    let one = app(pool.clone());
    let two = app(pool);
    let wrong = json!({ "email": "nobody@example.com", "password": "wrong-password" });
    // The login limit is 10 per IP per 15 minutes, split over both instances.
    for i in 0..10 {
        let target = if i % 2 == 0 { &one } else { &two };
        let res = post("/api/auth/login", wrong.clone())
            .ip("198.51.100.7")
            .send(target)
            .await;
        assert_eq!(res.status, StatusCode::UNAUTHORIZED, "attempt {i}");
    }
    for target in [&one, &two] {
        let res = post("/api/auth/login", wrong.clone())
            .ip("198.51.100.7")
            .send(target)
            .await;
        assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            res.body["error"],
            "Too many login attempts — please wait 15 minutes and try again."
        );
    }
    // Another IP is unaffected.
    let other = post("/api/auth/login", wrong)
        .ip("198.51.100.8")
        .send(&one)
        .await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn limits_are_per_bucket_and_key_and_reset_after_the_window(pool: PgPool) {
    let a = RateLimiter::new("test.a", 2, "slow down");
    let b = RateLimiter::new("test.b", 2, "slow down");
    assert!(a.check(&pool, "k").await.is_ok());
    assert!(a.check(&pool, "k").await.is_ok());
    assert!(matches!(
        a.check(&pool, "k").await,
        Err(RateLimited::Limited("slow down"))
    ));
    // Same key, other limiter; other key, same limiter.
    assert!(b.check(&pool, "k").await.is_ok());
    assert!(a.check(&pool, "other").await.is_ok());

    // Once the 15-minute window has passed the count starts again.
    sqlx::query(
        "UPDATE rate_limit_hits SET window_start = now() - interval '16 minutes'
         WHERE bucket = 'test.a' AND key = 'k'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(a.check(&pool, "k").await.is_ok());
    assert!(a.check(&pool, "k").await.is_ok());
    assert!(a.check(&pool, "k").await.is_err());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn pruning_drops_only_expired_rows(pool: PgPool) {
    let limiter = RateLimiter::new("test.prune", 5, "slow down");
    limiter.check(&pool, "old").await.unwrap();
    limiter.check(&pool, "new").await.unwrap();
    sqlx::query(
        "UPDATE rate_limit_hits SET window_start = now() - interval '1 hour' WHERE key = 'old'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(rate_limit::prune(&pool).await.unwrap(), 1);
    let left: Vec<String> = sqlx::query_scalar("SELECT key FROM rate_limit_hits")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(left, vec!["new".to_string()]);

    consume_jti(&pool, "expired", Duration::from_secs(60))
        .await
        .unwrap();
    consume_jti(&pool, "live", Duration::from_secs(60))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE used_jtis SET expires_at = now() - interval '1 second' WHERE jti = 'expired'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(prune_used_jtis(&pool).await.unwrap(), 1);
    // A still-live token stays used.
    assert!(
        !consume_jti(&pool, "live", Duration::from_secs(60))
            .await
            .unwrap()
    );
}
