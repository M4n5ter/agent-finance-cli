use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use wreq::Client;

use crate::http::{build_url, parse_optional_f64, send_get_text, timestamp_ms_to_utc};
use crate::model::{HistoryBatch, OhlcBar, Quote};

const PROVIDER: &str = "coingecko";
const BASE_URL: &str = "https://api.coingecko.com/api/v3";

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let coin_id = coin_id(symbol);
    let vs_currency = quote_currency(symbol).unwrap_or_else(|| "usd".to_string());
    let payload = get_json(
        client,
        "/simple/price",
        vec![
            ("ids", coin_id.clone()),
            ("vs_currencies", vs_currency.clone()),
            ("include_market_cap", "true".to_string()),
            ("include_24hr_vol", "true".to_string()),
            ("include_24hr_change", "true".to_string()),
            ("include_last_updated_at", "true".to_string()),
        ],
    )
    .await?;
    let row = payload
        .get(&coin_id)
        .ok_or_else(|| anyhow!("CoinGecko simple price had no row for {coin_id}"))?;
    let price = number(row, &vs_currency).context("missing CoinGecko simple price")?;
    Ok(Quote {
        symbol: coin_id.to_uppercase(),
        price,
        currency: Some(vs_currency.to_uppercase()),
        provider: PROVIDER.to_string(),
        session: Some("24h".to_string()),
        fetched_at_utc: crate::http::utc_now(),
        market_time: row
            .get("last_updated_at")
            .and_then(Value::as_i64)
            .and_then(|value| timestamp_ms_to_utc(value * 1000)),
        previous_close: None,
        open: None,
        high: None,
        low: None,
        volume: number(row, &format!("{vs_currency}_24h_vol")).map(|value| value as u64),
        exchange: Some("CoinGecko".to_string()),
        provider_symbol: Some(coin_id),
        change_pct: number(row, &format!("{vs_currency}_24h_change")),
    })
}

pub async fn fetch_history(
    client: &Client,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    let coin_id = coin_id(symbol);
    let vs_currency = quote_currency(symbol).unwrap_or_else(|| "usd".to_string());
    let payload = get_json(
        client,
        &format!("/coins/{coin_id}/ohlc"),
        vec![
            ("vs_currency", vs_currency),
            ("days", days_for_interval(interval, limit).to_string()),
        ],
    )
    .await?;
    let mut bars = payload
        .as_array()
        .ok_or_else(|| anyhow!("CoinGecko OHLC payload was not an array"))?
        .iter()
        .filter_map(|row| ohlc_to_bar(&coin_id, row))
        .collect::<Vec<_>>();
    bars.sort_by(|left, right| left.open_time.cmp(&right.open_time));
    let keep = limit.min(bars.len());
    if keep < bars.len() {
        bars = bars.split_off(bars.len() - keep);
    }
    Ok(HistoryBatch {
        symbol: coin_id.to_uppercase(),
        provider: PROVIDER.to_string(),
        interval: interval.to_string(),
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    })
}

pub async fn simple_price(client: &Client, symbol: &str) -> Result<Value> {
    let coin_id = coin_id(symbol);
    let vs_currency = quote_currency(symbol).unwrap_or_else(|| "usd".to_string());
    let payload = get_json(
        client,
        "/simple/price",
        vec![
            ("ids", coin_id.clone()),
            ("vs_currencies", vs_currency.clone()),
            ("include_market_cap", "true".to_string()),
            ("include_24hr_vol", "true".to_string()),
            ("include_24hr_change", "true".to_string()),
            ("include_last_updated_at", "true".to_string()),
        ],
    )
    .await?;
    validate_simple_price(&payload, &coin_id, &vs_currency)?;
    Ok(payload)
}

pub async fn coins_list(client: &Client, limit: usize) -> Result<Value> {
    get_json(client, "/coins/list", Vec::new())
        .await
        .map(|mut value| {
            if let Some(rows) = value.as_array_mut() {
                rows.truncate(limit);
            }
            value
        })
}

