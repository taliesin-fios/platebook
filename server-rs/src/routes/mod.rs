// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

pub mod ai;
pub mod auth;
pub mod clients;
pub mod costs;
pub mod grocery;
pub mod health;
pub mod intake;
pub mod invoices;
pub mod onboarding;
pub mod payments;
pub mod planner;
pub mod public;
pub mod recipes;
pub mod sessions;
pub mod settings;
pub mod templates;

use axum::body::Bytes;
use axum::extract::FromRequest;
use axum::extract::Request;
use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::js;

/// Request body as a JSON object, forgiving like `express.json()` + `req.body`:
/// a missing, empty or non-object body is an empty object, so handlers read
/// fields the way the Node routes do (`String(req.body.email || '')`).
pub struct JsonBody(pub Map<String, Value>);

impl<S: Send + Sync> FromRequest<S> for JsonBody {
    type Rejection = crate::error::AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| crate::error::AppError::validation(e.body_text()))?;
        let map = match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(map)) => map,
            _ => Map::new(),
        };
        Ok(JsonBody(map))
    }
}

impl JsonBody {
    /// `String(req.body[key] || '')`: falsy values become "", others are stringified.
    pub fn str(&self, key: &str) -> String {
        match self.0.get(key) {
            None | Some(Value::Null) | Some(Value::Bool(false)) => String::new(),
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => String::new(),
            Some(other) => other.to_string(),
        }
    }

    /// The raw value, `None` when the key is absent (`undefined`).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    /// `req.body[key] !== undefined` (a JSON body can't carry `undefined`,
    /// so present, even as `null`, means defined).
    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// `!!req.body[key]`.
    pub fn truthy(&self, key: &str) -> bool {
        self.0.get(key).is_some_and(js::truthy)
    }

    /// The value bound as text: `None` for absent/`null`, else `String(v)`.
    pub fn opt_text(&self, key: &str) -> Option<String> {
        match self.0.get(key) {
            None | Some(Value::Null) => None,
            Some(v) => Some(js::string(v)),
        }
    }

    /// `req.body[key] || null` for a TEXT column.
    pub fn text_or_none(&self, key: &str) -> Option<String> {
        self.0.get(key).filter(|v| js::truthy(v)).map(js::string)
    }

    /// `req.body[key] || ''`.
    pub fn text_or_empty(&self, key: &str) -> String {
        self.text_or_none(key).unwrap_or_default()
    }

    /// `req.body[key]?.trim() || null`.
    pub fn trimmed_or_none(&self, key: &str) -> Option<String> {
        self.opt_text(key)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// `req.body[key] || default` for an INTEGER column.
    pub fn i64_or(&self, key: &str, default: i64) -> i64 {
        self.0
            .get(key)
            .filter(|v| js::truthy(v))
            .and_then(|v| {
                let f = js::number(v);
                f.is_finite().then(|| f.round() as i64)
            })
            .unwrap_or(default)
    }

    /// A value bound into an INTEGER column as-is: `None` when absent, `null`,
    /// blank or not a number.
    pub fn opt_i64(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(v) => {
                let f = js::number(v);
                f.is_finite().then(|| f.round() as i64)
            }
        }
    }

    /// A value bound into a NUMERIC column as-is (`None` when absent/null/NaN).
    pub fn opt_decimal(&self, key: &str) -> Option<Decimal> {
        match self.0.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(v) => js::decimal(js::number(v)),
        }
    }

    /// `req.body[key] || default` for a NUMERIC column.
    pub fn decimal_or(&self, key: &str, default: Decimal) -> Decimal {
        self.0
            .get(key)
            .filter(|v| js::truthy(v))
            .and_then(|v| js::decimal(js::number(v)))
            .unwrap_or(default)
    }

    /// A boolean flag column: `None` when absent, else JS truthiness.
    pub fn opt_flag(&self, key: &str) -> Option<bool> {
        self.0.get(key).map(js::truthy)
    }
}

/// A route `:id` the way SQLite compared it: a non-numeric id matches no row,
/// so it is a 404 rather than a 400.
pub fn parse_id(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok()
}

/// A body id (`client_id`, `intake_id`, ...) coerced like SQLite did.
pub fn body_id(v: Option<&Value>) -> Option<i64> {
    v.and_then(js::integer)
}

/// Ids from a JSON array, coerced like `Number(x)`; non-integers dropped.
pub fn id_list(v: Option<&Value>) -> Option<Vec<i64>> {
    match v {
        Some(Value::Array(items)) => Some(items.iter().filter_map(js::integer).collect()),
        _ => None,
    }
}

/// A JSONB value for an array column: `JSON.stringify(v || [])`.
pub fn json_or_empty(v: Option<&Value>) -> Value {
    match v {
        Some(v) if js::truthy(v) => v.clone(),
        _ => Value::Array(vec![]),
    }
}
