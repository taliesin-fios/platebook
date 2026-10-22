// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/middleware/requireAuth.js` and `server/utils/authTokens.js`.
//!
//! The session is the same httpOnly `authToken` cookie holding an HS256 JWT
//! `{ accountId, email }` valid for 7 days, signed with the same
//! `JWT_SECRET`, so cookies issued by the Node server keep working here and
//! the other way round.

use std::time::Duration;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use cookie::{Cookie, SameSite};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::AppError;
use crate::state::AppState;

pub const COOKIE_NAME: &str = "authToken";
pub const SESSION_DAYS: i64 = 7;

/// The signed-in chef business: `req.account` in the Node server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
}

/// Extracting `Account` requires a valid session; protected routes never get
/// this far without one (the middleware already answered 401).
impl<S: Send + Sync> FromRequestParts<S> for Account {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, AppError> {
        parts
            .extensions
            .get::<Account>()
            .cloned()
            .ok_or_else(|| AppError::unauthorized("Authentication required"))
    }
}

/// The signed-in account if there is one, for public routes that behave
/// differently for a signed-in chef.
#[derive(Debug, Clone)]
pub struct MaybeAccount(pub Option<Account>);

impl<S: Send + Sync> FromRequestParts<S> for MaybeAccount {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(MaybeAccount(parts.extensions.get::<Account>().cloned()))
    }
}

// ── Tokens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct SessionClaims {
    #[serde(rename = "accountId", default, skip_serializing_if = "Option::is_none")]
    account_id: Option<i64>,
    #[serde(default)]
    email: String,
    iat: i64,
    exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OneTimeClaims {
    #[serde(rename = "accountId", default)]
    pub account_id: Option<i64>,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub jti: String,
    iat: i64,
    exp: i64,
}

/// Why a token was rejected; mirrors jsonwebtoken's
/// `TokenExpiredError` / `JsonWebTokenError` split in Node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    Expired,
    Invalid,
}

fn validation() -> Validation {
    let mut v = Validation::new(Algorithm::HS256);
    // node-jsonwebtoken has no clock tolerance by default.
    v.leeway = 0;
    v
}

fn decode<T: serde::de::DeserializeOwned>(token: &str, secret: &str) -> Result<T, TokenError> {
    jsonwebtoken::decode::<T>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation(),
    )
    .map(|data| data.claims)
    .map_err(|e| match e.kind() {
        ErrorKind::ExpiredSignature => TokenError::Expired,
        _ => TokenError::Invalid,
    })
}

fn encode<T: Serialize>(claims: &T, secret: &str) -> String {
    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("HS256 signing cannot fail")
}

pub fn sign_session(account: &Account, secret: &str) -> String {
    let iat = Utc::now().timestamp();
    encode(
        &SessionClaims {
            account_id: Some(account.id),
            email: account.email.clone(),
            iat,
            exp: iat + SESSION_DAYS * 24 * 60 * 60,
        },
        secret,
    )
}

/// Returns the account, or why the token is unusable. A pre-multi-tenant
/// cookie (no `accountId`) is invalid.
pub fn verify_session(token: &str, secret: &str) -> Result<Account, TokenError> {
    verify_session_issued(token, secret).map(|(account, _)| account)
}

/// [`verify_session`] plus the token's `iat` (unix seconds).
pub fn verify_session_issued(token: &str, secret: &str) -> Result<(Account, i64), TokenError> {
    let claims: SessionClaims = decode(token, secret)?;
    match claims.account_id {
        Some(id) => Ok((
            Account {
                id,
                email: claims.email,
            },
            claims.iat,
        )),
        None => Err(TokenError::Invalid),
    }
}

/// Why a session cookie was refused.
enum SessionError {
    Token(TokenError),
    /// Valid signature, but the account no longer exists.
    NoAccount,
    /// Issued before `accounts.sessions_valid_after` (a password reset).
    Revoked,
    Db(sqlx::Error),
}

/// Verifies the cookie, then checks it against the account row (the one
/// pre-tenant lookup every authenticated request makes).
async fn check_session(pool: &PgPool, token: &str, secret: &str) -> Result<Account, SessionError> {
    let (account, iat) = verify_session_issued(token, secret).map_err(SessionError::Token)?;
    let row: Option<Option<chrono::DateTime<Utc>>> =
        sqlx::query_scalar("SELECT sessions_valid_after FROM accounts WHERE id = $1")
            .bind(account.id)
            .fetch_optional(pool)
            .await
            .map_err(SessionError::Db)?;
    match row {
        None => Err(SessionError::NoAccount),
        Some(Some(cutoff)) if iat < cutoff.timestamp() => Err(SessionError::Revoked),
        Some(_) => Ok(account),
    }
}

