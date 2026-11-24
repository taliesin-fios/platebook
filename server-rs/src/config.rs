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
    /// `EMAIL_TLS`: how the SMTP connection is secured.
    pub email_tls: EmailTls,
    /// `EMAIL_TIMEOUT_SECS`: how long one SMTP attempt may take.
    pub email_timeout: std::time::Duration,
    /// `EMAIL_CAPTURE=true` (never in production): with no SMTP settings,
    /// mail is kept in the in-memory outbox and logged instead of sent, and
    /// reported as "captured", never as sent. For local development and tests.
    pub email_capture: bool,
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
    /// `TRUST_PROXY_HOPS`: how many reverse proxies in front of the server
    /// append to `X-Forwarded-For`. 0 = clients connect directly (the header
    /// is ignored); 1 (default) = one proxy, e.g. Caddy or nginx.
    pub trust_proxy_hops: u8,
    pub anthropic_api_key: Option<String>,
    /// `ANTHROPIC_MODEL` override; each route has its own default otherwise.
    pub anthropic_model: Option<String>,
    /// Anthropic API base (overridable so tests can point at a fake).
    pub anthropic_api_base: String,
    /// `AI_ALLOW_PRIVATE_URLS=true`: let chefs point their AI settings at
    /// private or plain-HTTP addresses (a model server on the operator's own
    /// network). Off by default, so a chef can't make the server call
    /// internal services.
    pub ai_allow_private_urls: bool,
    /// Directory with the built React app, served when set (production).
    pub client_dist: Option<String>,
    /// Optional pg_dump backups (Railway Postgres has its own; off by default).
    pub backup: BackupConfig,
    /// `SIGNUP_MODE`: who may create an account.
    pub signup_mode: SignupMode,
    /// `DEMO_MODE=true`: keep a shared demo account, open to anyone with
    /// "Try the demo", and wipe and reseed it every `DEMO_RESET_HOURS`.
    pub demo: DemoConfig,
}

/// The public demo account (`DEMO_MODE`, `DEMO_RESET_HOURS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DemoConfig {
    pub enabled: bool,
    /// Hours between resets (1-168), default 24.
    pub reset_hours: u32,
}

impl DemoConfig {
    pub fn parse(mode: Option<&str>, hours: Option<&str>) -> Result<Self, ConfigError> {
        let enabled = match mode {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => return Err(ConfigError::Invalid("DEMO_MODE")),
        };
        let reset_hours = match hours {
            None => 24,
            Some(h) => h
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|h| (1..=168).contains(h))
                .ok_or(ConfigError::Invalid("DEMO_RESET_HOURS"))?,
        };
        Ok(DemoConfig {
            enabled,
            reset_hours,
        })
    }
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
/// account, for a single chef on their own server), `invite` (only with a
/// one-time link from `chef-server invite create`) or `closed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupMode {
    Open,
    First,
    Invite,
    Closed,
}

