use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

use crate::args::FuturesPeriod;
use crate::http::utc_now;

use super::config::BinanceConfig;
use super::symbol::normalize_symbol;
use super::types::{CryptoEndpointReport, CryptoSentimentReport, CryptoSnapshotReport};
use super::{
    futures_24h, futures_basis, futures_flow, futures_funding, futures_mark, futures_open_interest,
    futures_ratios, futures_ticker, spot_24h, spot_book_ticker, spot_klines, spot_ticker,
    spot_trades,
};

pub async fn snapshot(config: &BinanceConfig, symbol: &str) -> CryptoSnapshotReport {
    let symbol = normalize_symbol(symbol).unwrap_or_else(|_| symbol.to_uppercase());
    let fetched_at_utc = utc_now();
    let mut spot = BTreeMap::new();
    let mut futures = BTreeMap::new();
    let mut errors = BTreeMap::new();

    push_report(
        &mut spot,
        &mut errors,
        "spot.ticker",
        spot_ticker(config, &symbol).await,
    );
    push_report(
        &mut spot,
        &mut errors,
        "spot.24h",
        spot_24h(config, &symbol).await,
    );
    push_report(
        &mut spot,
        &mut errors,
        "spot.book_ticker",
        spot_book_ticker(config, &symbol).await,
    );
    push_report(
        &mut spot,
        &mut errors,
        "spot.trades",
        spot_trades(config, &symbol, 10, true).await,
    );
    push_report(
        &mut spot,
        &mut errors,
        "spot.klines",
        spot_klines(config, &symbol, "1h", 24).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "futures.ticker",
        futures_ticker(config, &symbol).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "futures.24h",
        futures_24h(config, &symbol).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "futures.mark",
        futures_mark(config, &symbol).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "futures.open_interest",
        futures_open_interest(config, &symbol).await,
    );

    CryptoSnapshotReport {
        symbol,
        provider: "binance".to_string(),
        fetched_at_utc,
        spot,
        futures,
        errors,
    }
}

pub async fn sentiment(config: &BinanceConfig, symbol: &str) -> CryptoSentimentReport {
    let symbol = normalize_symbol(symbol).unwrap_or_else(|_| symbol.to_uppercase());
    let fetched_at_utc = utc_now();
    let mut futures = BTreeMap::new();
    let mut errors = BTreeMap::new();

    push_report(
        &mut futures,
        &mut errors,
        "mark",
        futures_mark(config, &symbol).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "funding",
        futures_funding(config, &symbol, 8).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "open_interest",
        futures_open_interest(config, &symbol).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "ratios",
        futures_ratios(config, &symbol, FuturesPeriod::FiveMin, 30).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "flow",
        futures_flow(config, &symbol, FuturesPeriod::FiveMin, 30).await,
    );
    push_report(
        &mut futures,
        &mut errors,
        "basis",
        futures_basis(config, &symbol, FuturesPeriod::FiveMin, 30).await,
    );

    CryptoSentimentReport {
        symbol,
        provider: "binance".to_string(),
        fetched_at_utc,
        futures,
        errors,
    }
}

fn push_report(
    target: &mut BTreeMap<String, Value>,
    errors: &mut BTreeMap<String, String>,
    key: &str,
    result: Result<CryptoEndpointReport>,
) {
    match result {
        Ok(report) => {
            target.insert(key.to_string(), report.payload);
        }
        Err(error) => {
            errors.insert(key.to_string(), format!("{error:#}"));
        }
    }
}
