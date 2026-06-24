use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, NaiveDate};
use scraper::{Html, Selector};
use serde::Deserialize;
use wreq::Client;
use zip::ZipArchive;

use crate::cli::{StooqAsset, StooqFrequency, StooqMarket};
use crate::http::{clean_text, parse_optional_f64, parse_optional_u64, utc_now};
use crate::model::{HistoryBatch, OhlcBar, Quote, StooqSyncReport};

#[path = "stooq/catalog.rs"]
mod catalog;

pub use catalog::catalog;
use catalog::{cached_zip_path, catalog_package};

#[derive(Debug, Deserialize)]
struct StooqQuoteRow {
    #[serde(rename = "Date")]
    date: Option<String>,
    #[serde(rename = "Time")]
    time: Option<String>,
    #[serde(rename = "Open")]
    open: Option<String>,
    #[serde(rename = "High")]
    high: Option<String>,
    #[serde(rename = "Low")]
    low: Option<String>,
    #[serde(rename = "Close")]
    close: Option<String>,
    #[serde(rename = "Volume")]
    volume: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StooqHistoryRow {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Open")]
    open: Option<String>,
    #[serde(rename = "High")]
    high: Option<String>,
    #[serde(rename = "Low")]
    low: Option<String>,
    #[serde(rename = "Close")]
    close: Option<String>,
    #[serde(rename = "Volume")]
    volume: Option<String>,
}

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let provider_symbol = stooq_symbol(symbol);
    let url = format!("https://stooq.com/q/l/?s={provider_symbol}&f=sd2t2ohlcv&h&e=csv");
    let text = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .context("Stooq request failed")?
        .error_for_status()
        .context("Stooq returned HTTP error")?
        .text()
        .await
        .context("Stooq response text parse failed")?;
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let row: StooqQuoteRow = reader
        .deserialize()
        .next()
        .ok_or_else(|| anyhow!("Stooq response missing rows"))?
        .context("Stooq CSV parse failed")?;
    let price = parse_optional_f64(row.close.as_deref())
        .ok_or_else(|| anyhow!("Stooq response missing usable close"))?;
    let market_time = match (
        clean_text(row.date.as_deref()),
        clean_text(row.time.as_deref()),
    ) {
        (Some(date), Some(time)) => Some(format!("{date} {time}")),
        (Some(date), None) => Some(date.to_string()),
        _ => None,
    };

    Ok(Quote {
        symbol: symbol.to_uppercase(),
        price,
        currency: provider_symbol.ends_with(".us").then(|| "USD".to_string()),
        provider: "stooq".to_string(),
        session: Some("regular".to_string()),
        fetched_at_utc: utc_now(),
        market_time,
        previous_close: None,
        open: parse_optional_f64(row.open.as_deref()),
        high: parse_optional_f64(row.high.as_deref()),
        low: parse_optional_f64(row.low.as_deref()),
        volume: parse_optional_u64(row.volume.as_deref()),
        exchange: None,
        provider_symbol: Some(provider_symbol),
        change_pct: None,
    })
}

pub async fn fetch_history(
    client: &Client,
    symbol: &str,
    interval: &str,
    limit: usize,
    stooq_market: StooqMarket,
    stooq_asset: StooqAsset,
) -> Result<HistoryBatch> {
    if matches!(interval, "5m" | "5minute" | "1h" | "hour" | "60m") {
        return fetch_bulk_history(symbol, interval, limit, stooq_market, stooq_asset);
    }

    let provider_symbol = stooq_symbol(symbol);
    let interval_code = stooq_interval(interval)?;
    let api_key = std::env::var("STOOQ_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let bars = match api_key.as_deref() {
        Some(api_key) => match fetch_live_csv_history(
            client,
            symbol,
            &provider_symbol,
            interval_code,
            limit,
            Some(api_key),
        )
        .await
        {
            Ok(bars) => bars,
            Err(error) if is_stooq_csv_auth_error(&error) => {
                fetch_live_html_history(client, symbol, &provider_symbol, interval_code, limit)
                    .await?
            }
            Err(error) => return Err(error),
        },
        None => {
            fetch_live_html_history(client, symbol, &provider_symbol, interval_code, limit).await?
        }
    };

    Ok(live_history_batch(symbol, interval, bars))
}