impl SignupMode {
    pub fn parse(v: Option<&str>) -> Result<Self, ConfigError> {
        match v {
            None | Some("open") => Ok(Self::Open),
            Some("first") => Ok(Self::First),
            Some("invite") => Ok(Self::Invite),
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
        let trust_proxy_hops = match var("TRUST_PROXY_HOPS") {
            Some(v) => v
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|n| *n <= 10)
                .ok_or(ConfigError::Invalid("TRUST_PROXY_HOPS"))?,
            None => 1,
        };
        let email_port = match var("EMAIL_PORT") {
            Some(p) => p.parse().map_err(|_| ConfigError::Invalid("EMAIL_PORT"))?,
            None => 587,
        };
        let email_tls = EmailTls::parse(var("EMAIL_TLS").as_deref(), email_port)?;
        let email_capture = match var("EMAIL_CAPTURE").as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => return Err(ConfigError::Invalid("EMAIL_CAPTURE")),
        };
        let email_host = var("EMAIL_HOST").unwrap_or_default();
        let has_credentials = var("EMAIL_USER").is_some() || var("EMAIL_PASS").is_some();
        check_email_settings(
            production,
            &email_host,
            email_tls,
            email_capture,
            has_credentials,
        )?;
        let email_timeout = match var("EMAIL_TIMEOUT_SECS") {
            Some(v) => v
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|n| (1..=120).contains(n))
                .map(std::time::Duration::from_secs)
                .ok_or(ConfigError::Invalid("EMAIL_TIMEOUT_SECS"))?,
            None => DEFAULT_EMAIL_TIMEOUT,
        };
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
            email_host,
            allow_dev_link: var("ALLOW_DEV_LINK").as_deref() == Some("true"),
            email_port,
            email_user: var("EMAIL_USER").unwrap_or_default(),
            email_pass: var("EMAIL_PASS").unwrap_or_default(),
            email_from: var("EMAIL_FROM")
                .or_else(|| var("EMAIL_USER"))
                .unwrap_or_else(|| "Platebook <noreply@example.com>".to_string()),
            email_tls,
            email_timeout,
            email_capture,
            intake_secret,
            payments_mode: PaymentsMode::parse(var("PAYMENTS_MODE").as_deref()),
            stripe_secret_key: var("STRIPE_SECRET_KEY"),
            stripe_webhook_secret: var("STRIPE_WEBHOOK_SECRET"),
            stripe_api_base: var("STRIPE_API_BASE")
                .unwrap_or_else(|| "https://api.stripe.com".to_string()),
            booking_fee_cents: var("BOOKING_FEE_CENTS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            trust_proxy_hops,
            anthropic_api_key: var("ANTHROPIC_API_KEY"),
            anthropic_model: var("ANTHROPIC_MODEL"),
            anthropic_api_base: var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            ai_allow_private_urls: var("AI_ALLOW_PRIVATE_URLS").as_deref() == Some("true"),
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
            demo: DemoConfig::parse(
                var("DEMO_MODE").as_deref(),
                var("DEMO_RESET_HOURS").as_deref(),
            )?,
        })
    }

    /// A config for tests: email captured in the outbox (never sent), manual
    /// payments, no AI key.
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
            email_tls: EmailTls::Starttls,
            email_timeout: DEFAULT_EMAIL_TIMEOUT,
            email_capture: true,
            intake_secret: "test-intake-secret".to_string(),
            payments_mode: PaymentsMode::Manual,
            stripe_secret_key: None,
            stripe_webhook_secret: None,
            stripe_api_base: "https://api.stripe.com".to_string(),
            booking_fee_cents: 5000,
            trust_proxy_hops: 1,
            anthropic_api_key: None,
            anthropic_model: None,
            anthropic_api_base: "https://api.anthropic.com".to_string(),
            ai_allow_private_urls: false,
            client_dist: None,
            backup: BackupConfig::default(),
            signup_mode: SignupMode::Open,
            demo: DemoConfig::default(),
        }
    }

    /// How mail leaves this server, if it can.
    ///
    /// SMTP needs `EMAIL_HOST`; `EMAIL_USER` and `EMAIL_PASS` are optional
    /// (a local relay or mail sink may take none) but go together. Without a
    /// host, mail is captured only when `EMAIL_CAPTURE=true` outside
    /// production; otherwise there is no transport and every send fails.
    pub fn email_mode(&self) -> EmailMode {
        let (user, pass) = (!self.email_user.is_empty(), !self.email_pass.is_empty());
        if !self.email_host.is_empty() {
            if user != pass {
                return EmailMode::Disabled("EMAIL_USER and EMAIL_PASS must be set together");
            }
            return EmailMode::Smtp;
        }
        if user || pass {
            return EmailMode::Disabled("EMAIL_USER/EMAIL_PASS are set but EMAIL_HOST is empty");
        }
        if self.email_capture && !self.production {
            return EmailMode::Capture;
        }
        EmailMode::Disabled("no SMTP settings (EMAIL_HOST is empty)")
    }

    /// Whether mail can be handed to SMTP or the local capture outbox.
    pub fn email_available(&self) -> bool {
        !matches!(self.email_mode(), EmailMode::Disabled(_))
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
            .field("trust_proxy_hops", &self.trust_proxy_hops)
            .field("production", &self.production)
            .field("jwt_secret", &"<redacted>")
            .field("client_url", &self.client_url)
            .field("email_mode", &self.email_mode())
            .field("email_tls", &self.email_tls)
            .field("allow_dev_link", &self.allow_dev_link)
            .field("payments_mode", &self.payments_mode)
            .field("stripe_configured", &self.stripe_secret_key.is_some())
            .field("ai_server_key", &self.anthropic_api_key.is_some())
            .field("ai_allow_private_urls", &self.ai_allow_private_urls)
            .field("client_dist", &self.client_dist)
            .field("auto_backup", &self.backup.enabled)
            .field("signup_mode", &self.signup_mode)
            .field("demo", &self.demo)
            .finish()
    }
}

/// Default for `EMAIL_TIMEOUT_SECS`.
pub const DEFAULT_EMAIL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// See [`Config::email_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailMode {
    /// Handed to the SMTP server in `EMAIL_HOST`.
    Smtp,
    /// Kept in the in-memory outbox and logged; nothing leaves the server.
    Capture,
    /// No usable transport (the reason is for the log).
    Disabled(&'static str),
}

