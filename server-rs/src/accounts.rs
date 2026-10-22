// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Port of `server/utils/accounts.js`: booking slugs, account creation and
//! per-account settings lookups.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use unicode_normalization::UnicodeNormalization;

/// Paths the client app owns; a chef can't claim these as a booking slug.
pub const RESERVED_SLUGS: &[&str] = &[
    "api",
    "admin",
    "login",
    "signup",
    "welcome",
    "onboarding",
    "book",
    "intake",
    "settings",
    "new",
    "forgot",
    "reset",
    "health",
    "assets",
];

/// Port of `slugify()`: lowercase ASCII letters/digits joined by single
/// hyphens, accents stripped, at most 40 characters.
pub fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    // Same combining-mark range as the JS `/[̀-ͯ]/g` strip.
    for c in value
        .to_lowercase()
        .nfkd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
    {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.push(c);
        } else {
            pending_hyphen = true;
        }
    }
    out.truncate(40);
    out.trim_end_matches('-').to_string()
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow {
    pub id: i64,
    pub email: String,
    pub slug: String,
    pub password_hash: Option<String>,
    pub onboarding_completed_at: Option<DateTime<Utc>>,
}

/// First free slug based on `base`: my-kitchen, my-kitchen-2, my-kitchen-3…
pub async fn unique_slug(conn: &mut PgConnection, base: &str) -> Result<String, sqlx::Error> {
    let mut root = slugify(base);
    if root.is_empty() {
        root = "chef".to_string();
    }
    if RESERVED_SLUGS.contains(&root.as_str()) {
        root = format!("{root}-kitchen");
    }
    for n in 1..10_000 {
        let candidate = if n == 1 {
            root.clone()
        } else {
            format!("{root}-{n}")
        };
        let taken = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM accounts WHERE slug = $1) AS "taken!""#,
            candidate
        )
        .fetch_one(&mut *conn)
        .await?;
        if !taken {
            return Ok(candidate);
        }
    }
    Err(sqlx::Error::Protocol(
        "Could not allocate a unique slug".into(),
    ))
}

/// Creates the account and its settings row in one transaction. The caller
/// handles validation and hashing; `conn` should be a fresh transaction.
pub async fn create_account(
    conn: &mut PgConnection,
    email: &str,
    password_hash: Option<&str>,
    business_name: &str,
) -> Result<AccountRow, sqlx::Error> {
    let base = if business_name.is_empty() {
        email.split('@').next().unwrap_or_default()
    } else {
        business_name
    };
    let slug = unique_slug(conn, base).await?;
    let account = sqlx::query_as!(
        AccountRow,
        r#"INSERT INTO accounts (email, password_hash, slug) VALUES ($1, $2, $3)
           RETURNING id, email, slug, password_hash, onboarding_completed_at"#,
        email,
        password_hash,
        slug
    )
    .fetch_one(&mut *conn)
    .await?;
    // settings is under RLS (FORCEd), so name the tenant before inserting.
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(account.id.to_string())
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "INSERT INTO settings (account_id, business_name, email) VALUES ($1, $2, $3)",
        account.id,
        business_name,
        email
    )
    .execute(&mut *conn)
    .await?;
    Ok(account)
}

pub async fn get_account(
    conn: &mut PgConnection,
    id: i64,
) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, email, slug, password_hash, onboarding_completed_at FROM accounts WHERE id = $1",
        id
    )
    .fetch_optional(conn)
    .await
}

pub async fn find_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, email, slug, password_hash, onboarding_completed_at FROM accounts WHERE lower(email) = lower($1)",
        email
    )
    .fetch_optional(conn)
    .await
}

pub async fn get_account_by_slug(
    conn: &mut PgConnection,
    slug: &str,
) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, email, slug, password_hash, onboarding_completed_at FROM accounts WHERE slug = $1",
        slug.to_lowercase()
    )
    .fetch_optional(conn)
    .await
}

/// One account's settings row (`SELECT * FROM settings`).
#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow, ts_rs::TS)]
#[ts(export, rename = "SettingsRow")]
pub struct Settings {
    #[ts(type = "number")]
    pub account_id: i64,
    pub business_name: Option<String>,
    pub chef_name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    #[ts(type = "number | null")]
    pub default_session_window: Option<Decimal>,
    pub default_serving_size: Option<i32>,
    #[ts(type = "number | null")]
    pub default_service_fee: Option<Decimal>,
    pub logo_base64: Option<String>,
    pub invoice_prefix: Option<String>,
    #[ts(type = "number | null")]
    pub deposit_cents: Option<i64>,
    pub cancellation_hours: Option<i32>,
    pub payment_methods: Option<String>,
    pub booking_terms: Option<String>,
    pub response_time_text: Option<String>,
    #[serde(serialize_with = "crate::serde_util::json_array::serialize")]
    #[ts(type = "string[]")]
    pub service_types: Option<serde_json::Value>,
    #[serde(serialize_with = "crate::serde_util::ts::serialize")]
    #[ts(type = "string | null")]
    pub updated_at: Option<DateTime<Utc>>,
    /// Email the client a reminder the day before each session.
    #[serde(serialize_with = "crate::serde_util::bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub remind_client: bool,
    /// Email the chef a prep sheet the day before each session.
    #[serde(serialize_with = "crate::serde_util::bool_as_int::serialize")]
    #[ts(type = "0 | 1")]
    pub remind_chef: bool,
    /// IANA time zone for session times (NULL = UTC).
    pub timezone: Option<String>,
}

/// Port of `getSettings(db, accountId)`: always returns a value (all fields
/// empty if the account has no settings row). Run it on a tenant
/// transaction (`db::begin_tenant`); RLS hides other accounts' rows anyway.
pub async fn get_settings(
    conn: &mut PgConnection,
    account_id: i64,
) -> Result<Settings, sqlx::Error> {
    let row = sqlx::query_as!(
        Settings,
        r#"SELECT account_id, business_name, chef_name, phone, email, website, location,
                  default_session_window, default_serving_size, default_service_fee, logo_base64,
                  invoice_prefix, deposit_cents, cancellation_hours, payment_methods, booking_terms,
                  response_time_text, service_types, updated_at,
                  remind_client, remind_chef, timezone
           FROM settings WHERE account_id = $1"#,
        account_id
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.unwrap_or(Settings {
        account_id,
        ..Settings::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_matches_node() {
        assert_eq!(slugify("Sunday Suppers"), "sunday-suppers");
        assert_eq!(slugify("  Crème Brûlée & Co. "), "creme-brulee-co");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("Onboarding Co"), "onboarding-co");
        assert_eq!(slugify(&"a".repeat(50)).len(), 40);
        // Cut at 40 can leave a trailing hyphen, which is then trimmed.
        assert_eq!(slugify(&format!("{} b", "a".repeat(39))), "a".repeat(39));
    }
}
