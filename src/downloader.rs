use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate};
use polars::prelude::*;
use reqwest::Client;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::config::AppConfig;

const KLINE_COLUMNS: [&str; 12] = [
    "open_time",
    "open",
    "high",
    "low",
    "close",
    "volume",
    "close_time",
    "quote_volume",
    "count",
    "taker_buy_volume",
    "taker_buy_quote_volume",
    "ignore",
];

const KLINE_DTYPES: [DataType; 12] = [
    DataType::Int64,   // open_time
    DataType::Float64, // open
    DataType::Float64, // high
    DataType::Float64, // low
    DataType::Float64, // close
    DataType::Float64, // volume
    DataType::Int64,   // close_time
    DataType::Float64, // quote_volume
    DataType::Int64,   // count
    DataType::Float64, // taker_buy_volume
    DataType::Float64, // taker_buy_quote_volume
    DataType::Int64,   // ignore
];

/// Delete daily IPC files for a given month after successful monthly download.
fn cleanup_daily_files(dir: &str, symbol: &str, interval: &str, month: &str) {
    // month format: "2025-05"
    // Daily file pattern: {symbol}-{interval}-2025-05-*.ipc
    let prefix = format!("{}-{}-{}-", symbol, interval, month);

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".ipc") {
                let path = entry.path();
                if let Err(e) = fs::remove_file(&path) {
                    tracing::warn!("Failed to delete {}: {:#}", path.display(), e);
                } else {
                    tracing::debug!("Deleted daily file: {}", path.display());
                }
            }
        }
    }
}

/// Download one symbol with retry, then save as IPC.
///
/// 404 is logged and returns Ok(()) without retry (data not yet published).
/// Network/server errors are retried up to 5 times with 180s backoff.
async fn download_one(
    client: &Client,
    symbol: &str,
    url: &str,
    output_path: &str,
    cleanup_monthly: Option<(&str, &str)>,
) -> Result<()> {
    for attempt in 1..=5u32 {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let zip_bytes = resp.bytes().await?.to_vec();

                // Extract & parse
                let cursor = Cursor::new(&zip_bytes);
                let mut archive =
                    ::zip::read::ZipArchive::new(cursor).context("Failed to open zip")?;
                let csv_bytes = {
                    let mut file = archive.by_index(0)?;
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)?;
                    buf
                };

                let csv_str = String::from_utf8(csv_bytes)?;
                let csv_content = if csv_str.starts_with("open_time") {
                    csv_str
                } else {
                    format!("{}\n{}", KLINE_COLUMNS.join(","), csv_str)
                };

                let dtypes = Arc::new(KLINE_DTYPES.to_vec());
                let cursor = Cursor::new(csv_content.into_bytes());
                let mut df = CsvReadOptions::default()
                    .with_has_header(true)
                    .with_dtype_overwrite(Some(dtypes))
                    .into_reader_with_file_handle(cursor)
                    .finish()?;

                df = df.sort(["open_time"], SortMultipleOptions::default())?;
                let _ = df.drop_in_place("ignore")?;
                let n = df.height();
                let sym = Series::new(PlSmallStr::from_str("symbol"), vec![symbol.to_string(); n]);
                df.with_column(sym.into())?;

                // Write IPC
                if let Some(parent) = Path::new(output_path).parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut file = File::create(output_path)?;
                IpcWriter::new(&mut file)
                    .with_compression(Some(IpcCompression::ZSTD(Default::default())))
                    .finish(&mut df)?;

                tracing::debug!("Saved {} ({} rows)", output_path, df.height());

                // If monthly download, clean up daily files for this month
                if let Some((interval, month)) = cleanup_monthly
                    && let Some(parent) = Path::new(output_path).parent()
                {
                    cleanup_daily_files(&parent.to_string_lossy(), symbol, interval, month);
                }

                return Ok(());
            }
            Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
                // 404: data not yet published, no point retrying
                tracing::warn!("{}: HTTP 404 — data not available yet, skipping", symbol);
                return Ok(());
            }
            Ok(resp) => {
                // Other HTTP errors (429, 5xx, etc.) — retry
                tracing::warn!(
                    "Attempt {}/5: HTTP {} for {}. Retrying in 180s...",
                    attempt,
                    resp.status(),
                    symbol
                );
            }
            Err(e) => {
                // Network error — retry
                tracing::warn!(
                    "Attempt {}/5: {} network error: {:#}. Retrying in 180s...",
                    attempt,
                    symbol,
                    e
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(tokio::time::Duration::from_secs(180)).await;
        }
    }
    tracing::error!("{}: failed after 5 attempts, giving up", symbol);
    anyhow::bail!("Failed to download {} after 5 attempts", symbol)
}

