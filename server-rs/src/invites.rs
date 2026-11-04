// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Single-use sign-up invites (`SIGNUP_MODE=invite`).
//!
//! The server owner runs `chef-server invite create` on the server; it prints
//! a `/signup?invite=<token>` link once. Only the token's SHA-256 is stored
//! (`signup_invites`), so a database read can't be turned into a working
//! link. A link signs up one account, then it's used up. It also stops
//! working when it expires or is revoked. An invite made for an email address
//! only works for that address.
//!
//! This runs before any account exists, so it uses the pool directly, like
//! sign-up itself; `chef_app` has no access to the table.

use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection, PgPool};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::routes::settings::EMAIL_RE;

/// Days an invite lasts when `--days` isn't given.
pub const DEFAULT_DAYS: i64 = 14;
/// Longest an invite may last.
pub const MAX_DAYS: i64 = 90;

/// A fresh random token: 32 bytes from the OS, base64url without padding.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// What is stored for a token: its SHA-256, hex.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.trim().as_bytes()))
}

/// The link a chef opens to sign up with `token`.
pub fn signup_link(client_url: &str, token: &str) -> String {
    format!("{}/signup?invite={token}", client_url.trim_end_matches('/'))
}

fn norm_email(v: &str) -> String {
    v.trim().to_lowercase()
}

/// A newly created invite. `token` exists only here; it is not stored.
#[derive(Debug)]
pub struct Created {
    pub id: i64,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

/// Creates an invite lasting `days` (1 to [`MAX_DAYS`]), optionally for one
/// email address.
pub async fn create(
    pool: &PgPool,
    email: Option<&str>,
    days: i64,
    note: Option<&str>,
) -> Result<Created, String> {
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(format!("--days must be from 1 to {MAX_DAYS}"));
    }
    let email = email.map(norm_email).filter(|e| !e.is_empty());
    if let Some(e) = &email
        && (!EMAIL_RE.is_match(e) || e.len() > 254)
    {
        return Err(format!("{e} isn't a valid email address"));
    }
    let note = note.map(str::trim).filter(|n| !n.is_empty());
    let token = new_token();
    let expires_at = Utc::now() + Duration::days(days);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO signup_invites (token_hash, email, note, expires_at)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(hash_token(&token))
    .bind(&email)
    .bind(note)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("could not create the invite: {e}"))?;
    Ok(Created {
        id,
        token,
        expires_at,
    })
}

/// One row of `chef-server invite list`.
#[derive(Debug, FromRow)]
pub struct Listed {
    pub id: i64,
    pub email: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub used_by_email: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl Listed {
    /// `open`, `used by …`, `revoked` or `expired`.
    pub fn state(&self, now: DateTime<Utc>) -> String {
        if let Some(at) = self.used_at {
            let who = self.used_by_email.as_deref().unwrap_or("a deleted account");
            format!("used by {who} on {}", at.format("%Y-%m-%d"))
        } else if self.revoked_at.is_some() {
            "revoked".to_string()
        } else if self.expires_at <= now {
            "expired".to_string()
        } else {
            format!(
                "open until {}",
                self.expires_at.format("%Y-%m-%d %H:%M UTC")
            )
        }
    }
}

pub async fn list(pool: &PgPool) -> Result<Vec<Listed>, sqlx::Error> {
    sqlx::query_as::<_, Listed>(
        "SELECT i.id, i.email, i.note, i.created_at, i.expires_at, i.used_at,
                a.email AS used_by_email, i.revoked_at
           FROM signup_invites i
           LEFT JOIN accounts a ON a.id = i.used_by_account_id
          ORDER BY i.id",
    )
    .fetch_all(pool)
    .await
}

/// Revokes an unused invite. `false` when there is no such open invite.
pub async fn revoke(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE signup_invites SET revoked_at = now()
          WHERE id = $1 AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Whether `token` is an open invite (unused, not revoked, not expired).
pub async fn is_open(conn: &mut PgConnection, token: &str) -> Result<bool, sqlx::Error> {
    if token.trim().is_empty() {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM signup_invites
                         WHERE token_hash = $1 AND used_at IS NULL
                           AND revoked_at IS NULL AND expires_at > now())",
    )
    .bind(hash_token(token))
    .fetch_one(conn)
    .await
}

/// Why an invite can't be used for a sign-up.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Unknown, used, revoked or expired.
    NotOpen,
    /// Made for a different email address.
    WrongEmail,
}

