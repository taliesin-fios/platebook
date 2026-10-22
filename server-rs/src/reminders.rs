// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Opt-in day-before reminder emails (roadmap 3.2).
//!
//! Every [`INTERVAL`] the server looks for sessions starting within the next
//! [`LEAD`] whose account turned reminders on (`settings.remind_client` /
//! `remind_chef`). A session's start is its date plus its time (09:00 when
//! the time is blank or not `HH:MM`), read in the account's
//! `settings.timezone` (UTC when unset).
//!
//! Reminders go out one at a time, each in its own transaction: lock the next
//! due session (`FOR UPDATE SKIP LOCKED`, so several server instances never
//! pick the same one), re-read it, set `sessions.*_reminder_sent_at`, send,
//! and only then commit. The row lock is held across the send, so the
//! session can't be cancelled or moved in between (that update waits), and a
//! failed send, crash or redeploy rolls the marker back for the next run to
//! retry. The cost is at-least-once delivery: a crash after the email left
//! but before the commit would send it again. Moving a session's date or time
//! clears both markers (`routes::sessions::update`).
//!
//! Finding the next due session spans accounts, so like the other pre-tenant
//! lookups it runs as the connecting user; the transaction then switches to
//! that account's tenant (`db::enter_tenant`) for everything else.

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use crate::db::{Tx, enter_tenant};
use crate::email::{self, Reminder};
use crate::state::AppState;

/// How often the job runs.
pub const INTERVAL: Duration = Duration::from_secs(5 * 60);
/// How far ahead of a session its reminders go out.
pub const LEAD_HOURS: i32 = 24;
/// Reminders sent per kind per run (the rest wait for the next run).
const BATCH: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Client,
    Chef,
}

impl Kind {
    fn sent_column(self) -> &'static str {
        match self {
            Kind::Client => "client_reminder_sent_at",
            Kind::Chef => "chef_reminder_sent_at",
        }
    }

    fn setting(self) -> &'static str {
        match self {
            Kind::Client => "remind_client",
            Kind::Chef => "remind_chef",
        }
    }

    /// A client reminder needs somewhere to go.
    fn extra_filter(self) -> &'static str {
        match self {
            Kind::Client => "AND COALESCE(c.email, '') <> ''",
            Kind::Chef => "",
        }
    }
}

/// What one run did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunStats {
    pub sent: usize,
    pub failed: usize,
}

/// Starts the background job (from `main`).
pub fn start(state: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match run_once(&state, Utc::now()).await {
                Ok(s) if s.sent + s.failed > 0 => {
                    tracing::info!(sent = s.sent, failed = s.failed, "[reminders] run");
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(%err, "[reminders] run failed"),
            }
        }
    });
}

/// One pass: sends every reminder due at `now` (up to [`BATCH`] per kind).
pub async fn run_once(state: &AppState, now: DateTime<Utc>) -> Result<RunStats, sqlx::Error> {
    let mut stats = RunStats::default();
    for kind in [Kind::Client, Kind::Chef] {
        // Failures stay unsent for the next run, not retried in a loop now.
        let mut failed: Vec<i64> = Vec::new();
        for _ in 0..BATCH {
            let mut tx = state.pool.begin().await?;
            let Some((session_id, account_id)) = lock_next_due(&mut tx, kind, now, &failed).await?
            else {
                break;
            };
            match deliver(state, &mut tx, kind, session_id, account_id, now).await {
                Ok(()) => {
                    tx.commit().await?;
                    stats.sent += 1;
                }
                Err(err) => {
                    // Dropping the transaction rolls the sent marker back.
                    drop(tx);
                    failed.push(session_id);
                    stats.failed += 1;
                    tracing::warn!(%err, session_id, ?kind, "[reminders] send failed; will retry");
                }
            }
        }
    }
    Ok(stats)
}

/// Locks the next due, unsent reminder of `kind` (skipping rows another
/// instance holds and ids in `skip`) and returns `(session_id, account_id)`.
async fn lock_next_due(
    tx: &mut Tx,
    kind: Kind,
    now: DateTime<Utc>,
    skip: &[i64],
) -> Result<Option<(i64, i64)>, sqlx::Error> {
    // Session start as an instant: local date + time in the account's zone.
    // An unknown stored zone falls back to UTC rather than failing the run.
    let sql = format!(
        "SELECT s.id, s.account_id
           FROM sessions s
           JOIN settings st ON st.account_id = s.account_id
           JOIN clients c ON c.id = s.client_id
           LEFT JOIN pg_timezone_names z ON z.name = st.timezone
           WHERE st.{setting}
             AND s.{sent} IS NULL
             AND s.status IN ('Pending', 'Confirmed')
             AND c.is_active IS NOT FALSE
             {extra}
             AND s.session_date BETWEEN ($1 AT TIME ZONE 'UTC')::date - 1
                                    AND ($1 AT TIME ZONE 'UTC')::date + 2
             AND ((s.session_date + COALESCE(
                     substring(s.session_time FROM '^(([01]?[0-9]|2[0-3]):[0-5][0-9])')::time,
                     time '09:00'))
                  AT TIME ZONE COALESCE(z.name, 'UTC'))
                 BETWEEN $1 AND $1 + make_interval(hours => $2)
             AND s.id <> ALL($3)
           ORDER BY s.session_date, s.id
           LIMIT 1
           FOR UPDATE OF s SKIP LOCKED",
        setting = kind.setting(),
        sent = kind.sent_column(),
        extra = kind.extra_filter(),
    );
    sqlx::query_as(&sql)
        .bind(now)
        .bind(LEAD_HOURS)
        .bind(skip)
        .fetch_optional(&mut **tx)
        .await
}