pub async fn markets(client: &Client, vs_currency: &str, limit: usize) -> Result<Value> {
    get_json(
        client,
        "/coins/markets",
        vec![
            ("vs_currency", vs_currency.to_ascii_lowercase()),
            ("order", "market_cap_desc".to_string()),
            ("per_page", limit.clamp(1, 250).to_string()),
            ("page", "1".to_string()),
            ("sparkline", "false".to_string()),
            ("price_change_percentage", "1h,24h,7d,30d".to_string()),
        ],
    )
    .await
}

pub async fn coin(client: &Client, symbol: &str) -> Result<Value> {
    get_json(
        client,
        &format!("/coins/{}", coin_id(symbol)),
        vec![
            ("localization", "false".to_string()),
            ("tickers", "true".to_string()),
            ("market_data", "true".to_string()),
            ("community_data", "true".to_string()),
            ("developer_data", "true".to_string()),
            ("sparkline", "false".to_string()),
        ],
    )
    .await
}

pub async fn ohlc(client: &Client, symbol: &str, interval: &str, limit: usize) -> Result<Value> {
    let mut payload = get_json(
        client,
        &format!("/coins/{}/ohlc", coin_id(symbol)),
        vec![
            (
                "vs_currency",
                quote_currency(symbol).unwrap_or_else(|| "usd".to_string()),
            ),
            ("days", days_for_interval(interval, limit).to_string()),
        ],
    )
    .await?;
    truncate_array(&mut payload, limit);
    Ok(payload)
}

pub async fn market_chart(
    client: &Client,
    symbol: &str,
    days: &str,
    limit: usize,
) -> Result<Value> {
    let mut payload = get_json(
        client,
        &format!("/coins/{}/market_chart", coin_id(symbol)),
        vec![
            (
                "vs_currency",
                quote_currency(symbol).unwrap_or_else(|| "usd".to_string()),
            ),
            ("days", days.to_string()),
        ],
    )
    .await?;
    truncate_market_chart(&mut payload, limit);
    Ok(payload)
}

pub async fn trending(client: &Client) -> Result<Value> {
    get_json(client, "/search/trending", Vec::new()).await
}

pub async fn global(client: &Client) -> Result<Value> {
    get_json(client, "/global", Vec::new()).await
}

pub async fn exchanges(client: &Client, limit: usize) -> Result<Value> {
    get_json(
        client,
        "/exchanges",
        vec![
            ("per_page", limit.clamp(1, 250).to_string()),
            ("page", "1".to_string()),
        ],
    )
    .await
}

pub async fn derivatives(client: &Client, limit: usize) -> Result<Value> {
    get_json(client, "/derivatives", Vec::new())
        .await
        .map(|mut value| {
            truncate_array(&mut value, limit);
            value
        })
}

pub async fn derivatives_exchanges(client: &Client, limit: usize) -> Result<Value> {
    get_json(
        client,
        "/derivatives/exchanges",
        vec![
            ("per_page", limit.clamp(1, 250).to_string()),
            ("page", "1".to_string()),
        ],
    )
    .await
}

