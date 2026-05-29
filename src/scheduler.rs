use anyhow::Result;
use chrono::{Duration, Timelike, Utc};

use crate::config::AppConfig;
use crate::downloader;

/// Calculate the next UTC 13:15 time.
/// If the current time is past 13:15 UTC today, returns tomorrow's 13:15.
pub fn next_utc_13_15() -> chrono::DateTime<Utc> {
    let now = Utc::now();
    let mut target = now
        .with_hour(13)
        .unwrap()
        .with_minute(15)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();

    if target <= now {
        target += Duration::days(1);
    }

    target
}

/// Run the scheduler loop: sleep until next UTC 13:15, then execute downloads.
pub async fn run_scheduler(config: AppConfig) -> Result<()> {
    loop {
        let next_run = next_utc_13_15();
        let now = Utc::now();
        let wait_duration = (next_run - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(0));

        tracing::info!(
            "Next download at {} (in {:.1} hours)",
            next_run.format("%Y-%m-%d %H:%M:%S UTC"),
            wait_duration.as_secs_f64() / 3600.0
        );

        tokio::time::sleep(wait_duration).await;

        tracing::info!(
            "Triggered scheduled download at {}",
            Utc::now().format("%H:%M:%S UTC")
        );

        // Download yesterday's daily data for all intervals
        let yesterday = (Utc::now() - Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();

        for interval in &config.intervals {
            if let Err(e) = downloader::dump(&config, "daily", interval, &yesterday).await {
                tracing::error!("Error downloading interval {}: {:#}", interval, e);
            }
        }
    }
}