pub struct StooqSyncRequest {
    pub frequency: StooqFrequency,
    pub market: StooqMarket,
    pub asset: StooqAsset,
    pub url: Option<String>,
    pub zip_path: Option<PathBuf>,
    pub force: bool,
}

pub async fn sync_bulk(client: &Client, request: StooqSyncRequest) -> Result<StooqSyncReport> {
    let package = catalog_package(request.frequency, request.market, request.asset)
        .ok_or_else(|| anyhow!("Stooq catalog does not list this frequency/market/asset combo"))?;
    let cache_key = package.cache_key();
    let target = cached_zip_path(&cache_key)?;
    if target.exists() && !request.force {
        return Err(anyhow!(
            "Stooq cache already exists at {}; pass --force to overwrite",
            target.display()
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Stooq cache directory {}",
                parent.display()
            )
        })?;
    }
    let temp = target.with_extension("zip.tmp");
    if temp.exists() {
        fs::remove_file(&temp)
            .with_context(|| format!("failed to remove stale temp ZIP {}", temp.display()))?;
    }

    let (bytes, source) = match (request.url.as_deref(), request.zip_path.as_deref()) {
        (Some(url), None) => {
            let bytes = download_zip_to_file(client, url, &temp).await?;
            (bytes, url.to_string())
        }
        (None, Some(path)) => (
            fs::copy(path, &temp).with_context(|| {
                format!(
                    "failed to copy Stooq ZIP {} to {}",
                    path.display(),
                    temp.display()
                )
            })?,
            path.display().to_string(),
        ),
        (Some(_), Some(_)) => {
            return Err(anyhow!("pass either --url or --zip-path, not both"));
        }
        (None, None) => {
            return Err(anyhow!(
                "Stooq bulk downloads require a captcha-authorized URL or local ZIP; run `agent-finance market stooq catalog` for package listings"
            ));
        }
    };
    validate_zip_file(&temp)?;
    if target.exists() {
        fs::remove_file(&target).with_context(|| {
            format!("failed to remove old Stooq cache ZIP {}", target.display())
        })?;
    }
    fs::rename(&temp, &target).with_context(|| {
        format!(
            "failed to move Stooq cache ZIP {} to {}",
            temp.display(),
            target.display()
        )
    })?;

    Ok(StooqSyncReport {
        provider: "stooq".to_string(),
        frequency: request.frequency.label().to_string(),
        market: request.market.label().to_string(),
        asset: request.asset.label().to_string(),
        cache_key,
        zip_path: target.display().to_string(),
        bytes,
        imported_at_utc: utc_now(),
        source,
    })
}

async fn fetch_live_csv_history(
    client: &Client,
    symbol: &str,
    provider_symbol: &str,
    interval_code: &str,
    limit: usize,
    api_key: Option<&str>,
) -> Result<Vec<OhlcBar>> {
    let url = stooq_history_url(provider_symbol, interval_code, api_key);
    let text = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .context("Stooq history request failed")?
        .error_for_status()
        .context("Stooq history returned HTTP error")?
        .text()
        .await
        .context("Stooq history response text parse failed")?;
    if is_stooq_csv_auth_challenge(&text) {
        return Err(anyhow!(
            "Stooq CSV history requires a captcha-issued API key; set STOOQ_API_KEY or use the no-key HTML table fallback"
        ));
    }
    parse_csv_history_bars(symbol, &text, limit)
}

