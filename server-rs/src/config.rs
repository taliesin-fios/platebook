// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Runtime configuration, read once from the environment (and `server-rs/.env`
//! via dotenvy). Variable names match the Node server so one Railway service
//! config can drive either.
//!
//! Secrets are never printed: `Config`'s `Debug` impl redacts them.

use std::fmt;
use std::sync::OnceLock;

const DEV_JWT_SECRET: &str = "dev-secret-change-me";
const DEV_INTAKE_SECRET: &str = "chef-app-intake-secret-change-me";

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub production: bool,
    pub jwt_secret: String,
    pub client_url: String,
    /// SMTP host. Empty means email is not configured (dev mode).
    pub email_host: String,
    /// Opt-in for returning magic/reset links in the JSON response in dev.
    pub allow_dev_link: bool,
    pub email_port: u16,
    pub email_user: String,
    pub email_pass: String,
    /// `EMAIL_FROM`, else `EMAIL_USER`, else a placeholder.
    pub email_from: String,
    /// HMAC secret for client intake edit links (`INTAKE_SECRET`).
    pub intake_secret: String,
    /// `PAYMENTS_MODE`: disabled | manual | stripe | paypal | square.
    pub payments_mode: PaymentsMode,
    pub stripe_secret_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    /// Stripe API base (overridable so tests can point at a fake).
    pub stripe_api_base: String,
    /// `BOOKING_FEE_CENTS`, used only when an account has no deposit setting.
    pub booking_fee_cents: i64,
    pub anthropic_api_key: Option<String>,
    /// `ANTHROPIC_MODEL` override; each route has its own default otherwise.
    pub anthropic_model: Option<String>,
    /// Anthropic API base (overridable so tests can point at a fake).
    pub anthropic_api_base: String,
    /// Directory with the built React app, served when set (production).
    pub client_dist: Option<String>,
    /// Optional pg_dump backups (Railway Postgres has its own; off by default).
    pub backup: BackupConfig,
    /// `SIGNUP_MODE`: who may create an account.
    pub signup_mode: SignupMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentsMode {
    Disabled,
    Manual,
    Stripe,
    Paypal,
    Square,
}

impl PaymentsMode {
    pub fn parse(v: Option<&str>) -> Self {
        match v {
            Some("disabled") => Self::Disabled,
            Some("stripe") => Self::Stripe,
            Some("paypal") => Self::Paypal,
            Some("square") => Self::Square,
            _ => Self::Manual,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Manual => "manual",
            Self::Stripe => "stripe",
            Self::Paypal => "paypal",
            Self::Square => "square",
        }
    }
}

/// `SIGNUP_MODE`: `open` (default), `first` (only while the server has no
/// account, for a single chef on their own server) or `closed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupMode {
    Open,
    First,
    Closed,
}

