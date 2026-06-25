use anyhow::{Context, Result};
use serde_json::json;

use crate::args::FuturesPeriod;

mod aggregate;
mod config;
mod quote_history;
mod rest;
mod symbol;
#[cfg(test)]
mod tests;
mod types;
mod websocket;

pub use aggregate::{sentiment, snapshot};
pub use config::BinanceConfig;
pub use quote_history::{fetch_history, fetch_quote, futures_quote};
pub use symbol::{normalize_symbol, symbol_to_pair};
pub use types::{
    BINANCE_ENDPOINTS, CryptoEndpointReport, CryptoSentimentReport, CryptoSnapshotReport,
    CryptoStreamReport,
};
pub use websocket::stream_messages;

use symbol::futures_period;
use symbol::{clamp_usize, ensure_futures_interval, ensure_spot_interval};
use types::RestMarket;

pub async fn spot_exchange_info(
    config: &BinanceConfig,
    symbol: Option<&str>,
) -> Result<CryptoEndpointReport> {
    let route = "crypto spot exchange-info";
    let normalized = symbol.map(normalize_symbol).transpose()?;
    let mut params = Vec::new();
    if let Some(symbol) = normalized.as_deref() {
        params.push(("symbol", symbol.to_string()));
    }
    rest::endpoint_report(
        config,
        RestMarket::Spot,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        normalized,
        params,
    )
    .await
}

pub async fn spot_ticker(config: &BinanceConfig, symbol: &str) -> Result<CryptoEndpointReport> {
    let route = "crypto spot ticker";
    symbol_endpoint(
        config,
        RestMarket::Spot,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn spot_24h(config: &BinanceConfig, symbol: &str) -> Result<CryptoEndpointReport> {
    let route = "crypto spot ticker24h";
    symbol_endpoint(
        config,
        RestMarket::Spot,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn spot_book(
    config: &BinanceConfig,
    symbol: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto spot book";
    symbol_endpoint(
        config,
        RestMarket::Spot,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![("limit", clamp_usize(limit, 1, 5000).to_string())],
    )
    .await
}

pub async fn spot_book_ticker(
    config: &BinanceConfig,
    symbol: &str,
) -> Result<CryptoEndpointReport> {
    symbol_endpoint(
        config,
        RestMarket::Spot,
        "book-ticker",
        "/api/v3/ticker/bookTicker",
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn spot_trades(
    config: &BinanceConfig,
    symbol: &str,
    limit: usize,
    aggregate: bool,
) -> Result<CryptoEndpointReport> {
    let paths = types::official_paths("crypto spot trades")?;
    let (endpoint, path) = if aggregate {
        ("agg-trades", paths[0])
    } else {
        ("trades", paths[1])
    };
    symbol_endpoint(
        config,
        RestMarket::Spot,
        endpoint,
        path,
        symbol,
        vec![("limit", clamp_usize(limit, 1, 1000).to_string())],
    )
    .await
}

pub async fn spot_klines(
    config: &BinanceConfig,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto spot klines";
    ensure_spot_interval(interval)?;
    symbol_endpoint(
        config,
        RestMarket::Spot,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![
            ("interval", interval.to_string()),
            ("limit", clamp_usize(limit, 1, 1000).to_string()),
        ],
    )
    .await
}

pub async fn futures_ticker(config: &BinanceConfig, symbol: &str) -> Result<CryptoEndpointReport> {
    let route = "crypto futures ticker";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn futures_24h(config: &BinanceConfig, symbol: &str) -> Result<CryptoEndpointReport> {
    let route = "crypto futures ticker24h";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn futures_book(
    config: &BinanceConfig,
    symbol: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures book";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![("limit", clamp_usize(limit, 1, 1000).to_string())],
    )
    .await
}

pub async fn futures_trades(
    config: &BinanceConfig,
    symbol: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures trades";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        "agg-trades",
        types::official_path(route)?,
        symbol,
        vec![("limit", clamp_usize(limit, 1, 1000).to_string())],
    )
    .await
}

pub async fn futures_klines(
    config: &BinanceConfig,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures klines";
    ensure_futures_interval(interval)?;
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![
            ("interval", interval.to_string()),
            ("limit", clamp_usize(limit, 1, 1500).to_string()),
        ],
    )
    .await
}

pub async fn futures_mark(config: &BinanceConfig, symbol: &str) -> Result<CryptoEndpointReport> {
    let route = "crypto futures mark";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn futures_funding(
    config: &BinanceConfig,
    symbol: &str,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures funding";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![("limit", clamp_usize(limit, 1, 1000).to_string())],
    )
    .await
}

pub async fn futures_open_interest(
    config: &BinanceConfig,
    symbol: &str,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures open-interest";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        Vec::new(),
    )
    .await
}

pub async fn futures_ratios(
    config: &BinanceConfig,
    symbol: &str,
    period: FuturesPeriod,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let paths = types::official_paths("crypto futures ratios")?;
    let symbol = normalize_symbol(symbol)?;
    let params = vec![
        ("symbol", symbol.clone()),
        ("period", futures_period(period).to_string()),
        ("limit", clamp_usize(limit, 1, 1000).to_string()),
    ];
    let (global, accounts, positions) = tokio::join!(
        rest::rest_get(config, RestMarket::Futures, paths[0], params.clone()),
        rest::rest_get(config, RestMarket::Futures, paths[1], params.clone()),
        rest::rest_get(config, RestMarket::Futures, paths[2], params)
    );
    let payload = json!({
        "global_long_short": global.context("Binance futures global long/short request failed")?,
        "top_trader_accounts": accounts.context("Binance futures top accounts request failed")?,
        "top_trader_positions": positions.context("Binance futures top positions request failed")?,
    });
    Ok(types::report_from_value(
        RestMarket::Futures,
        "ratios",
        Some(symbol),
        200,
        Vec::new(),
        payload,
    ))
}

pub async fn futures_flow(
    config: &BinanceConfig,
    symbol: &str,
    period: FuturesPeriod,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures flow";
    symbol_endpoint(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        symbol,
        vec![
            ("period", futures_period(period).to_string()),
            ("limit", clamp_usize(limit, 1, 1000).to_string()),
        ],
    )
    .await
}

pub async fn futures_basis(
    config: &BinanceConfig,
    symbol: &str,
    period: FuturesPeriod,
    limit: usize,
) -> Result<CryptoEndpointReport> {
    let route = "crypto futures basis";
    let symbol = normalize_symbol(symbol)?;
    rest::endpoint_report(
        config,
        RestMarket::Futures,
        types::report_endpoint(route)?,
        types::official_path(route)?,
        Some(symbol.clone()),
        vec![
            ("pair", symbol_to_pair(&symbol)),
            ("contractType", "PERPETUAL".to_string()),
            ("period", futures_period(period).to_string()),
            ("limit", clamp_usize(limit, 1, 1000).to_string()),
        ],
    )
    .await
}

async fn symbol_endpoint(
    config: &BinanceConfig,
    market: RestMarket,
    endpoint: &str,
    path: &str,
    symbol: &str,
    mut params: Vec<(&'static str, String)>,
) -> Result<CryptoEndpointReport> {
    let symbol = normalize_symbol(symbol)?;
    params.insert(0, ("symbol", symbol.clone()));
    rest::endpoint_report(config, market, endpoint, path, Some(symbol), params).await
}
