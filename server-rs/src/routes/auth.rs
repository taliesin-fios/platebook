// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/routes/auth.js` (accounts: email + password, magic link).
//!
//! GET  /api/auth/signup          — { open }: may a new account be created (SIGNUP_MODE)
//! POST /api/auth/signup          — create an account (+ settings row), sign in
//! POST /api/auth/login           — email + password
//! POST /api/auth/request-link    — email a one-click magic login link
//! GET  /api/auth/verify          — validate magic link, set session cookie, redirect /
//! POST /api/auth/forgot-password — email a password-reset link
//! POST /api/auth/reset-password  — set a new password from a reset token, sign in
//! GET  /api/auth/me              — current account (protected)
//! POST /api/auth/logout          — clear session cookie

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use sqlx::PgConnection;
use ts_rs::TS;

use crate::accounts::{self, AccountRow};
use crate::auth::{self, Account, TokenError, clear_cookie, session_cookie};
use crate::config::SignupMode;
use crate::email;
use crate::error::{AppError, AppResult, is_unique_violation};
use crate::rate_limit::ClientIp;
use crate::routes::JsonBody;
use crate::state::AppState;

const MIN_PASSWORD: usize = 10;
const MAX_PASSWORD: usize = 200;
pub const BCRYPT_ROUNDS: u32 = 12;
const MAGIC_TTL: Duration = Duration::from_secs(15 * 60);
const RESET_TTL: Duration = Duration::from_secs(30 * 60);

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").expect("valid regex"));

// A real-looking hash so a login for an unknown email costs the same time
// as a wrong password (no account-enumeration timing signal).
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    bcrypt::hash("timing-equalizer-not-a-password", BCRYPT_ROUNDS).expect("bcrypt hash")
});

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signup", get(signup_status).post(signup))
        .route("/login", post(login))
        .route("/request-link", post(request_link))
        .route("/verify", get(verify))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

fn norm_email(v: &str) -> String {
    v.to_lowercase().trim().to_string()
}

/// GET /api/auth/me, and the body of login, signup and reset-password.
#[derive(Debug, Serialize, TS)]
#[ts(export, rename = "Chef")]
pub struct ChefAccount {
    #[ts(type = "number")]
    pub id: i64,
    pub email: String,
    /// Public booking-link slug (`/book/:slug`).
    pub slug: String,
    pub onboarding_completed: bool,
}

/// GET /api/auth/signup.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SignupStatus {
    /// Whether `SIGNUP_MODE` lets a new account be created now.
    pub open: bool,
}

/// POST /api/auth/request-link and /api/auth/forgot-password.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct LinkSentResponse {
    #[ts(type = "true")]
    pub sent: bool,
    /// Only in development, when email isn't configured and ALLOW_DEV_LINK=true.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dev_link: Option<String>,
}

fn account_json(a: &AccountRow) -> ChefAccount {
    ChefAccount {
        id: a.id,
        email: a.email.clone(),
        slug: a.slug.clone(),
        onboarding_completed: a.onboarding_completed_at.is_some(),
    }
}

/// A JSON response that also sets the session cookie.
fn signed_in(state: &AppState, status: StatusCode, account: &AccountRow) -> Response {
    let cookie = session_cookie(
        &Account {
            id: account.id,
            email: account.email.clone(),
        },
        &state.config.jwt_secret,
        state.config.production,
    );
    let mut res = (status, Json(account_json(account))).into_response();
    res.headers_mut().append(header::SET_COOKIE, cookie);
    res
}

/// Express `res.redirect(path)`: 302 Found.
fn redirect(location: &str) -> Response {
    let mut res = StatusCode::FOUND.into_response();
    res.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(location).expect("ASCII location"),
    );
    res
}

async fn hash_password(password: String) -> AppResult<String> {
    tokio::task::spawn_blocking(move || bcrypt::hash(password, BCRYPT_ROUNDS))
        .await
        .map_err(AppError::internal)?
        .map_err(AppError::internal)
}