async fn get_json(
    client: &Client,
    path: &str,
    params: Vec<(&'static str, String)>,
) -> Result<Value> {
    let base_url = std::env::var("COINGECKO_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
    let url = build_url(&base_url, path, &params)
        .with_context(|| format!("invalid CoinGecko API URL: {base_url}{path}"))?;
    let headers = coingecko_headers();
    let (status, body) = send_get_text(client, "CoinGecko", &url, &headers).await?;
    if !status.is_success() {
        return Err(anyhow!(
            "CoinGecko request failed status={} body={body}",
            status.as_u16()
        ));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("CoinGecko response JSON decode failed: {url}"))
}

fn coingecko_headers() -> Vec<(&'static str, String)> {
    if let Ok(key) = std::env::var("COINGECKO_API_KEY") {
        vec![("x-cg-pro-api-key", key)]
    } else if let Ok(key) = std::env::var("COINGECKO_DEMO_API_KEY") {
        vec![("x-cg-demo-api-key", key)]
    } else {
        Vec::new()
    }
}

fn validate_simple_price(payload: &Value, coin_id: &str, vs_currency: &str) -> Result<()> {
    let row = payload
        .get(coin_id)
        .ok_or_else(|| anyhow!("CoinGecko simple price had no row for {coin_id}"))?;
    if row.get(vs_currency).and_then(Value::as_f64).is_none() {
        return Err(anyhow!(
            "CoinGecko simple price missing {} quote for {}",
            vs_currency,
            coin_id
        ));
    }
    Ok(())
}

fn truncate_array(payload: &mut Value, limit: usize) {
    if let Some(rows) = payload.as_array_mut() {
        rows.truncate(limit.clamp(1, 1000));
    }
}

fn truncate_market_chart(payload: &mut Value, limit: usize) {
    for field in ["prices", "market_caps", "total_volumes"] {
        if let Some(rows) = payload.get_mut(field).and_then(Value::as_array_mut) {
            rows.truncate(limit.clamp(1, 1000));
        }
    }
}

fn coin_id(input: &str) -> String {
    let base = base_symbol(input).to_ascii_lowercase();
    match base.as_str() {
        "btc" | "xbt" => "bitcoin".to_string(),
        "eth" => "ethereum".to_string(),
        "sol" => "solana".to_string(),
        "xrp" => "ripple".to_string(),
        "doge" => "dogecoin".to_string(),
        "ada" => "cardano".to_string(),
        "bnb" => "binancecoin".to_string(),
        "avax" => "avalanche-2".to_string(),
        "link" => "chainlink".to_string(),
        "ltc" => "litecoin".to_string(),
        "dot" => "polkadot".to_string(),
        "matic" | "pol" => "matic-network".to_string(),
        other => other.to_string(),
    }
}

fn base_symbol(input: &str) -> String {
    let upper = input.trim().to_uppercase().replace(['/', '_', ':'], "-");
    if let Some((base, _)) = upper.split_once('-') {
        return base.to_string();
    }
    for quote in ["USDT", "USDC", "USD", "EUR", "BTC", "ETH"] {
        if let Some(base) = upper.strip_suffix(quote).filter(|base| !base.is_empty()) {
            return base.to_string();
        }
    }
    upper
}

fn quote_currency(input: &str) -> Option<String> {
    let upper = input.trim().to_uppercase().replace(['/', '_', ':'], "-");
    if let Some((_, quote)) = upper.split_once('-') {
        return Some(coingecko_vs_currency(quote));
    }
    for quote in ["USDT", "USDC", "USD", "EUR", "BTC", "ETH"] {
        if upper.ends_with(quote) && upper.len() > quote.len() {
            return Some(coingecko_vs_currency(quote));
        }
    }
    None
}

fn coingecko_vs_currency(quote: &str) -> String {
    match quote.to_ascii_uppercase().as_str() {
        "USDT" | "USDC" => "usd".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

fn days_for_interval(interval: &str, limit: usize) -> &'static str {
    match interval {
        "1m" | "3m" | "5m" | "15m" | "30m" | "1h" => "1",
        "4h" | "6h" | "12h" => "7",
        "1d" if limit <= 30 => "30",
        "1d" if limit <= 90 => "90",
        "1d" if limit <= 180 => "180",
        "1d" => "365",
        _ => "30",
    }
}

fn ohlc_to_bar(coin_id: &str, row: &Value) -> Option<OhlcBar> {
    let row = row.as_array()?;
    let close_time = value_i64(row.first())?;
    Some(OhlcBar {
        symbol: coin_id.to_uppercase(),
        provider: PROVIDER.to_string(),
        open_time: timestamp_ms_to_utc(close_time)?,
        close_time: timestamp_ms_to_utc(close_time),
        open: value_f64(row.get(1)),
        high: value_f64(row.get(2)),
        low: value_f64(row.get(3)),
        close: value_f64(row.get(4))?,
        adj_close: None,
        volume: None,
        quote_volume: None,
        trades: None,
        dividend: None,
        stock_split: None,
        capital_gain: None,
        repaired: false,
    })
}

fn number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| value_f64(Some(value)))
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
