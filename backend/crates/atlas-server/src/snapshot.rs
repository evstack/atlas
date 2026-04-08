use anyhow::{bail, Result};
use chrono::{DateTime, NaiveTime, Utc};
use std::time::Duration;

use crate::config::SnapshotConfig;

const SNAPSHOT_RETRY_DELAYS: &[u64] = &[5, 10, 20, 30, 60];
const SNAPSHOT_MAX_RETRY_DELAY: u64 = 60;

/// Calculate duration from `now` until the next occurrence of `target` time (UTC).
/// If `target` has already passed today, returns the duration until tomorrow's `target`.
fn duration_until_next(target: NaiveTime, now: DateTime<Utc>) -> Duration {
    let today_target = now.date_naive().and_time(target).and_utc();
    let next = if today_target > now {
        today_target
    } else {
        today_target + chrono::Duration::days(1)
    };
    (next - now).to_std().expect("positive duration")
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_secs(
        SNAPSHOT_RETRY_DELAYS
            .get(attempt)
            .copied()
            .unwrap_or(SNAPSHOT_MAX_RETRY_DELAY),
    )
}

fn sleep_duration(target: NaiveTime, now: DateTime<Utc>, retry_attempt: Option<usize>) -> Duration {
    retry_attempt
        .map(retry_delay)
        .unwrap_or_else(|| duration_until_next(target, now))
}

async fn attempt_snapshot(config: &SnapshotConfig) -> Result<()> {
    tokio::fs::create_dir_all(&config.dir).await?;

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%S");
    let filename = format!("atlas_snapshot_{timestamp}.dump");
    let tmp_path = format!("{}/{filename}.tmp", config.dir);
    let final_path = format!("{}/{filename}", config.dir);

    tracing::info!(%filename, "Starting database snapshot");

    let pg_config = crate::postgres_connection_config(&config.database_url)?;
    let status = crate::portable_pg_dump_command_async("pg_dump", &pg_config)
        .args(["--file", tmp_path.as_str()])
        .status()
        .await?;

    if status.success() {
        tokio::fs::rename(&tmp_path, &final_path).await?;
        tracing::info!(%filename, "Snapshot complete");
        cleanup_old_snapshots(&config.dir, config.retention).await;
        Ok(())
    } else {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        bail!("pg_dump failed with status: {status}");
    }
}

/// Run the snapshot scheduler loop.
/// Snapshot attempts retry with backoff within the same scheduled run so a
/// transient failure does not skip the day entirely.
pub async fn run_snapshot_loop(config: SnapshotConfig) -> Result<()> {
    tracing::info!(
        time = %config.time.format("%H:%M"),
        retention = config.retention,
        dir = %config.dir,
        "Snapshot scheduler started"
    );

    let mut retry_attempt = None;

    loop {
        let sleep_dur = sleep_duration(config.time, Utc::now(), retry_attempt);
        if let Some(attempt) = retry_attempt {
            tracing::warn!(
                attempt = attempt + 1,
                seconds = sleep_dur.as_secs(),
                "Retrying failed snapshot after backoff"
            );
        } else {
            tracing::info!(
                seconds = sleep_dur.as_secs(),
                "Sleeping until next snapshot"
            );
        }
        tokio::time::sleep(sleep_dur).await;

        match attempt_snapshot(&config).await {
            Ok(()) => retry_attempt = None,
            Err(err) => {
                let next_attempt = retry_attempt.map(|attempt| attempt + 1).unwrap_or(0);
                tracing::error!(
                    error = %err,
                    attempt = next_attempt + 1,
                    "Snapshot attempt failed"
                );
                retry_attempt = Some(next_attempt);
            }
        }
    }
}