/// Download and save kline data for all symbols concurrently.
pub async fn dump(config: &AppConfig, frequency: &str, interval: &str, date: &str) -> Result<()> {
    let client = {
        tracing::debug!("Using proxy: {}", config.proxy);
        Client::builder()
            .proxy(reqwest::Proxy::all(&config.proxy)?)
            .build()?
    };

    tracing::debug!(
        "Starting {} download for interval={}, date={}, symbols={:?}",
        frequency,
        interval,
        date,
        config.symbols
    );

    let mut join_set = JoinSet::new();

    for symbol in &config.symbols {
        let client = client.clone();
        let symbol = symbol.clone();
        let frequency = frequency.to_string();
        let interval = interval.to_string();
        let date = date.to_string();
        let url = format!(
            "https://data.binance.vision/data/futures/um/{}/klines/{}/{}/{}-{}-{}.zip",
            frequency, symbol, interval, symbol, interval, date
        );
        let output_path = format!(
            "{}/{}/{}/{}-{}-{}.ipc",
            config.output_dir, interval, symbol, symbol, interval, date
        );

        join_set.spawn(async move {
            let cleanup = if frequency == "monthly" {
                Some((interval.as_str(), date.as_str()))
            } else {
                None
            };
            download_one(&client, &symbol, &url, &output_path, cleanup).await
        });
    }

    // Wait for all downloads to complete, propagate first error
    let mut first_error = None;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("Download failed: {:#}", e);
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                tracing::error!("Task panicked: {:#}", e);
                if first_error.is_none() {
                    first_error = Some(anyhow::anyhow!("Task panicked: {:#}", e));
                }
            }
        }
    }

    if let Some(e) = first_error {
        return Err(e);
    }

    tracing::info!("Completed {} download for interval={}", frequency, interval);
    Ok(())
}

/// Generate (frequency, date) tasks for a date range.
///
/// Complete months are downloaded as monthly, the last partial month as daily.
fn generate_tasks(start: NaiveDate, end: NaiveDate) -> Vec<(&'static str, String)> {
    let mut tasks = Vec::new();

    // Start from the 1st of the start month
    let mut current = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();

    while current <= end {
        // First day of next month
        let next_month = if current.month() == 12 {
            NaiveDate::from_ymd_opt(current.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(current.year(), current.month() + 1, 1).unwrap()
        };
        let last_day = next_month - Duration::days(1);

        // Is this the last month and it's partial?
        let is_partial_last =
            current.year() == end.year() && current.month() == end.month() && end < last_day;

        if is_partial_last {
            // Download daily from 1st to end
            let mut day = current;
            while day <= end {
                tasks.push(("daily", day.format("%Y-%m-%d").to_string()));
                day += Duration::days(1);
            }
        } else {
            // Download monthly
            tasks.push(("monthly", current.format("%Y-%m").to_string()));
        }

        current = next_month;
    }

    tasks
}

/// Backfill historical data by date range.
///
/// Downloads monthly for complete months, daily for the last partial month.
pub async fn backfill(config: &AppConfig, interval: &str, start: &str, end: &str) -> Result<()> {
    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d").context("Invalid start date")?;
    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d").context("Invalid end date")?;

    let tasks = generate_tasks(start_date, end_date);
    tracing::debug!(
        "Generated {} tasks for interval={}, symbols={:?}",
        tasks.len(),
        interval,
        config.symbols
    );

    let client = Client::builder()
        .proxy(reqwest::Proxy::all(&config.proxy)?)
        .build()?;

    let semaphore = Arc::new(Semaphore::new(16));
    let mut join_set = JoinSet::new();

    for symbol in &config.symbols {
        for (frequency, date) in &tasks {
            let client = client.clone();
            let symbol = symbol.clone();
            let frequency = frequency.to_string();
            let date = date.clone();
            let interval = interval.to_string();
            let sem = semaphore.clone();
            let url = format!(
                "https://data.binance.vision/data/futures/um/{}/klines/{}/{}/{}-{}-{}.zip",
                frequency, symbol, interval, symbol, interval, date
            );
            let output_path = format!(
                "{}/{}/{}/{}-{}-{}.ipc",
                config.output_dir, interval, symbol, symbol, interval, date
            );

            join_set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let cleanup = if frequency == "monthly" {
                    Some((interval.as_str(), date.as_str()))
                } else {
                    None
                };
                download_one(&client, &symbol, &url, &output_path, cleanup).await
            });
        }
    }

    let total = join_set.len();
    tracing::debug!("Spawned {} download tasks (max 16 concurrent)", total);

    let mut first_error = None;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("Download failed: {:#}", e);
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
            Err(e) => {
                tracing::error!("Task panicked: {:#}", e);
                if first_error.is_none() {
                    first_error = Some(anyhow::anyhow!("Task panicked: {:#}", e));
                }
            }
        }
    }

    if let Some(e) = first_error {
        return Err(e);
    }

    tracing::info!("Backfill completed");
    Ok(())
}