fn parse_csv_history_bars(symbol: &str, text: &str, limit: usize) -> Result<Vec<OhlcBar>> {
    let mut reader = csv::Reader::from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for row in reader.deserialize::<StooqHistoryRow>() {
        let row = row.context("Stooq history CSV parse failed")?;
        let Some(close) = parse_optional_f64(row.close.as_deref()) else {
            continue;
        };
        rows.push(OhlcBar {
            symbol: symbol.to_uppercase(),
            provider: "stooq".to_string(),
            open_time: row.date,
            close_time: None,
            open: parse_optional_f64(row.open.as_deref()),
            high: parse_optional_f64(row.high.as_deref()),
            low: parse_optional_f64(row.low.as_deref()),
            close,
            adj_close: None,
            volume: parse_optional_f64(row.volume.as_deref()),
            quote_volume: None,
            trades: None,
            dividend: None,
            stock_split: None,
            capital_gain: None,
            repaired: false,
        });
    }
    let bars = rows
        .into_iter()
        .rev()
        .take(limit.max(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(bars)
}

async fn fetch_live_html_history(
    client: &Client,
    symbol: &str,
    provider_symbol: &str,
    interval_code: &str,
    limit: usize,
) -> Result<Vec<OhlcBar>> {
    let limit = limit.max(1);
    let daily_limit = stooq_daily_limit_for_html_fallback(interval_code, limit);
    let daily_bars = fetch_live_html_daily_history(client, symbol, provider_symbol, daily_limit)
        .await
        .context("Stooq no-key HTML history fallback failed")?;
    match interval_code {
        "d" => Ok(take_latest_chronological(daily_bars, limit)),
        "w" => aggregate_daily_bars(daily_bars, limit, AggregationPeriod::Week),
        "m" => aggregate_daily_bars(daily_bars, limit, AggregationPeriod::Month),
        _ => Err(anyhow!("unsupported Stooq HTML fallback interval")),
    }
}

async fn fetch_live_html_daily_history(
    client: &Client,
    symbol: &str,
    provider_symbol: &str,
    limit: usize,
) -> Result<Vec<OhlcBar>> {
    let mut newest_first = Vec::new();
    for page in 1..=stooq_html_page_cap(limit) {
        let url = stooq_history_page_url(provider_symbol, page);
        let text = client
            .get(url)
            .header("Accept-Encoding", "identity")
            .send()
            .await
            .context("Stooq history HTML request failed")?
            .error_for_status()
            .context("Stooq history HTML returned HTTP error")?
            .text()
            .await
            .context("Stooq history HTML response text parse failed")?;
        let mut page_bars = parse_html_history_page(symbol, &text)?;
        if page_bars.is_empty() {
            break;
        }
        newest_first.append(&mut page_bars);
        if newest_first.len() >= limit || !html_has_next_page(&text, page) {
            break;
        }
    }
    if newest_first.is_empty() {
        return Err(anyhow!(
            "Stooq history HTML page did not contain usable OHLC rows"
        ));
    }
    let mut bars = newest_first.into_iter().take(limit).collect::<Vec<_>>();
    bars.reverse();
    Ok(bars)
}

fn parse_html_history_page(symbol: &str, text: &str) -> Result<Vec<OhlcBar>> {
    if is_stooq_html_rate_limited(text) {
        return Err(anyhow!(
            "Stooq no-key HTML history hit the daily site hits limit; set STOOQ_API_KEY for CSV history or import bulk history with `agent-finance market stooq sync`"
        ));
    }
    let document = Html::parse_document(text);
    let row_selector = Selector::parse("table#fth1 tbody tr").expect("valid Stooq row selector");
    let cell_selector = Selector::parse("td").expect("valid Stooq cell selector");
    let bars = document
        .select(&row_selector)
        .filter_map(|row| {
            let cells = row
                .select(&cell_selector)
                .map(|cell| normalize_html_text(cell.text()))
                .collect::<Vec<_>>();
            html_cells_to_bar(symbol, &cells).transpose()
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(bars)
}

fn html_cells_to_bar(symbol: &str, cells: &[String]) -> Result<Option<OhlcBar>> {
    if cells.len() < 6 || cells[1].eq_ignore_ascii_case("date") {
        return Ok(None);
    }
    let Some(close) = parse_stooq_number(cells.get(5).map(String::as_str)) else {
        return Ok(None);
    };
    Ok(Some(OhlcBar {
        symbol: symbol.to_uppercase(),
        provider: "stooq".to_string(),
        open_time: parse_stooq_html_date(&cells[1])?,
        close_time: None,
        open: parse_stooq_number(cells.get(2).map(String::as_str)),
        high: parse_stooq_number(cells.get(3).map(String::as_str)),
        low: parse_stooq_number(cells.get(4).map(String::as_str)),
        close,
        adj_close: None,
        volume: parse_stooq_number(cells.last().map(String::as_str)),
        quote_volume: None,
        trades: None,
        dividend: None,
        stock_split: None,
        capital_gain: None,
        repaired: false,
    }))
}

fn parse_stooq_html_date(value: &str) -> Result<String> {
    let value = normalize_space(value);
    for format in ["%e %b %Y", "%d %b %Y", "%Y-%m-%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(&value, format) {
            return Ok(date.format("%Y-%m-%d").to_string());
        }
    }
    Err(anyhow!(
        "Stooq history HTML row has unsupported date: {value}"
    ))
}

fn parse_stooq_number(value: Option<&str>) -> Option<f64> {
    let value = clean_text(value)?;
    value.replace(',', "").parse::<f64>().ok()
}

fn normalize_html_text<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    normalize_space(&parts.collect::<Vec<_>>().join(" "))
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn html_has_next_page(text: &str, page: usize) -> bool {
    let document = Html::parse_document(text);
    let selector = Selector::parse("a[href]").expect("valid link selector");
    let next = format!("l={}", page + 1);
    document.select(&selector).any(|element| {
        element
            .value()
            .attr("href")
            .is_some_and(|href| href.contains("q/d/?") && href.contains(&next))
    })
}

fn stooq_daily_limit_for_html_fallback(interval_code: &str, limit: usize) -> usize {
    match interval_code {
        "w" => limit.max(1) * 7 + 7,
        "m" => limit.max(1) * 31 + 31,
        _ => limit.max(1),
    }
}

fn stooq_html_page_cap(limit: usize) -> usize {
    (limit.max(1).div_ceil(40) + 1).min(120)
}

fn live_history_batch(symbol: &str, interval: &str, bars: Vec<OhlcBar>) -> HistoryBatch {
    HistoryBatch {
        symbol: symbol.to_uppercase(),
        provider: "stooq".to_string(),
        interval: interval.to_string(),
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    }
}

fn is_stooq_csv_auth_error(error: &anyhow::Error) -> bool {
    is_stooq_csv_auth_challenge(&error.to_string())
}

fn is_stooq_csv_auth_challenge(text: &str) -> bool {
    text.contains("Get your apikey") || text.contains("get_apikey")
}

fn is_stooq_html_rate_limited(text: &str) -> bool {
    text.contains("Exceeded the daily site hits limit") || text.contains("The data has been hidden")
}

#[derive(Clone, Copy)]
enum AggregationPeriod {
    Week,
    Month,
}

fn aggregate_daily_bars(
    daily_bars: Vec<OhlcBar>,
    limit: usize,
    period: AggregationPeriod,
) -> Result<Vec<OhlcBar>> {
    let mut aggregates = Vec::new();
    let mut current_key = None::<String>;
    let mut current_bar = None::<OhlcBar>;

    for bar in daily_bars {
        let key = aggregation_key(&bar.open_time, period)?;
        if current_key.as_deref() != Some(key.as_str()) {
            if let Some(bar) = current_bar.take() {
                aggregates.push(bar);
            }
            current_key = Some(key);
            current_bar = Some(bar);
            continue;
        }

        if let Some(current) = current_bar.as_mut() {
            current.close_time = Some(bar.open_time);
            current.high = max_optional(current.high, bar.high);
            current.low = min_optional(current.low, bar.low);
            current.close = bar.close;
            current.volume = sum_optional(current.volume, bar.volume);
        }
    }
    if let Some(bar) = current_bar {
        aggregates.push(bar);
    }
    Ok(take_latest_chronological(aggregates, limit.max(1)))
}

fn aggregation_key(date: &str, period: AggregationPeriod) -> Result<String> {
    let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("Stooq aggregate fallback cannot parse date: {date}"))?;
    Ok(match period {
        AggregationPeriod::Week => {
            let week = date.iso_week();
            format!("{}-{:02}", week.year(), week.week())
        }
        AggregationPeriod::Month => format!("{}-{:02}", date.year(), date.month()),
    })
}

fn take_latest_chronological<T>(items: Vec<T>, limit: usize) -> Vec<T> {
    let len = items.len();
    items.into_iter().skip(len.saturating_sub(limit)).collect()
}

fn max_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn min_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn sum_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

async fn download_zip_to_file(client: &Client, url: &str, target: &Path) -> Result<u64> {
    let response = client
        .get(url)
        .send()
        .await
        .context("Stooq bulk download request failed")?
        .error_for_status()
        .context("Stooq bulk download returned HTTP error")?;
    let mut file = fs::File::create(target)
        .with_context(|| format!("failed to create temp ZIP {}", target.display()))?;
    let bytes = response
        .bytes()
        .await
        .context("Stooq bulk download body read failed")?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write temp ZIP {}", target.display()))?;
    Ok(bytes.len() as u64)
}

fn stooq_symbol(symbol: &str) -> String {
    let mut normalized = symbol.trim().to_lowercase().replace('-', ".");
    if !normalized.contains('.') {
        normalized.push_str(".us");
    }
    normalized
}

fn stooq_interval(interval: &str) -> Result<&'static str> {
    match interval {
        "1d" | "d" => Ok("d"),
        "1w" | "w" => Ok("w"),
        "1mo" | "1M" | "m" => Ok("m"),
        _ => Err(anyhow!(
            "Stooq history supports daily/weekly/monthly intervals only"
        )),
    }
}

fn stooq_history_url(provider_symbol: &str, interval_code: &str, api_key: Option<&str>) -> String {
    let mut url = format!("https://stooq.com/q/d/l/?s={provider_symbol}&i={interval_code}");
    if let Some(api_key) = api_key.map(str::trim).filter(|value| !value.is_empty()) {
        url.push_str("&apikey=");
        url.push_str(api_key);
    }
    url
}

fn stooq_history_page_url(provider_symbol: &str, page: usize) -> String {
    format!("https://stooq.com/q/d/?s={provider_symbol}&i=d&l={page}")
}

fn fetch_bulk_history(
    symbol: &str,
    interval: &str,
    limit: usize,
    market: StooqMarket,
    asset: StooqAsset,
) -> Result<HistoryBatch> {
    let frequency = match interval {
        "5m" | "5minute" => StooqFrequency::FiveMin,
        "1h" | "hour" | "60m" => StooqFrequency::Hourly,
        _ => return Err(anyhow!("Stooq bulk supports 5m and 1h intervals")),
    };
    let package = catalog_package(frequency, market, asset)
        .ok_or_else(|| anyhow!("Stooq catalog does not list this frequency/market/asset combo"))?;
    let cache_key = package.cache_key();
    let zip_path = cached_zip_path(&cache_key)?;
    if !zip_path.exists() {
        return Err(anyhow!(
            "no Stooq {} bulk ZIP is cached at {}; import it with `agent-finance market stooq sync --frequency {} --market {} --asset {} --zip-path <file>`",
            package.label,
            zip_path.display(),
            frequency.label(),
            market.label(),
            asset.label()
        ));
    }

    let normalized = stooq_symbol(symbol);
    if let Some(history) = read_symbol_from_zip(&zip_path, symbol, &normalized, interval, limit)? {
        return Ok(history);
    }
    Err(anyhow!(
        "symbol {} was not found in cached Stooq {} ZIP {}",
        normalized,
        package.label,
        zip_path.display()
    ))
}

fn validate_zip_file(path: &Path) -> Result<()> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open Stooq ZIP {}", path.display()))?;
    ZipArchive::new(file).context("Stooq bulk payload is not a readable ZIP")?;
    Ok(())
}

