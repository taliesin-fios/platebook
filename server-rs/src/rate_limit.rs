// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Fixed-window rate limits, matching the `express-rate-limit` settings in
//! `server/routes/auth.js` (15-minute windows, keyed on the client IP; 429
//! with `{ "error": "<message>" }`), plus the per-account limits on the
//! expensive data routes, which key on the account instead of the IP
//! (CLAUDE.md "Multi-tenancy rules").
//!
//! Counters live in Postgres (`rate_limit_hits`, migration 0003), so every
//! server instance shares them: one atomic upsert per request.

use std::net::SocketAddr;
use std::time::Duration;

use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use sqlx::PgPool;

const WINDOW: Duration = Duration::from_secs(15 * 60);

pub struct RateLimiter {
    /// Names this limiter's rows in `rate_limit_hits`.
    bucket: &'static str,
    max: u32,
    message: &'static str,
}

impl RateLimiter {
    pub fn new(bucket: &'static str, max: u32, message: &'static str) -> Self {
        RateLimiter {
            bucket,
            max,
            message,
        }
    }

    /// Counts a request from `key`; `Err` (a 429 response) once over the
    /// limit. A window starts with the first request and lasts [`WINDOW`].
    pub async fn check(&self, pool: &PgPool, key: &str) -> Result<(), RateLimited> {
        let hits: i32 = sqlx::query_scalar(
            "INSERT INTO rate_limit_hits AS r (bucket, key, window_start, hits)
             VALUES ($1, $2, now(), 1)
             ON CONFLICT (bucket, key) DO UPDATE SET
               window_start = CASE WHEN r.window_start <= now() - make_interval(secs => $3::float8)
                                   THEN now() ELSE r.window_start END,
               hits         = CASE WHEN r.window_start <= now() - make_interval(secs => $3::float8)
                                   THEN 1 ELSE r.hits + 1 END
             RETURNING hits",
        )
        .bind(self.bucket)
        .bind(key)
        .bind(WINDOW.as_secs_f64())
        .fetch_one(pool)
        .await
        .map_err(RateLimited::Db)?;
        if hits as u32 > self.max {
            Err(RateLimited::Limited(self.message))
        } else {
            Ok(())
        }
    }
}

/// Deletes counters whose window has ended (run periodically from `main`).
pub async fn prune(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let done = sqlx::query(
        "DELETE FROM rate_limit_hits WHERE window_start <= now() - make_interval(secs => $1::float8)",
    )
    .bind(WINDOW.as_secs_f64())
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}

#[derive(Debug)]
pub enum RateLimited {
    /// Over the limit: the limiter's message.
    Limited(&'static str),
    /// The counter couldn't be updated (answers 500, like any DB error).
    Db(sqlx::Error),
}

impl RateLimited {
    /// The `{ error, code: 'RATE_LIMITED' }` body the per-account limiters send.
    pub fn with_code(self) -> crate::error::AppError {
        match self {
            RateLimited::Limited(message) => {
                crate::error::AppError::new(StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", message)
            }
            RateLimited::Db(err) => err.into(),
        }
    }
}

impl IntoResponse for RateLimited {
    fn into_response(self) -> Response {
        match self {
            RateLimited::Limited(message) => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": message })),
            )
                .into_response(),
            RateLimited::Db(err) => crate::error::AppError::from(err).into_response(),
        }
    }
}

pub struct AuthLimits {
    pub magic_link: RateLimiter,
    pub login: RateLimiter,
    pub signup: RateLimiter,
    pub reset: RateLimiter,
    /// Public POST /api/intake: every submission stores a row and sends two
    /// emails, one to an address the submitter chooses.
    pub intake: RateLimiter,
    /// Public POST /api/intake/:id/checkout: each call opens a Stripe
    /// Checkout session (the edit token is also required).
    pub checkout: RateLimiter,
}

impl Default for AuthLimits {
    fn default() -> Self {
        AuthLimits {
            magic_link: RateLimiter::new(
                "auth.magic_link",
                5,
                "Too many login attempts — please wait 15 minutes and try again.",
            ),
            login: RateLimiter::new(
                "auth.login",
                10,
                "Too many login attempts — please wait 15 minutes and try again.",
            ),
            signup: RateLimiter::new(
                "auth.signup",
                5,
                "Too many sign-ups from this network — please try again later.",
            ),
            reset: RateLimiter::new(
                "auth.reset",
                5,
                "Too many reset requests — please wait 15 minutes and try again.",
            ),
            intake: RateLimiter::new(
                "public.intake",
                20,
                "Too many booking requests from this network — please try again later.",
            ),
            checkout: RateLimiter::new(
                "public.checkout",
                20,
                "Too many payment attempts from this network — please try again later.",
            ),
        }
    }
}

/// Per-account limits on the expensive data routes (AI, import, export).
/// Keyed on `acct:{id}`, never the IP: the Anthropic key is platform-wide,
/// so limits follow the account (CLAUDE.md "Multi-tenancy rules").
pub struct AccountLimits {
    /// POST /api/ai/suggest-menu, POST /api/ai/test — 30 / 15 min.
    pub ai: RateLimiter,
    /// POST /api/recipes/import/{url,text} — 20 / 15 min.
    pub import: RateLimiter,
    /// GET /api/costs/export-csv — 10 / 15 min.
    pub csv_export: RateLimiter,
    /// GET .../pdf (meal plan, grocery, planner, invoice) — 60 / 15 min.
    pub pdf: RateLimiter,
    /// POST /api/sessions/:id/email-pdf, /api/invoices/:id/email — 30 / 15 min.
    pub email: RateLimiter,
}

impl Default for AccountLimits {
    fn default() -> Self {
        AccountLimits {
            ai: RateLimiter::new(
                "acct.ai",
                30,
                "Too many AI requests — please wait a few minutes.",
            ),
            import: RateLimiter::new(
                "acct.import",
                20,
                "Too many import requests — please wait a few minutes.",
            ),
            csv_export: RateLimiter::new(
                "acct.csv_export",
                10,
                "Too many export requests — please wait 15 minutes.",
            ),
            pdf: RateLimiter::new(
                "acct.pdf",
                60,
                "Too many PDF downloads — please wait a few minutes.",
            ),
            email: RateLimiter::new(
                "acct.email",
                30,
                "Too many emails sent — please wait a few minutes.",
            ),
        }
    }
}

impl AccountLimits {
    /// The limiter key for an account.
    pub fn key(account_id: i64) -> String {
        format!("acct:{account_id}")
    }
}

/// Client IP the way Express computes it with `trust proxy = 1` (Railway's
/// single reverse proxy): the right-most `X-Forwarded-For` entry, else the
/// socket peer address.
pub fn client_ip(headers: &HeaderMap, peer: Option<&ConnectInfo<SocketAddr>>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|ip| ip.trim().to_string())
        .filter(|ip| !ip.is_empty())
        .or_else(|| peer.map(|ConnectInfo(addr)| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extractor: the caller's IP per [`client_ip`].
pub struct ClientIp(pub String);

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>();
        Ok(ClientIp(client_ip(&parts.headers, peer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_rightmost_forwarded_for() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.1.1.1, 2.2.2.2".parse().unwrap());
        assert_eq!(client_ip(&h, None), "2.2.2.2");
        assert_eq!(client_ip(&HeaderMap::new(), None), "unknown");
    }
}
