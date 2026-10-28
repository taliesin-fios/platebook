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

/// The client IP that rate limits key on, like Express's `trust proxy = N`.
///
/// Each of the `trusted_hops` reverse proxies in front of the server appends
/// the address it saw to `X-Forwarded-For`, so the client is the entry
/// `trusted_hops` places from the right (the left-most one if the header is
/// shorter). Entries further left were written by the client and are never
/// trusted. With 0 hops (clients connect directly) the header is ignored:
/// anyone could put any address in it and get a fresh rate-limit bucket.
/// Falls back to the socket peer address when there's no usable header.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<&ConnectInfo<SocketAddr>>,
    trusted_hops: u8,
) -> String {
    let peer_ip = || peer.map(|ConnectInfo(addr)| addr.ip().to_string());
    if trusted_hops == 0 {
        return peer_ip().unwrap_or_else(|| "unknown".to_string());
    }
    // All X-Forwarded-For headers, in order, as one list.
    let forwarded: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
        .collect();
    let from_right = usize::from(trusted_hops) - 1;
    forwarded
        .iter()
        .rev()
        .nth(from_right)
        .or_else(|| forwarded.first())
        .map(|ip| ip.to_string())
        .or_else(peer_ip)
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extractor: the caller's IP per [`client_ip`] and `TRUST_PROXY_HOPS`.
pub struct ClientIp(pub String);

impl FromRequestParts<crate::state::AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::state::AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>();
        Ok(ClientIp(client_ip(
            &parts.headers,
            peer,
            state.config.trust_proxy_hops,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xff(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", v.parse().unwrap());
        h
    }

    fn peer(ip: &str) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 51000))
    }

    #[test]
    fn one_proxy_uses_the_rightmost_entry() {
        // The client sent "1.1.1.1"; the proxy appended the real 2.2.2.2.
        assert_eq!(client_ip(&xff("1.1.1.1, 2.2.2.2"), None, 1), "2.2.2.2");
        assert_eq!(client_ip(&HeaderMap::new(), None, 1), "unknown");
        let p = peer("10.0.0.9");
        assert_eq!(client_ip(&HeaderMap::new(), Some(&p), 1), "10.0.0.9");
    }

    #[test]
    fn no_proxy_ignores_the_header() {
        // Directly exposed: a spoofed header must not pick the bucket.
        let p = peer("198.51.100.7");
        assert_eq!(client_ip(&xff("203.0.113.1"), Some(&p), 0), "198.51.100.7");
        assert_eq!(client_ip(&xff("203.0.113.1"), None, 0), "unknown");
    }

    #[test]
    fn two_proxies_skip_the_inner_one() {
        // client spoof, real client (added by the outer proxy), outer proxy
        // (added by the inner proxy).
        let h = xff("6.6.6.6, 2.2.2.2, 172.70.1.1");
        assert_eq!(client_ip(&h, None, 2), "2.2.2.2");
        // Shorter than the hop count: the left-most entry.
        assert_eq!(client_ip(&xff("2.2.2.2"), None, 2), "2.2.2.2");
    }

    #[test]
    fn several_headers_are_one_list() {
        let mut h = HeaderMap::new();
        h.append("x-forwarded-for", "6.6.6.6".parse().unwrap());
        h.append("x-forwarded-for", "2.2.2.2".parse().unwrap());
        assert_eq!(client_ip(&h, None, 1), "2.2.2.2");
    }
}