impl SignupMode {
    pub fn parse(v: Option<&str>) -> Result<Self, ConfigError> {
        match v {
            None | Some("open") => Ok(Self::Open),
            Some("first") => Ok(Self::First),
            Some("closed") => Ok(Self::Closed),
            Some(_) => Err(ConfigError::Invalid("SIGNUP_MODE")),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BackupConfig {
    /// `AUTO_BACKUP=true` turns the daily pg_dump on.
    pub enabled: bool,
    /// `BACKUP_DIR`: where dumps go (a mounted volume).
    pub dir: String,
    /// `AUTO_BACKUP_HOUR` (0-23, UTC), default 3.
    pub hour: u32,
    /// `PG_DUMP_PATH`, default `pg_dump` on the PATH.
    pub pg_dump: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} must be set")]
    Missing(&'static str),
    #[error("{0} is not a valid value")]
    Invalid(&'static str),
    #[error("JWT_SECRET must be set in production — refusing to use dev default")]
    DevSecretInProduction,
    #[error("INTAKE_SECRET must be set in production — refusing to use dev default")]
    DevIntakeSecretInProduction,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    /// Loads `.env` (if present) and reads the environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();
        Self::from_vars()
    }

    /// Reads the environment without touching `.env` (used by tests).
    pub fn from_vars() -> Result<Self, ConfigError> {
        let production = var("NODE_ENV").as_deref() == Some("production");
        let jwt_secret = var("JWT_SECRET")
            .or_else(|| var("INTAKE_SECRET"))
            .unwrap_or_else(|| DEV_JWT_SECRET.to_string());
        if production && jwt_secret == DEV_JWT_SECRET {
            return Err(ConfigError::DevSecretInProduction);
        }
        // Intake edit links are signed with it: the public default would let
        // anyone forge one.
        let intake_secret = var("INTAKE_SECRET").unwrap_or_else(|| DEV_INTAKE_SECRET.to_string());
        if production && intake_secret == DEV_INTAKE_SECRET {
            return Err(ConfigError::DevIntakeSecretInProduction);
        }
        let port = match var("PORT") {
            Some(p) => p.parse().map_err(|_| ConfigError::Invalid("PORT"))?,
            None => 3002,
        };
        Ok(Config {
            database_url: var("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?,
            port,
            production,
            jwt_secret,
            client_url: var("CLIENT_URL").unwrap_or_else(|| "http://localhost:5174".to_string()),
            email_host: var("EMAIL_HOST").unwrap_or_default(),
            allow_dev_link: var("ALLOW_DEV_LINK").as_deref() == Some("true"),
            email_port: match var("EMAIL_PORT") {
                Some(p) => p.parse().map_err(|_| ConfigError::Invalid("EMAIL_PORT"))?,
                None => 587,
            },
            email_user: var("EMAIL_USER").unwrap_or_default(),
            email_pass: var("EMAIL_PASS").unwrap_or_default(),
            email_from: var("EMAIL_FROM")
                .or_else(|| var("EMAIL_USER"))
                .unwrap_or_else(|| "Platebook <noreply@example.com>".to_string()),
            intake_secret,
            payments_mode: PaymentsMode::parse(var("PAYMENTS_MODE").as_deref()),
            stripe_secret_key: var("STRIPE_SECRET_KEY"),
            stripe_webhook_secret: var("STRIPE_WEBHOOK_SECRET"),
            stripe_api_base: var("STRIPE_API_BASE")
                .unwrap_or_else(|| "https://api.stripe.com".to_string()),
            booking_fee_cents: var("BOOKING_FEE_CENTS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            anthropic_api_key: var("ANTHROPIC_API_KEY"),
            anthropic_model: var("ANTHROPIC_MODEL"),
            anthropic_api_base: var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            client_dist: var("CLIENT_DIST")
                .or_else(|| production.then(|| "../client/dist".to_string())),
            backup: BackupConfig {
                enabled: var("AUTO_BACKUP").as_deref() == Some("true"),
                dir: var("BACKUP_DIR").unwrap_or_else(|| "backups".to_string()),
                hour: var("AUTO_BACKUP_HOUR")
                    .and_then(|v| v.parse().ok())
                    .filter(|h| *h < 24)
                    .unwrap_or(3),
                pg_dump: var("PG_DUMP_PATH").unwrap_or_else(|| "pg_dump".to_string()),
            },
            signup_mode: SignupMode::parse(var("SIGNUP_MODE").as_deref())?,
        })
    }

    /// A config for tests: no email, manual payments, no AI key.
    pub fn for_tests(jwt_secret: &str) -> Self {
        Config {
            database_url: String::new(),
            port: 0,
            production: false,
            jwt_secret: jwt_secret.to_string(),
            client_url: "http://localhost:5174".to_string(),
            email_host: String::new(),
            allow_dev_link: false,
            email_port: 587,
            email_user: String::new(),
            email_pass: String::new(),
            email_from: "Platebook <noreply@example.com>".to_string(),
            intake_secret: "test-intake-secret".to_string(),
            payments_mode: PaymentsMode::Manual,
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            stripe_api_base: "https://api.stripe.com".to_string(),
            booking_fee_cents: 5000,
            anthropic_api_key: None,
            anthropic_model: None,
            anthropic_api_base: "https://api.anthropic.com".to_string(),
            client_dist: None,
            backup: BackupConfig::default(),
            signup_mode: SignupMode::Open,
        }
    }

    /// Email is sent only with host, user and password all set (Node's rule).
    pub fn email_configured(&self) -> bool {
        !self.email_host.is_empty() && !self.email_user.is_empty() && !self.email_pass.is_empty()
    }

    /// Dev-only: may a magic/reset link be echoed back in the response?
    /// Mirrors `!isProd() && !EMAIL_HOST && ALLOW_DEV_LINK === 'true'`.
    pub fn expose_dev_link(&self) -> bool {
        !self.production && self.email_host.is_empty() && self.allow_dev_link
    }

    /// Origins allowed by CORS (only needed for local dev; production is same-origin).
    pub fn allowed_origins(&self) -> Vec<String> {
        let client = var("CLIENT_URL");
        let mut origins: Vec<String> = if self.production {
            vec![]
        } else {
            vec![
                "http://localhost:5173".to_string(),
                "http://localhost:5174".to_string(),
            ]
        };
        origins.extend(client);
        origins
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &"<redacted>")
            .field("port", &self.port)
            .field("production", &self.production)
            .field("jwt_secret", &"<redacted>")
            .field("client_url", &self.client_url)
            .field("email_configured", &self.email_configured())
            .field("allow_dev_link", &self.allow_dev_link)
            .field("payments_mode", &self.payments_mode)
            .field("stripe_configured", &self.stripe_secret_key.is_some())
            .field("ai_configured", &self.anthropic_api_key.is_some())
            .field("client_dist", &self.client_dist)
            .field("auto_backup", &self.backup.enabled)
            .field("signup_mode", &self.signup_mode)
            .finish()
    }
}

static PRODUCTION: OnceLock<bool> = OnceLock::new();

/// Whether internal error details must be hidden. Set once at startup by
/// `app::build`; falls back to reading `NODE_ENV`.
pub fn is_production() -> bool {
    *PRODUCTION.get_or_init(|| var("NODE_ENV").as_deref() == Some("production"))
}

pub(crate) fn set_production(production: bool) {
    let _ = PRODUCTION.set(production);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signup_mode_parses_known_values_only() {
        assert_eq!(SignupMode::parse(None).unwrap(), SignupMode::Open);
        assert_eq!(SignupMode::parse(Some("open")).unwrap(), SignupMode::Open);
        assert_eq!(SignupMode::parse(Some("first")).unwrap(), SignupMode::First);
        assert_eq!(
            SignupMode::parse(Some("closed")).unwrap(),
            SignupMode::Closed
        );
        assert!(SignupMode::parse(Some("Closed")).is_err());
        assert!(SignupMode::parse(Some("invite")).is_err());
    }
}
