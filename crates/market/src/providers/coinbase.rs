use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use wreq::Client;

use crate::http::{build_url, parse_optional_f64, send_get_text, timestamp_sec_to_utc};
use crate::model::{HistoryBatch, OhlcBar, Quote};

const PROVIDER: &str = "coinbase";
const BASE_URL: &str = "https://api.exchange.coinbase.com";

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let product = product_id(symbol);
    let payload = get_json(client, &format!("/products/{product}/ticker"), Vec::new()).await?;
    let price = number(&payload, "price").context("missing Coinbase ticker price")?;
    Ok(Quote {
        symbol: product.replace('-', ""),
        price,
        currency: quote_currency(&product),
        provider: PROVIDER.to_string(),
        session: Some("24h".to_string()),
        fetched_at_utc: crate::http::utc_now(),
        market_time: payload
            .get("time")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        previous_close: None,
        open: None,
        high: None,
        low: None,
        volume: number(&payload, "volume").map(|value| value as u64),
        exchange: Some("Coinbase Exchange".to_string()),
        provider_symbol: Some(product),
        change_pct: None,
    })
}

pub async fn fetch_history(
    client: &Client,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    let product = product_id(symbol);
    let granularity = granularity(interval)?;
    let payload = get_json(
        client,
        &format!("/products/{product}/candles"),
        vec![("granularity", granularity.to_string())],
    )
    .await?;
    let mut bars = payload
        .as_array()
        .ok_or_else(|| anyhow!("Coinbase candles payload was not an array"))?
        .iter()
        .filter_map(|row| candle_to_bar(&product, row))
        .collect::<Vec<_>>();
    bars.sort_by(|left, right| left.open_time.cmp(&right.open_time));
    let keep = limit.min(bars.len());
    if keep < bars.len() {
        bars = bars.split_off(bars.len() - keep);
    }
    Ok(HistoryBatch {
        symbol: product.replace('-', ""),
        provider: PROVIDER.to_string(),
        interval: interval.to_string(),
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    })
}

pub async fn products(client: &Client) -> Result<Value> {
    get_json(client, "/products", Vec::new()).await
}

pub async fn volume_summary(client: &Client) -> Result<Value> {
    get_json(client, "/products/volume-summary", Vec::new()).await
}

pub async fn product(client: &Client, symbol: &str) -> Result<Value> {
    get_json(
        client,
        &format!("/products/{}", product_id(symbol)),
        Vec::new(),
    )
    .await
}

pub async fn ticker(client: &Client, symbol: &str) -> Result<Value> {
    get_json(
        client,
        &format!("/products/{}/ticker", product_id(symbol)),
        Vec::new(),
    )
    .await
}

pub async fn stats(client: &Client, symbol: &str) -> Result<Value> {
    get_json(
        client,
        &format!("/products/{}/stats", product_id(symbol)),
        Vec::new(),
    )
    .await
}

pub async fn book(client: &Client, symbol: &str, limit: usize) -> Result<Value> {
    let mut payload = get_json(
        client,
        &format!("/products/{}/book", product_id(symbol)),
        vec![("level", "2".to_string())],
    )
    .await?;
    truncate_book_side(&mut payload, "bids", limit);
    truncate_book_side(&mut payload, "asks", limit);
    Ok(payload)
}

pub async fn trades(client: &Client, symbol: &str, limit: usize) -> Result<Value> {
    get_json(
        client,
        &format!("/products/{}/trades", product_id(symbol)),
        vec![("limit", limit.clamp(1, 1000).to_string())],
    )
    .await
}

pub async fn candles(client: &Client, symbol: &str, interval: &str, limit: usize) -> Result<Value> {
    let mut payload = get_json(
        client,
        &format!("/products/{}/candles", product_id(symbol)),
        vec![("granularity", granularity(interval)?.to_string())],
    )
    .await?;
    truncate_array(&mut payload, limit);
    Ok(payload)
}

async fn get_json(
    client: &Client,
    path: &str,
    params: Vec<(&'static str, String)>,
) -> Result<Value> {
    let base_url =
        std::env::var("COINBASE_EXCHANGE_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
    let url = build_url(&base_url, path, &params)
        .with_context(|| format!("invalid Coinbase API URL: {base_url}{path}"))?;
    let (status, body) = send_get_text(client, "Coinbase", &url, &[]).await?;
    if !status.is_success() {
        return Err(anyhow!(
            "Coinbase request failed status={} body={body}",
            status.as_u16()
        ));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("Coinbase response JSON decode failed: {url}"))
}

fn product_id(input: &str) -> String {
    let upper = input.trim().to_uppercase().replace(['/', '_', ':'], "-");
    if upper.contains('-') {
        return upper;
    }
    for quote in ["USDT", "USDC", "USD", "EUR", "GBP", "BTC", "ETH"] {
        if let Some(base) = upper.strip_suffix(quote).filter(|base| !base.is_empty()) {
            return format!("{base}-{quote}");
        }
    }
    format!("{upper}-USD")
}

fn quote_currency(product: &str) -> Option<String> {
    product.rsplit_once('-').map(|(_, quote)| quote.to_string())
}

fn granularity(interval: &str) -> Result<u64> {
    match interval {
        "1m" => Ok(60),
        "5m" => Ok(300),
        "15m" => Ok(900),
        "1h" => Ok(3600),
        "6h" => Ok(21600),
        "1d" => Ok(86400),
        _ => Err(anyhow!("unsupported Coinbase candle interval: {interval}")),
    }
}

fn truncate_book_side(payload: &mut Value, side: &str, limit: usize) {
    if let Some(rows) = payload.get_mut(side).and_then(Value::as_array_mut) {
        rows.truncate(limit.clamp(1, 1000));
    }
}

fn truncate_array(payload: &mut Value, limit: usize) {
    if let Some(rows) = payload.as_array_mut() {
        rows.truncate(limit.clamp(1, 1000));
    }
}

fn candle_to_bar(product: &str, row: &Value) -> Option<OhlcBar> {
    let row = row.as_array()?;
    let open_time = value_i64(row.first())?;
    Some(OhlcBar {
        symbol: product.replace('-', ""),
        provider: PROVIDER.to_string(),
        open_time: timestamp_sec_to_utc(open_time)?,
        close_time: None,
        open: value_f64(row.get(3)),
        high: value_f64(row.get(2)),
        low: value_f64(row.get(1)),
        close: value_f64(row.get(4))?,
        adj_close: None,
        volume: value_f64(row.get(5)),
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