/// Remove old snapshot files, keeping only the newest `retention` count.
async fn cleanup_old_snapshots(dir: &str, retention: u32) {
    let mut files = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("atlas_snapshot_") && name.ends_with(".dump") && !name.ends_with(".tmp")
        {
            files.push(entry.path());
        }
    }

    // Sort descending (newest first) — timestamp in filename gives lexicographic order
    files.sort();
    files.reverse();

    for old in files.into_iter().skip(retention as usize) {
        tracing::info!(path = %old.display(), "Removing old snapshot");
        if let Err(e) = tokio::fs::remove_file(&old).await {
            tracing::warn!(path = %old.display(), error = %e, "Failed to remove old snapshot");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn duration_until_next_target_in_future_today() {
        let now = Utc.with_ymd_and_hms(2026, 3, 19, 10, 0, 0).unwrap();
        let target = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        let dur = duration_until_next(target, now);
        assert_eq!(dur, Duration::from_secs(5 * 3600)); // 5 hours
    }

    #[test]
    fn duration_until_next_target_already_passed() {
        let now = Utc.with_ymd_and_hms(2026, 3, 19, 16, 0, 0).unwrap();
        let target = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let dur = duration_until_next(target, now);
        assert_eq!(dur, Duration::from_secs(11 * 3600)); // 11 hours until 03:00 next day
    }

    #[test]
    fn duration_until_next_target_exactly_now_wraps_to_tomorrow() {
        let now = Utc.with_ymd_and_hms(2026, 3, 19, 3, 0, 0).unwrap();
        let target = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let dur = duration_until_next(target, now);
        assert_eq!(dur, Duration::from_secs(24 * 3600)); // full 24 hours
    }

    #[test]
    fn sleep_duration_uses_schedule_when_not_retrying() {
        let now = Utc.with_ymd_and_hms(2026, 3, 19, 16, 0, 0).unwrap();
        let target = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let dur = sleep_duration(target, now, None);
        assert_eq!(dur, Duration::from_secs(11 * 3600));
    }

    #[test]
    fn sleep_duration_uses_retry_backoff_after_failure() {
        let now = Utc.with_ymd_and_hms(2026, 3, 19, 16, 0, 0).unwrap();
        let target = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let dur = sleep_duration(target, now, Some(0));
        assert_eq!(dur, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn cleanup_keeps_only_retention_count() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();

        // Create 5 snapshot files with different timestamps
        for i in 1..=5 {
            let path = dir
                .path()
                .join(format!("atlas_snapshot_2026-03-{i:02}T03-00-00.dump"));
            tokio::fs::write(&path, b"test").await.unwrap();
        }

        // Also create a .tmp file that should be ignored
        let tmp = dir
            .path()
            .join("atlas_snapshot_2026-03-06T03-00-00.dump.tmp");
        tokio::fs::write(&tmp, b"tmp").await.unwrap();

        cleanup_old_snapshots(dir_path, 3).await;

        let mut remaining = Vec::new();
        let mut entries = tokio::fs::read_dir(dir_path).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            remaining.push(entry.file_name().to_string_lossy().to_string());
        }
        remaining.sort();

        // Should keep 3 newest + the .tmp file
        assert_eq!(remaining.len(), 4);
        assert!(remaining.contains(&"atlas_snapshot_2026-03-03T03-00-00.dump".to_string()));
        assert!(remaining.contains(&"atlas_snapshot_2026-03-04T03-00-00.dump".to_string()));
        assert!(remaining.contains(&"atlas_snapshot_2026-03-05T03-00-00.dump".to_string()));
        assert!(remaining.contains(&"atlas_snapshot_2026-03-06T03-00-00.dump.tmp".to_string()));
    }

    #[tokio::test]
    async fn cleanup_noop_when_under_retention() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_str().unwrap();

        for i in 1..=2 {
            let path = dir
                .path()
                .join(format!("atlas_snapshot_2026-03-{i:02}T03-00-00.dump"));
            tokio::fs::write(&path, b"test").await.unwrap();
        }

        cleanup_old_snapshots(dir_path, 5).await;

        let mut count = 0;
        let mut entries = tokio::fs::read_dir(dir_path).await.unwrap();
        while entries.next_entry().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
