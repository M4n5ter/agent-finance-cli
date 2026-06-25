use anyhow::{Context, Result};
use serde_json::Value;

use crate::args::CryptoMarket;
use crate::http::{parse_optional_f64, timestamp_ms_to_utc};
use crate::model::{HistoryBatch, OhlcBar, Quote, SESSION_24H_PROXY};

use super::config::BinanceConfig;
use super::types::{FUTURES_PROVIDER, SPOT_PROVIDER};
use super::{futures_klines, futures_ticker, spot_klines, spot_ticker};

pub async fn fetch_quote(
    config: &BinanceConfig,
    market: CryptoMarket,
    symbol: &str,
) -> Result<Quote> {
    match market {
        CryptoMarket::Auto | CryptoMarket::Spot => spot_quote(config, symbol).await,
        CryptoMarket::UsdsFutures => futures_quote(config, symbol).await,
    }
}

pub async fn spot_quote(config: &BinanceConfig, symbol: &str) -> Result<Quote> {
    let report = spot_ticker(config, symbol).await?;
    let price = first_number(&report.payload, &["price"]).context("missing Binance spot price")?;
    Ok(Quote {
        symbol: report
            .symbol
            .clone()
            .unwrap_or_else(|| symbol.to_uppercase()),
        price,
        currency: Some("USDT".to_string()),
        provider: SPOT_PROVIDER.to_string(),
        session: Some("24h".to_string()),
        fetched_at_utc: report.fetched_at_utc,
        market_time: None,
        previous_close: None,
        open: None,
        high: None,
        low: None,
        volume: None,
        exchange: Some("Binance Spot".to_string()),
        provider_symbol: report.symbol,
        change_pct: None,
    })
}

pub async fn futures_quote(config: &BinanceConfig, symbol: &str) -> Result<Quote> {
    let report = futures_ticker(config, symbol).await?;
    let price =
        first_number(&report.payload, &["price"]).context("missing Binance futures price")?;
    Ok(Quote {
        symbol: report
            .symbol
            .clone()
            .unwrap_or_else(|| symbol.to_uppercase()),
        price,
        currency: Some("USDT".to_string()),
        provider: FUTURES_PROVIDER.to_string(),
        session: Some(SESSION_24H_PROXY.to_string()),
        fetched_at_utc: report.fetched_at_utc,
        market_time: report
            .payload
            .get("time")
            .and_then(Value::as_i64)
            .and_then(timestamp_ms_to_utc),
        previous_close: None,
        open: None,
        high: None,
        low: None,
        volume: None,
        exchange: Some("Binance USD-M Futures".to_string()),
        provider_symbol: report.symbol,
        change_pct: None,
    })
}

pub async fn fetch_history(
    config: &BinanceConfig,
    market: CryptoMarket,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    let report = match market {
        CryptoMarket::Auto | CryptoMarket::Spot => {
            spot_klines(config, symbol, interval, limit).await?
        }
        CryptoMarket::UsdsFutures => futures_klines(config, symbol, interval, limit).await?,
    };
    let bars = report
        .payload
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            kline_row_to_bar(
                report.symbol.as_deref().unwrap_or(symbol),
                &report.provider,
                row,
            )
        })
        .collect::<Vec<_>>();
    Ok(HistoryBatch {
        symbol: report.symbol.unwrap_or_else(|| symbol.to_uppercase()),
        provider: report.provider,
        interval: interval.to_string(),
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    })
}

fn kline_row_to_bar(symbol: &str, provider: &str, row: &Value) -> Option<OhlcBar> {
    let row = row.as_array()?;
    let open_time = value_i64(row.first())?;
    let close_time = row.get(6).and_then(|value| value_i64(Some(value)));
    let close = value_f64(row.get(4))?;
    Some(OhlcBar {
        symbol: symbol.to_uppercase(),
        provider: provider.to_string(),
        open_time: timestamp_ms_to_utc(open_time)?,
        close_time: close_time.and_then(timestamp_ms_to_utc),
        open: value_f64(row.get(1)),
        high: value_f64(row.get(2)),
        low: value_f64(row.get(3)),
        close,
        adj_close: None,
        volume: value_f64(row.get(5)),
        quote_volume: value_f64(row.get(7)),
        trades: row.get(8).and_then(Value::as_u64),
        dividend: None,
        stock_split: None,
        capital_gain: None,
        repaired: false,
    })
}

fn first_number(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(|value| value_f64(Some(value)))
        .or_else(|| {
            value.as_array().and_then(|values| {
                values
                    .first()
                    .and_then(|first| keys.iter().find_map(|key| first.get(*key)))
                    .and_then(|value| value_f64(Some(value)))
            })
        })
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::String(value) => value.parse().ok(),
        Value::Number(value) => value.as_i64(),
        _ => None,
    }
}

fn value_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::String(value) => parse_optional_f64(Some(value)),
        Value::Number(value) => value.as_f64(),
        _ => None,
    }
}