fn read_symbol_from_zip(
    path: &Path,
    symbol: &str,
    provider_symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<Option<HistoryBatch>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open Stooq cache ZIP {}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read Stooq cache ZIP {}", path.display()))?;
    let symbol_file = provider_symbol.to_ascii_lowercase();
    let plain_file = symbol_file.trim_end_matches(".us").to_string();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("failed to read Stooq ZIP entry {index}"))?;
        if !file.is_file() {
            continue;
        }
        let entry_name = file.name().to_ascii_lowercase();
        let Some(file_name) = entry_name.rsplit('/').next() else {
            continue;
        };
        if !matches_symbol_file(file_name, &symbol_file, &plain_file) {
            continue;
        }
        let bars = parse_bulk_bars_from_reader(symbol, &mut file, limit)?;
        return Ok(Some(HistoryBatch {
            symbol: symbol.to_uppercase(),
            provider: "stooq-bulk".to_string(),
            interval: interval.to_string(),
            adjustment: "raw".to_string(),
            actions_included: false,
            repair_requested: false,
            repair_applied: false,
            bars,
        }));
    }
    Ok(None)
}

fn matches_symbol_file(file_name: &str, symbol_file: &str, plain_file: &str) -> bool {
    let file_name = file_name.trim_end_matches(".txt").trim_end_matches(".csv");
    file_name == symbol_file || file_name == plain_file
}

