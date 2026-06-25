use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;

use crate::http::utc_now;

pub const SPOT_PROVIDER: &str = "binance-spot";
pub const FUTURES_PROVIDER: &str = "binance-usds-futures";

#[derive(Debug, Clone, Copy)]
pub(super) enum RestMarket {
    Spot,
    Futures,
}

#[derive(Debug, Clone, Serialize)]
pub struct CryptoEndpointReport {
    pub provider: String,
    pub market: String,
    pub endpoint: String,
    pub symbol: Option<String>,
    pub fetched_at_utc: String,
    pub status: u16,
    pub rate_limits: Vec<CryptoRateLimit>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct CryptoRateLimit {
    pub rate_limit_type: String,
    pub interval: String,
    pub interval_num: u32,
    pub count: u32,
    pub retry_after: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CryptoSnapshotReport {
    pub symbol: String,
    pub provider: String,
    pub fetched_at_utc: String,
    pub spot: BTreeMap<String, Value>,
    pub futures: BTreeMap<String, Value>,
    pub errors: BTreeMap<String, String>,
}

impl CryptoSnapshotReport {
    pub fn ensure_complete(&self) -> Result<()> {
        ensure_required(
            &self.symbol,
            "snapshot",
            &self.errors,
            &["spot.ticker", "futures.ticker"],
        )
    }
}

#[derive(Debug, Serialize)]
pub struct CryptoSentimentReport {
    pub symbol: String,
    pub provider: String,
    pub fetched_at_utc: String,
    pub futures: BTreeMap<String, Value>,
    pub errors: BTreeMap<String, String>,
}

impl CryptoSentimentReport {
    pub fn ensure_complete(&self) -> Result<()> {
        ensure_required(
            &self.symbol,
            "sentiment",
            &self.errors,
            &["mark", "funding"],
        )
    }
}

#[derive(Debug, Serialize)]
pub struct CryptoStreamReport {
    pub symbol: String,
    pub provider: String,
    pub market: String,
    pub kind: String,
    pub interval: Option<String>,
    pub fetched_at_utc: String,
    pub messages: Vec<Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct BinanceEndpoint {
    pub route: &'static str,
    pub market: &'static str,
    pub official_endpoint: &'static str,
    pub implementation: &'static str,
    pub output_model: &'static str,
    pub requires_api_key: bool,
    pub live_symbol: Option<&'static str>,
}

pub const BINANCE_ENDPOINTS: &[BinanceEndpoint] = &[
    endpoint(
        "crypto snapshot",
        "combined",
        "aggregate:/api/v3/ticker/price+/fapi/v2/ticker/price",
        "binance::snapshot",
        "CryptoSnapshotReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto sentiment",
        "usds-futures",
        "aggregate:/fapi/v1/premiumIndex+/fapi/v1/fundingRate",
        "binance::sentiment",
        "CryptoSentimentReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot exchange-info",
        "spot",
        "GET /api/v3/exchangeInfo",
        "binance::spot_exchange_info",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot ticker",
        "spot",
        "GET /api/v3/ticker/price",
        "binance::spot_ticker",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot ticker24h",
        "spot",
        "GET /api/v3/ticker/24hr",
        "binance::spot_24h",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot book",
        "spot",
        "GET /api/v3/depth",
        "binance::spot_book",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot trades",
        "spot",
        "GET /api/v3/aggTrades|GET /api/v3/trades",
        "binance::spot_trades",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto spot klines",
        "spot",
        "GET /api/v3/klines",
        "binance::spot_klines",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures ticker",
        "usds-futures",
        "GET /fapi/v2/ticker/price",
        "binance::futures_ticker",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures ticker24h",
        "usds-futures",
        "GET /fapi/v1/ticker/24hr",
        "binance::futures_24h",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures book",
        "usds-futures",
        "GET /fapi/v1/depth",
        "binance::futures_book",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures trades",
        "usds-futures",
        "GET /fapi/v1/aggTrades",
        "binance::futures_trades",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures klines",
        "usds-futures",
        "GET /fapi/v1/klines",
        "binance::futures_klines",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures mark",
        "usds-futures",
        "GET /fapi/v1/premiumIndex",
        "binance::futures_mark",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures funding",
        "usds-futures",
        "GET /fapi/v1/fundingRate",
        "binance::futures_funding",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures open-interest",
        "usds-futures",
        "GET /fapi/v1/openInterest",
        "binance::futures_open_interest",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures ratios",
        "usds-futures",
        "GET /futures/data/globalLongShortAccountRatio|GET /futures/data/topLongShortAccountRatio|GET /futures/data/topLongShortPositionRatio",
        "binance::futures_ratios",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures flow",
        "usds-futures",
        "GET /futures/data/takerlongshortRatio",
        "binance::futures_flow",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
    endpoint(
        "crypto futures basis",
        "usds-futures",
        "GET /futures/data/basis",
        "binance::futures_basis",
        "CryptoEndpointReport",
        Some("BTCUSDT"),
    ),
];

const fn endpoint(
    route: &'static str,
    market: &'static str,
    official_endpoint: &'static str,
    implementation: &'static str,
    output_model: &'static str,
    live_symbol: Option<&'static str>,
) -> BinanceEndpoint {
    BinanceEndpoint {
        route,
        market,
        official_endpoint,
        implementation,
        output_model,
        requires_api_key: false,
        live_symbol,
    }
}

pub(super) fn report_from_value(
    market: RestMarket,
    endpoint: &str,
    symbol: Option<String>,
    status: u16,
    rate_limits: Vec<CryptoRateLimit>,
    payload: Value,
) -> CryptoEndpointReport {
    CryptoEndpointReport {
        provider: provider(market).to_string(),
        market: market_name(market).to_string(),
        endpoint: endpoint.to_string(),
        symbol,
        fetched_at_utc: utc_now(),
        status,
        rate_limits,
        payload,
    }
}

pub(super) fn official_paths(route: &str) -> Result<Vec<&'static str>> {
    let endpoint = endpoint_by_route(route)?;
    endpoint
        .official_endpoint
        .split('|')
        .map(|part| {
            part.strip_prefix("GET ")
                .ok_or_else(|| anyhow!("Binance route {route} does not map to a REST GET path"))
        })
        .collect()
}

pub(super) fn official_path(route: &str) -> Result<&'static str> {
    official_paths(route)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Binance route {route} has no REST path"))
}

pub(super) fn report_endpoint(route: &str) -> Result<&'static str> {
    let endpoint = endpoint_by_route(route)?;
    let segment = endpoint
        .route
        .rsplit_once(' ')
        .map(|(_, segment)| segment)
        .ok_or_else(|| anyhow!("invalid Binance route: {route}"))?;
    Ok(match segment {
        "ticker24h" => "ticker-24h",
        "exchange-info" => "exchange-info",
        other => other,
    })
}

fn endpoint_by_route(route: &str) -> Result<&'static BinanceEndpoint> {
    BINANCE_ENDPOINTS
        .iter()
        .find(|endpoint| endpoint.route == route)
        .ok_or_else(|| anyhow!("unknown Binance route: {route}"))
}

fn ensure_required(
    symbol: &str,
    report_name: &str,
    errors: &BTreeMap<String, String>,
    required_keys: &[&str],
) -> Result<()> {
    let required_errors = errors
        .iter()
        .filter(|(key, _)| required_keys.contains(&key.as_str()))
        .map(|(key, error)| (key.clone(), error.clone()))
        .collect::<BTreeMap<_, _>>();
    if required_errors.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "Binance crypto {report_name} missed required data for {symbol}: {}",
        summarize_errors(&required_errors)
    ))
}

fn summarize_errors(errors: &BTreeMap<String, String>) -> String {
    errors
        .iter()
        .map(|(key, error)| format!("{key}={error}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn provider(market: RestMarket) -> &'static str {
    match market {
        RestMarket::Spot => SPOT_PROVIDER,
        RestMarket::Futures => FUTURES_PROVIDER,
    }
}

pub(super) fn market_name(market: RestMarket) -> &'static str {
    match market {
        RestMarket::Spot => "spot",
        RestMarket::Futures => "usds-futures",
    }
}