/// The cutoff a password reset stores in `sessions_valid_after`: now,
/// truncated to whole seconds so a cookie signed right after it (`iat` is in
/// seconds) is still accepted.
pub fn revocation_cutoff() -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap_or_else(Utc::now)
}

/// Short-lived, single-use (via `jti`) token for magic links and resets.
pub fn sign_one_time(account: &Account, purpose: &str, ttl: Duration, secret: &str) -> String {
    let iat = Utc::now().timestamp();
    encode(
        &OneTimeClaims {
            account_id: Some(account.id),
            email: account.email.clone(),
            purpose: purpose.to_string(),
            jti: uuid::Uuid::new_v4().to_string(),
            iat,
            exp: iat + ttl.as_secs() as i64,
        },
        secret,
    )
}

pub fn verify_one_time(
    token: &str,
    purpose: &str,
    secret: &str,
) -> Result<OneTimeClaims, TokenError> {
    let claims: OneTimeClaims = decode(token, secret)?;
    if claims.purpose != purpose || claims.account_id.is_none() || claims.jti.is_empty() {
        return Err(TokenError::Invalid);
    }
    Ok(claims)
}

/// Marks a one-time token's `jti` as used. True the first time (the caller
/// may proceed), false if it was already used.
///
/// Stored in Postgres (`used_jtis`, migration 0003) so a link can't be used
/// twice by hitting two different instances. The row is kept for `ttl` (the
/// token lifetime); after that the JWT `exp` check rejects the token anyway.
pub async fn consume_jti(pool: &PgPool, jti: &str, ttl: Duration) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "INSERT INTO used_jtis (jti, expires_at)
         VALUES ($1, now() + make_interval(secs => $2::float8))
         ON CONFLICT (jti) DO NOTHING",
    )
    .bind(jti)
    .bind(ttl.as_secs_f64())
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Deletes used JTIs whose tokens have expired (run periodically from `main`).
pub async fn prune_used_jtis(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM used_jtis WHERE expires_at < now()")
        .execute(pool)
        .await?;
    Ok(done.rows_affected())
}

// ── Cookies ─────────────────────────────────────────────────────────────────

/// `Set-Cookie` for a fresh session, like `setSessionCookie()`.
pub fn session_cookie(account: &Account, secret: &str, production: bool) -> HeaderValue {
    let max_age = cookie::time::Duration::days(SESSION_DAYS);
    let cookie = Cookie::build((COOKIE_NAME, sign_session(account, secret)))
        .path("/")
        .http_only(true)
        .secure(production)
        .same_site(SameSite::Lax)
        .max_age(max_age)
        .expires(cookie::time::OffsetDateTime::now_utc() + max_age)
        .build();
    HeaderValue::from_str(&cookie.to_string()).expect("cookie is valid header text")
}

/// `Set-Cookie` that clears the session, like `res.clearCookie('authToken')`.
pub fn clear_cookie() -> HeaderValue {
    HeaderValue::from_static("authToken=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT")
}

fn read_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(Cookie::split_parse)
        .filter_map(Result::ok)
        .find(|c| c.name() == COOKIE_NAME)
        .map(|c| c.value().to_string())
}

// ── Middleware ──────────────────────────────────────────────────────────────

/// Port of `isPublicRoute(req)`: routes reachable without a session.
/// Keep in sync with the allowlist table in ARCHITECTURE.md.
pub fn is_public_route(method: &Method, path: &str) -> bool {
    // Static files and React app — always public
    if !path.starts_with("/api/") {
        return true;
    }

    if method == Method::POST
        && matches!(
            path,
            "/api/auth/signup"
                | "/api/auth/login"
                | "/api/auth/request-link"
                | "/api/auth/forgot-password"
                | "/api/auth/reset-password"
                | "/api/auth/logout"
        )
    {
        return true;
    }
    if method == Method::GET && matches!(path, "/api/auth/verify" | "/api/auth/signup") {
        return true;
    }

    // Public booking page: a chef's safe business profile + booking policy, by slug
    if method == Method::GET
        && let Some(slug) = path
            .strip_prefix("/api/public/")
            .and_then(|rest| rest.strip_suffix("/settings"))
        && !slug.is_empty()
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return true;
    }

    // Client-facing intake submission
    if method == Method::POST && path == "/api/intake" {
        return true;
    }

    // Client intake view / edit (token checked inside the intake routes)
    if (method == Method::GET || method == Method::PUT)
        && let Some(id) = path.strip_prefix("/api/intake/")
        && !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit())
    {
        return true;
    }

    // Stripe webhook
    if method == Method::POST && path == "/api/payments/webhook" {
        return true;
    }

    // SEC6: legacy public settings (account #1 only — pre-multi-tenant intake links)
    if method == Method::GET && path == "/api/settings/public" {
        return true;
    }

    false
}