fn parse_bulk_bars_from_reader<R: Read>(
    symbol: &str,
    reader: R,
    limit: usize,
) -> Result<Vec<OhlcBar>> {
    let limit = limit.max(1);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(reader);
    let headers = reader
        .headers()
        .context("Stooq bulk CSV missing headers")?
        .clone();
    let mut bars = VecDeque::with_capacity(limit);
    for record in reader.records() {
        let record = record.context("Stooq bulk CSV record parse failed")?;
        if let Some(bar) = record_to_bar(symbol, &headers, &record) {
            if bars.len() == limit {
                bars.pop_front();
            }
            bars.push_back(bar);
        }
    }
    if bars.is_empty() {
        return Err(anyhow!("Stooq bulk CSV did not contain usable OHLC rows"));
    }
    Ok(bars.into_iter().collect())
}

fn record_to_bar(
    symbol: &str,
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
) -> Option<OhlcBar> {
    let date = field(headers, record, &["Date", "DATE", "<DATE>"])?;
    let time = field(headers, record, &["Time", "TIME", "<TIME>"]);
    let open_time = match time {
        Some(time) if !time.trim().is_empty() => format!("{date} {time}"),
        _ => date.to_string(),
    };
    let close = parse_optional_f64(field(headers, record, &["Close", "CLOSE", "<CLOSE>"]))?;
    Some(OhlcBar {
        symbol: symbol.to_uppercase(),
        provider: "stooq-bulk".to_string(),
        open_time,
        close_time: None,
        open: parse_optional_f64(field(headers, record, &["Open", "OPEN", "<OPEN>"])),
        high: parse_optional_f64(field(headers, record, &["High", "HIGH", "<HIGH>"])),
        low: parse_optional_f64(field(headers, record, &["Low", "LOW", "<LOW>"])),
        close,
        adj_close: None,
        volume: parse_optional_f64(field(headers, record, &["Volume", "VOL", "<VOL>"])),
        quote_volume: None,
        trades: None,
        dividend: None,
        stock_split: None,
        capital_gain: None,
        repaired: false,
    })
}