/// Locks the open invite for `token` inside the sign-up transaction and
/// checks it against `email`. Two sign-ups racing on one link: the second
/// waits on the row lock, then finds it used.
pub async fn claim(
    tx: &mut PgConnection,
    token: &str,
    email: &str,
) -> Result<Result<i64, Refused>, sqlx::Error> {
    if token.trim().is_empty() {
        return Ok(Err(Refused::NotOpen));
    }
    let row: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, email FROM signup_invites
          WHERE token_hash = $1 AND used_at IS NULL
            AND revoked_at IS NULL AND expires_at > now()
          FOR UPDATE",
    )
    .bind(hash_token(token))
    .fetch_optional(&mut *tx)
    .await?;
    Ok(match row {
        None => Err(Refused::NotOpen),
        Some((_, Some(for_email))) if for_email != norm_email(email) => Err(Refused::WrongEmail),
        Some((id, _)) => Ok(id),
    })
}

/// Marks a claimed invite used by `account_id` (same transaction as the new
/// account, so a failed sign-up leaves the link working).
pub async fn mark_used(
    tx: &mut PgConnection,
    invite_id: i64,
    account_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE signup_invites SET used_at = now(), used_by_account_id = $2 WHERE id = $1")
        .bind(invite_id)
        .bind(account_id)
        .execute(tx)
        .await?;
    Ok(())
}

pub const USAGE: &str = "\
Usage: chef-server invite <command>

  create [--email ADDRESS] [--days N] [--note TEXT]
      Print a one-time sign-up link. --email limits it to that address;
      --days sets how long it works (default 14, at most 90).
  list
      Show every invite and whether it is open, used, revoked or expired.
  revoke ID
      Stop an unused invite from working.

Sign-up links only work while SIGNUP_MODE=invite. Needs DATABASE_URL (and
CLIENT_URL for the link), the same as the server.";

/// `chef-server invite …`: the text to print on success, or an error message.
pub async fn run_cli(pool: &PgPool, client_url: &str, args: &[String]) -> Result<String, String> {
    let mut it = args.iter().map(String::as_str);
    match it.next() {
        Some("create") => {
            let (mut email, mut days, mut note) = (None, DEFAULT_DAYS, None);
            while let Some(flag) = it.next() {
                let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
                match flag {
                    "--email" => email = Some(value()?.to_string()),
                    "--note" => note = Some(value()?.to_string()),
                    "--days" => {
                        days = value()?
                            .parse()
                            .map_err(|_| "--days must be a whole number".to_string())?;
                    }
                    other => return Err(format!("unknown option {other}\n\n{USAGE}")),
                }
            }
            let created = create(pool, email.as_deref(), days, note.as_deref()).await?;
            let mut out = format!(
                "Invite {} (works once, until {}):\n{}\n",
                created.id,
                created.expires_at.format("%Y-%m-%d %H:%M UTC"),
                signup_link(client_url, &created.token)
            );
            if let Some(e) = email.as_deref().map(norm_email) {
                out.push_str(&format!("Only {e} can sign up with it.\n"));
            }
            out.push_str("The link is shown only now; it can't be looked up later.");
            Ok(out)
        }
        Some("list") => {
            let rows = list(pool).await.map_err(|e| e.to_string())?;
            if rows.is_empty() {
                return Ok("No invites yet.".to_string());
            }
            let now = Utc::now();
            Ok(rows
                .iter()
                .map(|r| {
                    let mut line = format!(
                        "{:>4}  {}  {}  {}",
                        r.id,
                        r.created_at.format("%Y-%m-%d"),
                        r.email.as_deref().unwrap_or("any email"),
                        r.state(now)
                    );
                    if let Some(n) = &r.note {
                        line.push_str(&format!("  ({n})"));
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n"))
        }
        Some("revoke") => {
            let id: i64 = it
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("revoke needs an invite ID\n\n{USAGE}"))?;
            if revoke(pool, id).await.map_err(|e| e.to_string())? {
                Ok(format!("Invite {id} revoked."))
            } else {
                Err(format!(
                    "No open invite {id} (it may be used, revoked or missing)."
                ))
            }
        }
        Some("help" | "--help" | "-h") | None => Ok(USAGE.to_string()),
        Some(other) => Err(format!("unknown command {other}\n\n{USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_url_safe_and_hashed() {
        let (a, b) = (new_token(), new_token());
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_eq!(hash_token(&a).len(), 64);
        assert_eq!(hash_token(&a), hash_token(&format!(" {a} ")));
        assert_ne!(hash_token(&a), hash_token(&b));
    }

    #[test]
    fn links_point_at_the_signup_page() {
        assert_eq!(
            signup_link("https://app.example.com/", "abc"),
            "https://app.example.com/signup?invite=abc"
        );
    }
}
