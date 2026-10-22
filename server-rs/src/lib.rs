// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Platebook API server (Axum + SQLx + Postgres), which replaced the old
//! Express server. See `server-rs/README.md`.

pub mod accounts;
pub mod anthropic;
pub mod api_types;
pub mod app;
pub mod auth;
pub mod backup;
pub mod config;
pub mod db;
pub mod email;
pub mod error;
pub mod intake_token;
pub mod js;
pub mod models;
pub mod money;
pub mod ownership;
pub mod pdf;
pub mod rate_limit;
pub mod recipe_import;
pub mod reminders;
pub mod routes;
pub mod safe_fetch;
pub mod security_headers;
pub mod seed;
pub mod serde_util;
pub mod state;
pub mod stripe;
