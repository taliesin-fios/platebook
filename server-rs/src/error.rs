// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! API error type. Every error response has the Node server's shape:
//! `{ "error": "<message>", "code": "<CODE>" }`.
//!
//! Unexpected failures (database, hashing, ...) go through [`err_msg`], the
//! port of `errMsg()` in `server/utils/helpers.js`: in production the client
//! only ever sees "Internal server error"; the real cause is logged.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Map, Value};

use crate::config::is_production;

/// Port of `errMsg(err)`: hides internal error details in production.
pub fn err_msg(err: &dyn std::fmt::Display) -> String {
    if is_production() {
        "Internal server error".to_string()
    } else {
        err.to_string()
    }
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    /// `None` for the Node routes that answer `{ error }` without a code.
    pub code: Option<&'static str>,
    pub message: String,
    /// Extra top-level fields some routes add (`field`, `invoice_id`, ...).
    pub extra: Map<String, Value>,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        AppError {
            status,
            code: Some(code),
            message: message.into(),
            extra: Map::new(),
        }
    }

    /// `{ "error": message }` with no `code`, as several Node routes send.
    pub fn bare(status: StatusCode, message: impl Into<String>) -> Self {
        AppError {
            status,
            code: None,
            message: message.into(),
            extra: Map::new(),
        }
    }

    /// Adds an extra top-level field to the error body.
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.extra.insert(key.to_string(), value.into());
        self
    }

    /// 500 `DB_ERROR`, the catch-all most Node data routes use.
    pub fn db(err: impl std::fmt::Display) -> Self {
        tracing::error!(error = %err, "database error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", err_msg(&err))
    }

    /// 500 with no `code` (`res.status(500).json({ error: errMsg(err) })`).
    pub fn internal_bare(err: impl std::fmt::Display) -> Self {
        tracing::error!(error = %err, "internal error");
        Self::bare(StatusCode::INTERNAL_SERVER_ERROR, err_msg(&err))
    }

    /// 404 with a route-specific message and `NOT_FOUND`.
    pub fn not_found_msg(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    /// Rewrites a generic 500 into the route group's error style (`DB_ERROR`
    /// or bare). Other errors pass through unchanged.
    pub fn restyle(self, style: ErrorStyle) -> Self {
        if self.status != StatusCode::INTERNAL_SERVER_ERROR || self.code != Some("SERVER_ERROR") {
            return self;
        }
        match style {
            ErrorStyle::Db => AppError {
                code: Some("DB_ERROR"),
                ..self
            },
            ErrorStyle::Bare => AppError { code: None, ..self },
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Not found")
    }

    /// 500 SERVER_ERROR with the message filtered through [`err_msg`].
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(error = %err, "internal error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "SERVER_ERROR",
            err_msg(&err),
        )
    }
}

/// How a route group reports unexpected 500s (see [`AppError::restyle`]).
#[derive(Debug, Clone, Copy)]
pub enum ErrorStyle {
    Db,
    Bare,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut body = Map::new();
        body.insert("error".into(), Value::String(self.message));
        if let Some(code) = self.code {
            body.insert("code".into(), Value::String(code.into()));
        }
        body.extend(self.extra);
        (self.status, Json(Value::Object(body))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::internal(err)
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// True for a Postgres unique-constraint violation (SQLSTATE 23505).
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

macro_rules! styled_error {
    ($name:ident, $style:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug)]
        pub struct $name(pub AppError);

        impl From<sqlx::Error> for $name {
            fn from(err: sqlx::Error) -> Self {
                $name(AppError::internal(err).restyle($style))
            }
        }

        impl From<AppError> for $name {
            fn from(err: AppError) -> Self {
                $name(err.restyle($style))
            }
        }

        impl IntoResponse for $name {
            fn into_response(self) -> Response {
                self.0.into_response()
            }
        }
    };
}

styled_error!(
    DbError,
    ErrorStyle::Db,
    "Error for route groups whose 500s are `{ error, code: 'DB_ERROR' }`."
);
styled_error!(
    BareError,
    ErrorStyle::Bare,
    "Error for route groups whose 500s are `{ error }` with no code."
);

pub type DbResult<T> = Result<T, DbError>;
pub type BareResult<T> = Result<T, BareError>;
