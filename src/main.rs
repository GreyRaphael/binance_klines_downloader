mod config;
mod downloader;
mod notify;

use std::io::Write;

use anyhow::Result;
use chrono::{Datelike, Duration, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use config::AppConfig;
use notify::NotifyLayer;

#[derive(Debug, Clone, ValueEnum)]
enum Frequency {
    Daily,
    Monthly,
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Frequency::Daily => write!(f, "daily"),
            Frequency::Monthly => write!(f, "monthly"),
        }
    }
}

#[derive(Parser)]
#[command(name = "binance_klines_downloader", version, about = "Binance Futures kline downloader: https://github.com/GreyRaphael/binance_klines_downloader.git")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download yesterday's daily data for all intervals
    ///
    /// Designed for external scheduler (cron / Task Scheduler).
    /// Configure to run daily at UTC 13:15.
    /// Built-in retry: 5 attempts with 180s backoff per symbol.
    DailyScheduler,

    /// Download previous month's monthly data for all intervals
    ///
    /// Designed for external scheduler (cron / Task Scheduler).
    /// Configure to run on the 3rd of each month at UTC 00:00.
    /// Built-in retry: 5 attempts with 180s backoff per symbol.
    MonthlyScheduler,

    /// Manual download
    ///
    /// Downloads for all intervals defined in config.toml.
    Download {
        /// Frequency: daily or monthly
        #[arg(long, short = 'f', value_enum)]
        frequency: Frequency,

        /// Date to download (daily: 2026-05-26, monthly: 2026-04)
        #[arg(long, short = 'd')]
        date: String,
    },

    /// Backfill historical data by date range
    ///
    /// Downloads for all intervals defined in config.toml.
    /// Downloads monthly for complete months, daily for the last partial month.
    /// Example: -s 2022-10-03 -e 2025-03-10
    ///   monthly: 2022-10, 2022-11, ..., 2025-02
    ///   daily:   2025-03-01, ..., 2025-03-10
    Backfill {
        /// Start date (inclusive, e.g. 2022-10-03)
        #[arg(long, short = 's')]
        start: String,

        /// End date (inclusive, e.g. 2025-03-10)
        #[arg(long, short = 'e')]
        end: String,
    },
}

/// Get yesterday's date string (YYYY-MM-DD) in UTC.
fn yesterday_utc() -> String {
    (Utc::now() - Duration::days(1)).format("%Y-%m-%d").to_string()
}

/// Get previous month string (YYYY-MM) in UTC.
fn previous_month_utc() -> String {
    let now = Utc::now();
    let (y, m) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    format!("{}-{:02}", y, m)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load()?;

    let notify_layer = NotifyLayer::new(&config.gotify_url, &config.gotify_token, &config.ntfy_url, &config.ntfy_token);
    let notify_handles = notify_layer.handles();

    let file_writer: Box<dyn Write + Send + 'static> = if config.log_dir.is_empty() {
        Box::new(std::io::sink())
    } else {
        Box::new(tracing_appender::rolling::daily(&config.log_dir, "app.log"))
    };
    let (non_blocking, _file_guard) = tracing_appender::non_blocking(file_writer);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,binance_klines_downloader=debug")))
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(notify_layer)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::DailyScheduler => {
            let date = yesterday_utc();
            tracing::debug!("DailyScheduler: downloading {}", date);
            for interval in &config.intervals {
                if let Err(e) = downloader::dump(&config, "daily", interval, &date).await {
                    tracing::error!("Error downloading {}: {:#}", interval, e);
                    std::process::exit(1);
                }
            }
            tracing::info!("DailyScheduler completed");
        }
        Commands::MonthlyScheduler => {
            let month = previous_month_utc();
            tracing::debug!("MonthlyScheduler: downloading {}", month);
            for interval in &config.intervals {
                if let Err(e) = downloader::dump(&config, "monthly", interval, &month).await {
                    tracing::error!("Error downloading {}: {:#}", interval, e);
                    std::process::exit(1);
                }
            }
            tracing::info!("MonthlyScheduler completed");
        }
        Commands::Download { frequency, date } => {
            let freq = frequency.to_string();
            tracing::debug!("Manual download: frequency={}, date={}", freq, date);
            for interval in &config.intervals {
                if let Err(e) = downloader::dump(&config, &freq, interval, &date).await {
                    tracing::error!("Error downloading {}: {:#}", interval, e);
                    std::process::exit(1);
                }
            }
            tracing::info!("Download completed");
        }
        Commands::Backfill { start, end } => {
            tracing::debug!("Backfill: start={}, end={}", start, end);
            for interval in &config.intervals {
                if let Err(e) = downloader::backfill(&config, interval, &start, &end).await {
                    tracing::error!("Error backfilling {}: {:#}", interval, e);
                    std::process::exit(1);
                }
            }
            tracing::info!("Backfill completed");
        }
    }

    NotifyLayer::flush(&notify_handles).await;

    Ok(())
}