fn field<'a>(
    headers: &csv::StringRecord,
    record: &'a csv::StringRecord,
    names: &[&str],
) -> Option<&'a str> {
    names.iter().find_map(|name| {
        headers
            .iter()
            .position(|header| header.eq_ignore_ascii_case(name))
            .and_then(|index| record.get(index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML_HISTORY_PAGE: &str = r#"
<html>
  <body>
    <a href="q/d/?s=aapl.us&i=d&l=2">&gt;</a>
    <table class="fth1" id="fth1">
      <thead>
        <tr><td>No.</td><td>Date</td><td>Open</td><td>High</td><td>Low</td><td>Close</td><td colspan="2">Change</td><td>Volume</td></tr>
      </thead>
      <tbody>
        <tr><td>10515</td><td>3 Jun 2026</td><td>314.175</td><td>316.94</td><td>308.85</td><td>310.26</td><td>-1.57%</td><td>-4.940</td><td>50,836,705</td></tr>
        <tr><td>10514</td><td>2 Jun 2026</td><td>307.46</td><td>315.45</td><td>306.685</td><td>315.2</td><td>+2.90%</td><td>+8.890</td><td>44,534,716</td></tr>
        <tr><td>10513</td><td>1 Jun 2026</td><td>309.625</td><td>310.94</td><td>305.02</td><td>306.31</td><td>-1.84%</td><td>-5.750</td><td>48,849,933</td></tr>
        <tr><td>10512</td><td>29 May 2026</td><td>311.775</td><td>315</td><td>309.53</td><td>312.06</td><td>-0.14%</td><td>-0.450</td><td>70,026,752</td></tr>
      </tbody>
    </table>
  </body>
</html>
"#;

    #[test]
    fn parses_no_key_stooq_html_history_table() {
        let bars = parse_html_history_page("AAPL", HTML_HISTORY_PAGE).expect("html history");

        assert_eq!(bars.len(), 4);
        assert_eq!(bars[0].open_time, "2026-06-03");
        assert_eq!(bars[0].open, Some(314.175));
        assert_eq!(bars[0].high, Some(316.94));
        assert_eq!(bars[0].low, Some(308.85));
        assert_eq!(bars[0].close, 310.26);
        assert_eq!(bars[0].volume, Some(50_836_705.0));
        assert!(html_has_next_page(HTML_HISTORY_PAGE, 1));
    }

    #[test]
    fn reports_stooq_html_daily_site_limit() {
        let html = "<b>Exceeded the daily site hits limit<br>The data has been hidden</b>";

        let error = parse_html_history_page("AAPL", html).expect_err("rate-limited page");

        assert!(error.to_string().contains("daily site hits limit"));
    }

    #[test]
    fn aggregates_no_key_daily_rows_when_stooq_html_ignores_interval() {
        let newest_first =
            parse_html_history_page("AAPL", HTML_HISTORY_PAGE).expect("html history");
        let mut chronological = newest_first;
        chronological.reverse();

        let monthly =
            aggregate_daily_bars(chronological, 1, AggregationPeriod::Month).expect("monthly bars");

        assert_eq!(monthly.len(), 1);
        assert_eq!(monthly[0].open_time, "2026-06-01");
        assert_eq!(monthly[0].close_time.as_deref(), Some("2026-06-03"));
        assert_eq!(monthly[0].open, Some(309.625));
        assert_eq!(monthly[0].high, Some(316.94));
        assert_eq!(monthly[0].low, Some(305.02));
        assert_eq!(monthly[0].close, 310.26);
        assert_eq!(monthly[0].volume, Some(144_221_354.0));
    }

    #[test]
    fn builds_stooq_urls_for_csv_key_and_html_table_paths() {
        assert_eq!(
            stooq_history_url("aapl.us", "d", None),
            "https://stooq.com/q/d/l/?s=aapl.us&i=d"
        );
        assert_eq!(
            stooq_history_url("aapl.us", "w", Some(" token ")),
            "https://stooq.com/q/d/l/?s=aapl.us&i=w&apikey=token"
        );
        assert_eq!(
            stooq_history_page_url("aapl.us", 3),
            "https://stooq.com/q/d/?s=aapl.us&i=d&l=3"
        );
    }

    #[test]
    fn parses_stooq_bulk_angle_headers_and_keeps_latest_limit() {
        let text = "\
<TICKER>,<PER>,<DATE>,<TIME>,<OPEN>,<HIGH>,<LOW>,<CLOSE>,<VOL>,<OPENINT>
AAPL.US,5,20260603,154500,312.21,312.36,311.91,311.95,5866,0
AAPL.US,5,20260603,155000,311.75,312.00,311.75,311.84,1482,0
";
        let bars = parse_bulk_bars_from_reader("AAPL", text.as_bytes(), 1).expect("bulk bars");

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].symbol, "AAPL");
        assert_eq!(bars[0].provider, "stooq-bulk");
        assert_eq!(bars[0].open_time, "20260603 155000");
        assert_eq!(bars[0].open, Some(311.75));
        assert_eq!(bars[0].high, Some(312.0));
        assert_eq!(bars[0].low, Some(311.75));
        assert_eq!(bars[0].close, 311.84);
        assert_eq!(bars[0].volume, Some(1482.0));
    }

    #[test]
    fn stooq_bulk_cache_key_uses_full_package_scope() {
        let stocks = catalog_package(StooqFrequency::FiveMin, StooqMarket::Us, StooqAsset::Stocks)
            .expect("us stocks 5m package");
        let etfs = catalog_package(StooqFrequency::FiveMin, StooqMarket::Us, StooqAsset::Etfs)
            .expect("us etfs 5m package");

        assert_eq!(stocks.cache_key(), "5m_us_stocks");
        assert_eq!(etfs.cache_key(), "5m_us_etfs");
        assert_ne!(stocks.cache_key(), etfs.cache_key());
        assert!(
            catalog_package(
                StooqFrequency::FiveMin,
                StooqMarket::Macro,
                StooqAsset::Macro
            )
            .is_none()
        );
    }
}
