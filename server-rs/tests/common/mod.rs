// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Shared helpers for integration tests. Each `#[sqlx::test]` gets its own
//! freshly migrated database on the server named by `DATABASE_URL`.

#![allow(dead_code)]

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use chef_server::config::Config;
use chef_server::state::AppState;
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub const JWT_SECRET: &str = "test-secret";

pub fn test_config() -> Config {
    Config::for_tests(JWT_SECRET)
}

pub fn app(pool: PgPool) -> Router {
    chef_server::app::build(AppState::new(pool, test_config()))
}

/// An app with a custom config, plus its state (for the email outbox).
pub fn app_with(pool: PgPool, config: Config) -> (Router, AppState) {
    let state = AppState::new(pool, config);
    (chef_server::app::build(state.clone()), state)
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
    /// The raw body (for non-JSON responses such as the CSV export).
    pub text: String,
    /// The raw bytes (for binary responses such as PDFs).
    pub bytes: Vec<u8>,
}

impl TestResponse {
    /// The `authToken=<jwt>` pair from Set-Cookie, ready for a Cookie header.
    pub fn auth_cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|c| c.starts_with("authToken="))
            .map(|c| c.split(';').next().unwrap().to_string())
    }

    pub fn set_cookie(&self) -> Option<String> {
        self.headers
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_string())
    }
}

pub struct Req {
    method: &'static str,
    uri: String,
    body: Option<Value>,
    cookie: Option<String>,
    ip: String,
    peer: Option<SocketAddr>,
}

pub fn get(uri: impl Into<String>) -> Req {
    Req::new("GET", uri)
}

pub fn post(uri: impl Into<String>, body: Value) -> Req {
    let mut r = Req::new("POST", uri);
    r.body = Some(body);
    r
}

pub fn put(uri: impl Into<String>, body: Value) -> Req {
    let mut r = Req::new("PUT", uri);
    r.body = Some(body);
    r
}

pub fn delete(uri: impl Into<String>) -> Req {
    Req::new("DELETE", uri)
}

impl Req {
    fn new(method: &'static str, uri: impl Into<String>) -> Self {
        Req {
            method,
            uri: uri.into(),
            body: None,
            cookie: None,
            ip: "10.0.0.1".to_string(),
            peer: None,
        }
    }

    pub fn cookie(mut self, cookie: impl Into<String>) -> Self {
        self.cookie = Some(cookie.into());
        self
    }

    /// Client IP as seen through Railway's proxy (X-Forwarded-For).
    pub fn ip(mut self, ip: impl Into<String>) -> Self {
        self.ip = ip.into();
        self
    }

    /// Socket peer address (as `ConnectInfo`), for servers with no proxy.
    pub fn peer(mut self, ip: &str) -> Self {
        self.peer = Some(SocketAddr::new(ip.parse().unwrap(), 51000));
        self
    }

    pub async fn send(self, app: &Router) -> TestResponse {
        let mut builder = Request::builder()
            .method(self.method)
            .uri(&self.uri)
            .header("x-forwarded-for", &self.ip);
        if let Some(addr) = self.peer {
            builder = builder.extension(ConnectInfo(addr));
        }
        if let Some(c) = &self.cookie {
            builder = builder.header(header::COOKIE, c);
        }
        let body = match &self.body {
            Some(v) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(serde_json::to_vec(v).unwrap())
            }
            None => Body::empty(),
        };
        let res = app
            .clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        TestResponse {
            status,
            headers,
            body,
            text: String::from_utf8_lossy(&bytes).into_owned(),
            bytes: bytes.to_vec(),
        }
    }
}

/// Creates an account (+ settings) directly, like `makeAccount()` in the
/// Node test helpers. Returns (id, cookie header value).
pub async fn make_account(pool: &PgPool, business_name: &str) -> (i64, String) {
    let email = format!(
        "{}@example.com",
        chef_server::accounts::slugify(business_name).replace('-', ".")
    );
    let mut tx = pool.begin().await.unwrap();
    let account = chef_server::accounts::create_account(&mut tx, &email, None, business_name)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let token = chef_server::auth::sign_session(
        &chef_server::auth::Account {
            id: account.id,
            email,
        },
        JWT_SECRET,
    );
    (account.id, format!("authToken={token}"))
}

/// A signed-in account and the app, for terse route tests.
pub struct Chef {
    pub id: i64,
    pub cookie: String,
    pub app: Router,
}

impl Chef {
    pub async fn new(pool: &PgPool, app: &Router, business_name: &str) -> Chef {
        let (id, cookie) = make_account(pool, business_name).await;
        Chef {
            id,
            cookie,
            app: app.clone(),
        }
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        get(uri).cookie(&self.cookie).send(&self.app).await
    }

    pub async fn post(&self, uri: &str, body: Value) -> TestResponse {
        post(uri, body).cookie(&self.cookie).send(&self.app).await
    }

    pub async fn put(&self, uri: &str, body: Value) -> TestResponse {
        put(uri, body).cookie(&self.cookie).send(&self.app).await
    }

    pub async fn delete(&self, uri: &str) -> TestResponse {
        delete(uri).cookie(&self.cookie).send(&self.app).await
    }
}