/// `EMAIL_TLS`: `starttls` (default), `tls` (default on port 465) or `none`
/// (plain SMTP, for a local mail sink or relay).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailTls {
    Starttls,
    Tls,
    None,
}

impl EmailTls {
    fn parse(v: Option<&str>, port: u16) -> Result<Self, ConfigError> {
        match v.map(str::trim) {
            None | Some("") if port == 465 => Ok(EmailTls::Tls),
            None | Some("") | Some("starttls") => Ok(EmailTls::Starttls),
            Some("tls") => Ok(EmailTls::Tls),
            Some("none") => Ok(EmailTls::None),
            Some(_) => Err(ConfigError::Invalid("EMAIL_TLS")),
        }
    }
}

/// Startup checks for settings that would otherwise silently misbehave.
fn check_email_settings(
    production: bool,
    host: &str,
    tls: EmailTls,
    capture: bool,
    has_credentials: bool,
) -> Result<(), ConfigError> {
    if capture && production {
        // Capture would make a live server look like it emails clients.
        return Err(ConfigError::Invalid(
            "EMAIL_CAPTURE (not allowed with NODE_ENV=production)",
        ));
    }
    if capture && !host.is_empty() {
        return Err(ConfigError::Invalid(
            "EMAIL_CAPTURE (set either EMAIL_HOST or EMAIL_CAPTURE, not both)",
        ));
    }
    if production && tls == EmailTls::None && has_credentials {
        return Err(ConfigError::Invalid(
            "EMAIL_TLS=none (would send SMTP credentials unencrypted in production)",
        ));
    }
    Ok(())
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
        assert_eq!(
            SignupMode::parse(Some("invite")).unwrap(),
            SignupMode::Invite
        );
        assert!(SignupMode::parse(Some("invites")).is_err());
    }

    fn mail(host: &str, user: &str, pass: &str, capture: bool, production: bool) -> Config {
        Config {
            email_host: host.into(),
            email_user: user.into(),
            email_pass: pass.into(),
            email_capture: capture,
            production,
            ..Config::for_tests("t")
        }
    }

    #[test]
    fn email_mode_never_fakes_a_transport() {
        use EmailMode::*;
        assert_eq!(
            mail("smtp.example.com", "u", "p", false, true).email_mode(),
            Smtp
        );
        // A relay or local sink may need no login.
        assert_eq!(mail("localhost", "", "", false, false).email_mode(), Smtp);
        // Partial settings: no transport, not a silent outbox.
        assert!(matches!(
            mail("smtp.example.com", "u", "", false, true).email_mode(),
            Disabled(_)
        ));
        assert!(matches!(
            mail("", "u", "p", true, false).email_mode(),
            Disabled(_)
        ));
        // Nothing configured: capture only when asked for, never in production.
        assert!(matches!(
            mail("", "", "", false, false).email_mode(),
            Disabled(_)
        ));
        assert_eq!(mail("", "", "", true, false).email_mode(), Capture);
        assert!(matches!(
            mail("", "", "", true, true).email_mode(),
            Disabled(_)
        ));
        assert!(!mail("", "", "", false, true).email_available());
    }

    #[test]
    fn email_settings_that_would_mislead_are_refused_at_startup() {
        assert!(check_email_settings(true, "", EmailTls::Starttls, true, false).is_err());
        assert!(check_email_settings(false, "mail", EmailTls::Starttls, true, false).is_err());
        assert!(check_email_settings(true, "mail", EmailTls::None, false, true).is_err());
        assert!(check_email_settings(true, "mail", EmailTls::None, false, false).is_ok());
        assert!(check_email_settings(false, "", EmailTls::Starttls, true, false).is_ok());
        assert_eq!(EmailTls::parse(None, 465).unwrap(), EmailTls::Tls);
        assert_eq!(EmailTls::parse(None, 587).unwrap(), EmailTls::Starttls);
        assert_eq!(EmailTls::parse(Some("none"), 1025).unwrap(), EmailTls::None);
        assert!(EmailTls::parse(Some("plain"), 25).is_err());
    }

    #[test]
    fn demo_config_parses_mode_and_hours() {
        let off = DemoConfig::parse(None, None).unwrap();
        assert!(!off.enabled);
        assert_eq!(off.reset_hours, 24);
        let on = DemoConfig::parse(Some("true"), Some("6")).unwrap();
        assert!(on.enabled);
        assert_eq!(on.reset_hours, 6);
        assert!(DemoConfig::parse(Some("yes"), None).is_err());
        assert!(DemoConfig::parse(Some("true"), Some("0")).is_err());
        assert!(DemoConfig::parse(Some("true"), Some("169")).is_err());
        assert!(DemoConfig::parse(Some("true"), Some("12h")).is_err());
    }
}