#[derive(sqlx::FromRow)]
struct SessionForReminder {
    session_date: Option<NaiveDate>,
    session_time: Option<String>,
    serving_size: Option<i32>,
    client_name: String,
    client_email: Option<String>,
    client_phone: Option<String>,
    address: Option<String>,
    parking_notes: Option<String>,
    allergens: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
enum SendError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Email(#[from] email::EmailError),
    #[error("session not found")]
    Missing,
}

/// Within the locked transaction: marks the reminder sent, reads the session
/// as its tenant, and sends. The caller commits only if this succeeds.
async fn deliver(
    state: &AppState,
    tx: &mut Tx,
    kind: Kind,
    session_id: i64,
    account_id: i64,
    now: DateTime<Utc>,
) -> Result<(), SendError> {
    enter_tenant(tx, account_id).await?;
    sqlx::query(&format!(
        "UPDATE sessions SET {} = $1 WHERE id = $2",
        kind.sent_column()
    ))
    .bind(now)
    .bind(session_id)
    .execute(&mut **tx)
    .await?;
    let s = sqlx::query_as::<_, SessionForReminder>(
        "SELECT s.session_date, s.session_time, s.serving_size,
                c.name AS client_name, c.email AS client_email, c.phone AS client_phone,
                c.address, c.parking_notes, c.allergens
         FROM sessions s JOIN clients c ON c.id = s.client_id
         WHERE s.id = $1 AND s.account_id = $2",
    )
    .bind(session_id)
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(SendError::Missing)?;
    let dishes: Vec<String> = sqlx::query_scalar(
        "SELECT r.name FROM session_dishes sd JOIN recipes r ON r.id = sd.recipe_id
         WHERE sd.session_id = $1 ORDER BY sd.sort_order, sd.id",
    )
    .bind(session_id)
    .fetch_all(&mut **tx)
    .await?;

    let allergens: Vec<String> = match &s.allergens {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => vec![],
    };
    let when = when_label(s.session_date, s.session_time.as_deref());
    let r = Reminder {
        account_id,
        session_id,
        client_name: &s.client_name,
        client_email: s.client_email.as_deref(),
        client_phone: s.client_phone.as_deref(),
        address: s.address.as_deref(),
        parking_notes: s.parking_notes.as_deref(),
        allergens: &allergens,
        when: &when,
        dishes: &dishes,
        serving_size: s.serving_size,
    };
    match kind {
        Kind::Client => email::send_client_reminder(state, &r).await?,
        Kind::Chef => email::send_chef_reminder(state, &r).await?,
    }
    Ok(())
}

/// "Tuesday, October 20 at 2:30 PM", or just the date without a time. A
/// time that isn't `HH:MM` is shown as the chef typed it.
fn when_label(date: Option<NaiveDate>, time: Option<&str>) -> String {
    let day = date
        .map(|d| d.format("%A, %B %-d").to_string())
        .unwrap_or_else(|| "soon".into());
    match time.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            let shown = chrono::NaiveTime::parse_from_str(t.get(..5).unwrap_or(t), "%H:%M")
                .map(|tm| tm.format("%-I:%M %p").to_string())
                .unwrap_or_else(|_| t.to_string());
            format!("{day} at {shown}")
        }
        None => day,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_the_slot() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 20);
        assert_eq!(
            when_label(d, Some("10:00")),
            "Tuesday, October 20 at 10:00 AM"
        );
        assert_eq!(
            when_label(d, Some("14:30")),
            "Tuesday, October 20 at 2:30 PM"
        );
        assert_eq!(
            when_label(d, Some("after lunch")),
            "Tuesday, October 20 at after lunch"
        );
        assert_eq!(when_label(d, Some(" ")), "Tuesday, October 20");
        assert_eq!(when_label(d, None), "Tuesday, October 20");
    }
}
