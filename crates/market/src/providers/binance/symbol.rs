use anyhow::{Result, anyhow};

use crate::args::{CryptoMarket, FuturesPeriod};

use super::RestMarket;

pub fn normalize_symbol(input: &str) -> Result<String> {
    let normalized = input
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '/' | '-' | '_' | ':' | ' '))
        .collect::<String>()
        .to_uppercase();
    if normalized.is_empty() {
        return Err(anyhow!("symbol is empty after normalization"));
    }
    if !normalized.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(anyhow!("unsupported Binance symbol format: {input}"));
    }
    Ok(normalized)
}

pub fn symbol_to_pair(symbol: &str) -> String {
    symbol
        .strip_suffix("USDT")
        .map(|base| format!("{base}USDT"))
        .unwrap_or_else(|| symbol.to_string())
}

pub(super) fn rest_market(market: CryptoMarket) -> Result<RestMarket> {
    match market {
        CryptoMarket::Auto => Err(anyhow!(
            "crypto market auto must be resolved before dispatch"
        )),
        CryptoMarket::Spot => Ok(RestMarket::Spot),
        CryptoMarket::UsdsFutures => Ok(RestMarket::Futures),
    }
}

pub(super) fn clamp_usize(value: usize, min: usize, max: usize) -> usize {
    value.clamp(min, max)
}

pub(super) fn ensure_spot_interval(interval: &str) -> Result<()> {
    match interval {
        "1s" | "1m" | "3m" | "5m" | "15m" | "30m" | "1h" | "2h" | "4h" | "6h" | "8h" | "12h"
        | "1d" | "3d" | "1w" | "1M" => Ok(()),
        _ => Err(anyhow!("unsupported Binance spot interval: {interval}")),
    }
}

pub(super) fn ensure_futures_interval(interval: &str) -> Result<()> {
    match interval {
        "1m" | "3m" | "5m" | "15m" | "30m" | "1h" | "2h" | "4h" | "6h" | "8h" | "12h" | "1d"
        | "3d" | "1w" | "1M" => Ok(()),
        _ => Err(anyhow!("unsupported Binance futures interval: {interval}")),
    }
}

pub(super) fn futures_period(period: FuturesPeriod) -> &'static str {
    match period {
        FuturesPeriod::FiveMin => "5m",
        FuturesPeriod::FifteenMin => "15m",
        FuturesPeriod::ThirtyMin => "30m",
        FuturesPeriod::OneHour => "1h",
        FuturesPeriod::TwoHour => "2h",
        FuturesPeriod::FourHour => "4h",
        FuturesPeriod::SixHour => "6h",
        FuturesPeriod::TwelveHour => "12h",
        FuturesPeriod::OneDay => "1d",
    }
}
