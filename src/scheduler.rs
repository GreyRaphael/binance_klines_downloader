use anyhow::Result;
use chrono::{Datelike, Duration, Timelike, Utc};

use crate::config::AppConfig;
use crate::downloader;

/// Calculate the next UTC 13:15 time.
fn next_utc_13_15() -> chrono::DateTime<Utc> {
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

/// Calculate the next 3rd of month at UTC 00:00.
fn next_utc_monthly_3rd() -> chrono::DateTime<Utc> {
    let now = Utc::now();
    let target = now
        .with_day(3)
        .unwrap()
        .with_hour(0)
        .unwrap()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();

    if target <= now {
        // Move to next month's 3rd
        let (y, m) = if now.month() == 12 {
            (now.year() + 1, 1)
        } else {
            (now.year(), now.month() + 1)
        };
        chrono::NaiveDate::from_ymd_opt(y, m, 3)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    } else {
        target
    }
}

/// Get the previous month string (YYYY-MM) relative to now.
fn previous_month() -> String {
    let now = Utc::now();
    let (y, m) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    format!("{}-{:02}", y, m)
}

enum ScheduledTask {
    Daily,
    Monthly,
}

/// Run the scheduler loop.
///
/// - Daily at UTC 13:15: download yesterday's daily data
/// - 3rd of every month at UTC 00:00: download previous month's monthly data
pub async fn run_scheduler(config: AppConfig) -> Result<()> {
    loop {
        let now = Utc::now();
        let next_daily = next_utc_13_15();
        let next_monthly = next_utc_monthly_3rd();

        // Choose the sooner event
        let (next_run, task) = if next_monthly < next_daily {
            (next_monthly, ScheduledTask::Monthly)
        } else {
            (next_daily, ScheduledTask::Daily)
        };

        let wait_duration = (next_run - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(0));

        match &task {
            ScheduledTask::Daily => {
                tracing::info!(
                    "Next daily download at {} (in {:.1} hours)",
                    next_run.format("%Y-%m-%d %H:%M:%S UTC"),
                    wait_duration.as_secs_f64() / 3600.0
                );
            }
            ScheduledTask::Monthly => {
                tracing::info!(
                    "Next monthly download at {} (in {:.1} hours)",
                    next_run.format("%Y-%m-%d %H:%M:%S UTC"),
                    wait_duration.as_secs_f64() / 3600.0
                );
            }
        }

        tokio::time::sleep(wait_duration).await;

        tracing::info!(
            "Triggered scheduled task at {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        match task {
            ScheduledTask::Daily => {
                let yesterday = (Utc::now() - Duration::days(1))
                    .format("%Y-%m-%d")
                    .to_string();
                for interval in &config.intervals {
                    if let Err(e) = downloader::dump(&config, "daily", interval, &yesterday).await {
                        tracing::error!("Error downloading daily {}: {:#}", interval, e);
                    }
                }
            }
            ScheduledTask::Monthly => {
                let month = previous_month();
                for interval in &config.intervals {
                    if let Err(e) = downloader::dump(&config, "monthly", interval, &month).await {
                        tracing::error!("Error downloading monthly {}: {:#}", interval, e);
                    }
                }
            }
        }
    }
}
