// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

//! Replacement for `server/utils/backupScheduler.js` + `server/scripts/backup.js`.
//!
//! On Railway, managed Postgres already takes daily backups with
//! point-in-time restore, so this is **off by default**. Set
//! `AUTO_BACKUP=true` to also keep `pg_dump` custom-format dumps in
//! `BACKUP_DIR` (a mounted volume), e.g. for an off-platform copy:
//!
//! - checks hourly; runs once per UTC day after `AUTO_BACKUP_HOUR` (default 3),
//!   so a restart at any time neither misses nor repeats a day;
//! - writes `chef_app_YYYY-MM-DD_HHMMSS.dump` and keeps the newest 30.
//!
//! Restore with `pg_restore --clean --if-exists -d "$DATABASE_URL" <file>`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{NaiveDate, Timelike, Utc};

use crate::config::{BackupConfig, Config};

pub const MAX_BACKUPS: usize = 30;
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("could not create {0}: {1}")]
    Dir(PathBuf, std::io::Error),
    #[error("could not run pg_dump: {0}")]
    Spawn(std::io::Error),
    #[error("pg_dump failed: {0}")]
    Failed(String),
}

/// Runs one dump now and prunes old ones. Returns the new file.
pub async fn run_backup(database_url: &str, cfg: &BackupConfig) -> Result<PathBuf, BackupError> {
    let dir = PathBuf::from(&cfg.dir);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| BackupError::Dir(dir.clone(), e))?;
    let file = dir.join(format!(
        "chef_app_{}.dump",
        Utc::now().format("%Y-%m-%d_%H%M%S")
    ));
    let out = tokio::process::Command::new(&cfg.pg_dump)
        .arg("--format=custom")
        .arg("--no-owner")
        .arg("--file")
        .arg(&file)
        .arg("--dbname")
        .arg(database_url)
        .output()
        .await
        .map_err(BackupError::Spawn)?;
    if !out.status.success() {
        return Err(BackupError::Failed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    prune(&dir, MAX_BACKUPS).await;
    Ok(file)
}

/// Keeps the newest `keep` dumps (names sort chronologically).
async fn prune(dir: &Path, keep: usize) {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };
    let mut files = Vec::new();
    while let Ok(Some(e)) = entries.next_entry().await {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("chef_app_") && name.ends_with(".dump") {
            files.push(name);
        }
    }
    files.sort();
    files.reverse();
    for old in files.into_iter().skip(keep) {
        if let Err(e) = tokio::fs::remove_file(dir.join(&old)).await {
            tracing::warn!(file = %old, error = %e, "[backup] could not prune");
        }
    }
}

/// Whether a dump is due: not yet today, and past the target hour (UTC).
pub fn due(last: Option<NaiveDate>, now: chrono::DateTime<Utc>, hour: u32) -> bool {
    last != Some(now.date_naive()) && now.hour() >= hour
}

/// Starts the hourly check in the background (no-op unless `AUTO_BACKUP=true`).
pub fn start_scheduler(config: &Config) {
    if !config.backup.enabled {
        tracing::info!("automatic pg_dump backups off (Railway Postgres backups apply)");
        return;
    }
    let url = config.database_url.clone();
    let cfg = config.backup.clone();
    tracing::info!(dir = %cfg.dir, hour = cfg.hour, "automatic pg_dump backups enabled (daily, UTC)");
    tokio::spawn(async move {
        let mut last: Option<NaiveDate> = None;
        // First check shortly after boot, then hourly.
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let now = Utc::now();
            if due(last, now, cfg.hour) {
                match run_backup(&url, &cfg).await {
                    Ok(file) => {
                        last = Some(now.date_naive());
                        tracing::info!(file = %file.display(), "[backup] complete");
                    }
                    Err(e) => tracing::error!(error = %e, "[backup] automatic backup failed"),
                }
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn runs_once_per_day_after_the_hour() {
        let at = |h| Utc.with_ymd_and_hms(2026, 10, 14, h, 0, 0).unwrap();
        let today = at(0).date_naive();
        assert!(!due(None, at(2), 3));
        assert!(due(None, at(3), 3));
        assert!(due(Some(today.pred_opt().unwrap()), at(9), 3));
        assert!(!due(Some(today), at(23), 3));
    }

    #[tokio::test]
    async fn prune_keeps_the_newest() {
        let dir = std::env::temp_dir().join(format!("chef-backup-test-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        for d in 1..=5 {
            tokio::fs::write(dir.join(format!("chef_app_2026-01-0{d}_000000.dump")), b"x")
                .await
                .unwrap();
        }
        tokio::fs::write(dir.join("other.txt"), b"x").await.unwrap();
        prune(&dir, 2).await;
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "chef_app_2026-01-04_000000.dump",
                "chef_app_2026-01-05_000000.dump",
                "other.txt"
            ]
        );
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }
}
