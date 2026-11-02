// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::config::Config;
use crate::rate_limit::{AccountLimits, AuthLimits};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    /// A few connections of their own for the rate-limit counters, so a
    /// handler that already holds a tenant transaction can't wait forever
    /// on `pool` for its limit check when every connection is taken.
    pub limits_pool: PgPool,
    pub config: Arc<Config>,
    pub limits: Arc<AuthLimits>,
    pub account_limits: Arc<AccountLimits>,
    /// Outgoing HTTP (Anthropic, Stripe). Recipe URL import builds its own
    /// client with the SSRF-filtering resolver.
    pub http: reqwest::Client,
    /// Mail recorded instead of sent when SMTP isn't configured.
    pub outbox: Arc<crate::email::Outbox>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        let limits_pool = PgPoolOptions::new()
            .max_connections(3)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_lazy_with((*pool.connect_options()).clone());
        AppState {
            pool,
            limits_pool,
            config: Arc::new(config),
            limits: Arc::new(AuthLimits::default()),
            account_limits: Arc::new(AccountLimits::default()),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("HTTP client"),
            outbox: Arc::new(crate::email::Outbox::default()),
        }
    }
}