/// Verifies against `$2a$`/`$2b$` hashes from bcryptjs as well as our own.
async fn verify_password(password: String, hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || {
        let hash = hash.as_deref().unwrap_or(DUMMY_HASH.as_str());
        bcrypt::verify(password, hash).unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

async fn touch_login(state: &AppState, id: i64) -> AppResult<()> {
    sqlx::query!(
        "UPDATE accounts SET last_login_at = now() WHERE id = $1",
        id
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

fn email_taken() -> AppError {
    AppError::new(
        StatusCode::CONFLICT,
        "EMAIL_TAKEN",
        "An account with that email already exists. Try signing in.",
    )
}

// ── GET /api/auth/signup ─────────────────────────────────────────────────────
/// `{ open }`: whether `SIGNUP_MODE` lets a new account be created now, so the
/// client can hide its sign-up links.
async fn signup_status(State(state): State<AppState>) -> AppResult<Response> {
    let mut conn = state.pool.acquire().await?;
    let open = signup_open(state.config.signup_mode, &mut conn).await?;
    Ok(Json(SignupStatus { open }).into_response())
}

/// `first` allows sign-up only while the server has no account at all.
async fn signup_open(mode: SignupMode, conn: &mut PgConnection) -> Result<bool, sqlx::Error> {
    Ok(match mode {
        SignupMode::Open => true,
        SignupMode::Closed => false,
        SignupMode::First => {
            !sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM accounts)")
                .fetch_one(conn)
                .await?
        }
    })
}

fn signup_closed() -> AppError {
    AppError::new(
        StatusCode::FORBIDDEN,
        "SIGNUP_CLOSED",
        "Sign-up is closed on this server. Ask its owner for an account.",
    )
}

// ── POST /api/auth/signup ────────────────────────────────────────────────────
async fn signup(State(state): State<AppState>, ClientIp(ip): ClientIp, body: JsonBody) -> Response {
    if let Err(limited) = state.limits.signup.check(&state.pool, &ip).await {
        return limited.into_response();
    }
    signup_inner(&state, body)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn signup_inner(state: &AppState, body: JsonBody) -> AppResult<Response> {
    let email = norm_email(&body.str("email"));
    let password = body.str("password");
    let business_name: String = body.str("business_name").trim().chars().take(120).collect();

    if !EMAIL_RE.is_match(&email) || email.len() > 254 {
        return Err(AppError::validation("Please enter a valid email address."));
    }
    // JS `.length` counts UTF-16 units; char count is equal for the BMP.
    let len = password.encode_utf16().count();
    if len < MIN_PASSWORD {
        return Err(AppError::validation(format!(
            "Password must be at least {MIN_PASSWORD} characters."
        )));
    }
    if len > MAX_PASSWORD {
        return Err(AppError::validation("Password is too long."));
    }

    let mode = state.config.signup_mode;
    let mut conn = state.pool.acquire().await?;
    if !signup_open(mode, &mut conn).await? {
        return Err(signup_closed());
    }
    if accounts::find_by_email(&mut conn, &email).await?.is_some() {
        return Err(email_taken());
    }
    drop(conn);

    let hash = hash_password(password).await?;
    let mut tx = state.pool.begin().await?;
    if mode == SignupMode::First {
        // Two simultaneous first sign-ups: the lock makes the second one see
        // the first account and stop.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('platebook.signup.first'))")
            .execute(&mut *tx)
            .await?;
        if !signup_open(mode, &mut tx).await? {
            return Err(signup_closed());
        }
    }
    let account = match accounts::create_account(&mut tx, &email, Some(&hash), &business_name).await
    {
        Ok(a) => a,
        Err(err) if is_unique_violation(&err) && is_email_conflict(&err) => {
            return Err(email_taken());
        }
        Err(err) => return Err(err.into()),
    };
    tx.commit().await?;
    touch_login(state, account.id).await?;
    Ok(signed_in(state, StatusCode::CREATED, &account))
}

fn is_email_conflict(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.constraint() == Some("accounts_email_lower_key"))
}

// ── POST /api/auth/login ─────────────────────────────────────────────────────
async fn login(State(state): State<AppState>, ClientIp(ip): ClientIp, body: JsonBody) -> Response {
    if let Err(limited) = state.limits.login.check(&state.pool, &ip).await {
        return limited.into_response();
    }
    login_inner(&state, body)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn login_inner(state: &AppState, body: JsonBody) -> AppResult<Response> {
    let email = norm_email(&body.str("email"));
    let password = body.str("password");
    let account = if email.is_empty() {
        None
    } else {
        let mut conn = state.pool.acquire().await?;
        accounts::find_by_email(&mut conn, &email).await?
    };

    let hash = account.as_ref().and_then(|a| a.password_hash.clone());
    let ok = verify_password(password, hash).await;
    let no_password = account.as_ref().is_some_and(|a| a.password_hash.is_none());
    let Some(account) = account.filter(|a| a.password_hash.is_some() && ok) else {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "INVALID_CREDENTIALS",
            if no_password {
                "This account doesn't have a password yet — use \"Email me a sign-in link\" or \"Forgot password\"."
            } else {
                "Incorrect email or password."
            },
        ));
    };

    touch_login(state, account.id).await?;
    Ok(signed_in(state, StatusCode::OK, &account))
}

// ── POST /api/auth/request-link ──────────────────────────────────────────────
async fn request_link(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: JsonBody,
) -> Response {
    if let Err(limited) = state.limits.magic_link.check(&state.pool, &ip).await {
        return limited.into_response();
    }
    send_link(&state, body, "magic-login", MAGIC_TTL)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

// ── POST /api/auth/forgot-password ───────────────────────────────────────────
async fn forgot_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: JsonBody,
) -> Response {
    if let Err(limited) = state.limits.reset.check(&state.pool, &ip).await {
        return limited.into_response();
    }
    send_link(&state, body, "password-reset", RESET_TTL)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

/// Shared body of request-link and forgot-password. Always responds the same
/// way so the response doesn't reveal whether the account exists.
async fn send_link(
    state: &AppState,
    body: JsonBody,
    purpose: &str,
    ttl: Duration,
) -> AppResult<Response> {
    let email = norm_email(&body.str("email"));
    let sent = || {
        Json(LinkSentResponse {
            sent: true,
            dev_link: None,
        })
        .into_response()
    };

    let account = if email.is_empty() {
        None
    } else {
        let mut conn = state.pool.acquire().await?;
        accounts::find_by_email(&mut conn, &email).await?
    };
    let Some(account) = account else {
        return Ok(sent());
    };

    let who = Account {
        id: account.id,
        email: account.email.clone(),
    };
    let token = auth::sign_one_time(&who, purpose, ttl, &state.config.jwt_secret);
    let client_url = &state.config.client_url;
    let (url, result) = if purpose == "magic-login" {
        let url = format!("{client_url}/api/auth/verify?token={token}");
        let r = email::send_magic_link(state, account.id, &account.email, &url).await;
        (url, r)
    } else {
        let url = format!("{client_url}/reset?token={token}");
        let r = email::send_password_reset(state, account.id, &account.email, &url).await;
        (url, r)
    };
    if let Err(err) = result {
        tracing::error!(error = %err, purpose, "[auth] sending link failed");
    }

    // SEC7: dev link only when explicitly opted in; never in production.
    if state.config.expose_dev_link() {
        return Ok(Json(LinkSentResponse {
            sent: true,
            dev_link: Some(url),
        })
        .into_response());
    }
    Ok(sent())
}

// ── GET /api/auth/verify?token= ──────────────────────────────────────────────
async fn verify(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(token) = q.get("token").filter(|t| !t.is_empty()) else {
        return redirect("/login?error=missing_token");
    };

    let claims = match auth::verify_one_time(token, "magic-login", &state.config.jwt_secret) {
        Ok(c) => c,
        Err(TokenError::Expired) => return redirect("/login?error=expired"),
        Err(TokenError::Invalid) => return redirect("/login?error=invalid_token"),
    };
    match auth::consume_jti(&state.pool, &claims.jti, MAGIC_TTL).await {
        Ok(true) => {}
        Ok(false) => return redirect("/login?error=link_already_used"),
        Err(_) => return redirect("/login?error=invalid_token"),
    }

    let result: AppResult<Option<AccountRow>> = async {
        let mut conn = state.pool.acquire().await?;
        Ok(accounts::get_account(&mut conn, claims.account_id.unwrap_or_default()).await?)
    }
    .await;
    let account = match result {
        Ok(Some(a)) => a,
        _ => return redirect("/login?error=invalid_token"),
    };
    if touch_login(&state, account.id).await.is_err() {
        return redirect("/login?error=invalid_token");
    }

    let mut res = redirect("/");
    res.headers_mut().append(
        header::SET_COOKIE,
        session_cookie(
            &Account {
                id: account.id,
                email: account.email,
            },
            &state.config.jwt_secret,
            state.config.production,
        ),
    );
    res
}

// ── POST /api/auth/reset-password ────────────────────────────────────────────
async fn reset_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: JsonBody,
) -> Response {
    if let Err(limited) = state.limits.reset.check(&state.pool, &ip).await {
        return limited.into_response();
    }
    reset_inner(&state, body)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn reset_inner(state: &AppState, body: JsonBody) -> AppResult<Response> {
    let password = body.str("password");
    let len = password.encode_utf16().count();
    if !(MIN_PASSWORD..=MAX_PASSWORD).contains(&len) {
        return Err(AppError::validation(format!(
            "Password must be at least {MIN_PASSWORD} characters."
        )));
    }

    let claims = match auth::verify_one_time(
        &body.str("token"),
        "password-reset",
        &state.config.jwt_secret,
    ) {
        Ok(c) => c,
        Err(TokenError::Expired) => {
            return Err(AppError::new(
                StatusCode::BAD_REQUEST,
                "TOKEN_EXPIRED",
                "That reset link has expired — request a new one.",
            ));
        }
        Err(TokenError::Invalid) => return Err(invalid_reset()),
    };
    if !auth::consume_jti(&state.pool, &claims.jti, RESET_TTL).await? {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "TOKEN_USED",
            "That reset link was already used — request a new one.",
        ));
    }

    let mut conn = state.pool.acquire().await?;
    let Some(account) =
        accounts::get_account(&mut conn, claims.account_id.unwrap_or_default()).await?
    else {
        return Err(invalid_reset());
    };
    drop(conn);

    let hash = hash_password(password).await?;
    // Ends every other session (auth::require_auth rejects older cookies);
    // the cookie set below is issued at or after the cutoff.
    let cutoff = auth::revocation_cutoff();
    sqlx::query!(
        "UPDATE accounts SET password_hash = $1, last_login_at = now(), sessions_valid_after = $2
         WHERE id = $3",
        hash,
        cutoff,
        account.id
    )
    .execute(&state.pool)
    .await?;
    Ok(signed_in(state, StatusCode::OK, &account))
}

fn invalid_reset() -> AppError {
    AppError::new(
        StatusCode::BAD_REQUEST,
        "INVALID_TOKEN",
        "That reset link is invalid.",
    )
}

// ── GET /api/auth/me ─────────────────────────────────────────────────────────
async fn me(State(state): State<AppState>, account: Account) -> AppResult<Response> {
    let mut conn = state.pool.acquire().await?;
    match accounts::get_account(&mut conn, account.id).await? {
        Some(a) => Ok(Json(account_json(&a)).into_response()),
        None => {
            let mut res = AppError::unauthorized("Account not found").into_response();
            res.headers_mut().append(header::SET_COOKIE, clear_cookie());
            Ok(res)
        }
    }
}

// ── POST /api/auth/logout ────────────────────────────────────────────────────
async fn logout() -> Response {
    let mut res = Json(json!({ "success": true })).into_response();
    res.headers_mut().append(header::SET_COOKIE, clear_cookie());
    res
}