/// Port of `requireAuth`: protects every `/api/*` route that isn't public.
/// Public routes still learn who's signed in, if anyone.
pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    // Static files and the React app never need the account.
    if !req.uri().path().starts_with("/api/") {
        return next.run(req).await;
    }
    let session = match read_cookie(req.headers()) {
        Some(t) => Some(check_session(&state.pool, &t, &state.config.jwt_secret).await),
        None => None,
    };

    if is_public_route(req.method(), req.uri().path()) {
        if let Some(Ok(account)) = session {
            req.extensions_mut().insert(account);
        }
        return next.run(req).await;
    }

    match session {
        Some(Ok(account)) => {
            req.extensions_mut().insert(account);
            next.run(req).await
        }
        None => AppError::unauthorized("Authentication required").into_response(),
        Some(Err(SessionError::Db(err))) => AppError::from(err).into_response(),
        Some(Err(SessionError::NoAccount)) => {
            let mut res = AppError::unauthorized("Account not found").into_response();
            res.headers_mut().append(header::SET_COOKIE, clear_cookie());
            res
        }
        Some(Err(err)) => {
            let code = match err {
                SessionError::Token(TokenError::Expired) | SessionError::Revoked => {
                    "SESSION_EXPIRED"
                }
                _ => "INVALID_TOKEN",
            };
            let mut res = AppError::new(
                axum::http::StatusCode::UNAUTHORIZED,
                code,
                "Session expired — please log in again",
            )
            .into_response();
            res.headers_mut().append(header::SET_COOKIE, clear_cookie());
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_route_allowlist_matches_node() {
        let get = Method::GET;
        let post = Method::POST;
        let put = Method::PUT;
        let delete = Method::DELETE;

        assert!(is_public_route(&get, "/"));
        assert!(is_public_route(&get, "/assets/index.js"));
        assert!(is_public_route(&post, "/api/auth/login"));
        assert!(is_public_route(&post, "/api/auth/logout"));
        assert!(!is_public_route(&get, "/api/auth/login"));
        assert!(is_public_route(&get, "/api/auth/verify"));
        assert!(is_public_route(&get, "/api/auth/signup"));
        assert!(!is_public_route(&put, "/api/auth/signup"));
        assert!(!is_public_route(&get, "/api/auth/me"));
        assert!(is_public_route(
            &get,
            "/api/public/sunday-suppers-2/settings"
        ));
        assert!(!is_public_route(&get, "/api/public/Sunday/settings"));
        assert!(!is_public_route(&get, "/api/public//settings"));
        assert!(is_public_route(&post, "/api/intake"));
        assert!(is_public_route(&get, "/api/intake/42"));
        assert!(is_public_route(&put, "/api/intake/42"));
        assert!(!is_public_route(&delete, "/api/intake/42"));
        assert!(!is_public_route(&get, "/api/intake/abc"));
        assert!(is_public_route(&post, "/api/payments/webhook"));
        assert!(is_public_route(&get, "/api/settings/public"));
        assert!(!is_public_route(&get, "/api/settings"));
        assert!(!is_public_route(&get, "/api/clients"));
    }

    #[test]
    fn session_round_trip_and_wrong_secret() {
        let acct = Account {
            id: 7,
            email: "a@b.co".into(),
        };
        let token = sign_session(&acct, "s3cret");
        assert_eq!(verify_session(&token, "s3cret"), Ok(acct));
        assert_eq!(verify_session(&token, "other"), Err(TokenError::Invalid));
        assert_eq!(
            verify_session("garbage", "s3cret"),
            Err(TokenError::Invalid)
        );
    }

    #[test]
    fn expired_and_legacy_sessions_are_rejected() {
        let now = Utc::now().timestamp();
        let expired = encode(
            &SessionClaims {
                account_id: Some(1),
                email: String::new(),
                iat: now - 100,
                exp: now - 10,
            },
            "k",
        );
        assert_eq!(verify_session(&expired, "k"), Err(TokenError::Expired));

        let legacy = encode(
            &SessionClaims {
                account_id: None,
                email: "chef@example.com".into(),
                iat: now,
                exp: now + 60,
            },
            "k",
        );
        assert_eq!(verify_session(&legacy, "k"), Err(TokenError::Invalid));
    }

    #[test]
    fn one_time_tokens_check_purpose() {
        let acct = Account {
            id: 3,
            email: "x@y.co".into(),
        };
        let t = sign_one_time(&acct, "magic-login", Duration::from_secs(900), "k");
        assert!(verify_one_time(&t, "magic-login", "k").is_ok());
        assert_eq!(
            verify_one_time(&t, "password-reset", "k").err(),
            Some(TokenError::Invalid)
        );
    }

    #[test]
    fn reads_the_auth_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("x=1; authToken=abc.def"),
        );
        assert_eq!(read_cookie(&h).as_deref(), Some("abc.def"));
    }
}
