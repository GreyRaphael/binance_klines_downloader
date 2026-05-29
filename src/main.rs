mod config;
mod downloader;
mod scheduler;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

use config::AppConfig;

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
#[command(name = "rust_downloader", about = "Binance Futures kline downloader")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the scheduler (daily at UTC 13:15)
    Scheduler,
    /// Manual download
    Download {
        /// Frequency: daily or monthly
        #[arg(long, value_enum)]
        frequency: Frequency,

        /// Kline interval: 5m, 15m, 30m, 1h
        #[arg(long)]
        interval: String,

        /// Date to download (daily: 2026-05-26, monthly: 2026-04)
        #[arg(long)]
        date: String,
    },

    /// Backfill historical data by date range
    ///
    /// Downloads monthly for complete months, daily for the last partial month.
    /// Example: --start 2022-10-03 --end 2025-03-10 --interval 1h
    ///   monthly: 2022-10, 2022-11, ..., 2025-02
    ///   daily:   2025-03-01, ..., 2025-03-10
    Backfill {
        /// Start date (inclusive, e.g. 2022-10-03)
        #[arg(long)]
        start: String,

        /// End date (inclusive, e.g. 2025-03-10)
        #[arg(long)]
        end: String,

        /// Kline interval: 5m, 15m, 30m, 1h
        #[arg(long)]
        interval: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = AppConfig::load()?;
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Scheduler) | None => {
            tracing::info!("Starting scheduler (next run at UTC 13:15)");
            scheduler::run_scheduler(config).await?;
        }
        Some(Commands::Download {
            frequency,
            interval,
            date,
        }) => {
            let freq = frequency.to_string();
            tracing::info!(
                "Manual download: frequency={}, interval={}, date={}",
                freq,
                interval,
                date
            );
            downloader::dump(&config, &freq, &interval, &date).await?;
            tracing::info!("Download completed");
        }
        Some(Commands::Backfill {
            start,
            end,
            interval,
        }) => {
            tracing::info!(
                "Backfill: start={}, end={}, interval={}",
                start,
                end,
                interval
            );
            downloader::backfill(&config, &interval, &start, &end).await?;
            tracing::info!("Backfill completed");
        }
    }

    Ok(())
}
