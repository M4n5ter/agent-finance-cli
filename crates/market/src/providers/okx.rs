use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use wreq::Client;

use crate::args::CryptoInstrument;
use crate::http::{
    build_url, parse_optional_f64, parse_optional_u64, send_get_text, timestamp_ms_to_utc,
};
use crate::model::{HistoryBatch, OhlcBar, Quote};

const PROVIDER: &str = "okx";
const BASE_URL: &str = "https://www.okx.com";

pub async fn fetch_quote(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
) -> Result<Quote> {
    let inst_id = instrument_id(symbol, instrument)?;
    let payload = get_json(
        client,
        "/api/v5/market/ticker",
        vec![("instId", inst_id.clone())],
    )
    .await?;
    let ticker = first_data(&payload).context("missing OKX ticker data")?;
    let price = number(ticker, "last").context("missing OKX last price")?;
    Ok(Quote {
        symbol: inst_id.replace('-', ""),
        price,
        currency: quote_currency(&inst_id),
        provider: PROVIDER.to_string(),
        session: Some("24h".to_string()),
        fetched_at_utc: crate::http::utc_now(),
        market_time: ticker
            .get("ts")
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(timestamp_ms_to_utc),
        previous_close: None,
        open: number(ticker, "open24h"),
        high: number(ticker, "high24h"),
        low: number(ticker, "low24h"),
        volume: ticker
            .get("vol24h")
            .and_then(Value::as_str)
            .and_then(|value| parse_optional_u64(Some(value))),
        exchange: Some("OKX".to_string()),
        provider_symbol: Some(inst_id),
        change_pct: None,
    })
}

pub async fn fetch_history(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    let inst_id = instrument_id(symbol, instrument)?;
    let payload = get_json(
        client,
        "/api/v5/market/candles",
        vec![
            ("instId", inst_id.clone()),
            ("bar", okx_bar(interval)?.to_string()),
            ("limit", limit.clamp(1, 300).to_string()),
        ],
    )
    .await?;
    let mut bars = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("OKX candles payload had no data array"))?
        .iter()
        .filter_map(|row| candle_to_bar(&inst_id, row))
        .collect::<Vec<_>>();
    bars.sort_by(|left, right| left.open_time.cmp(&right.open_time));
    Ok(HistoryBatch {
        symbol: inst_id.replace('-', ""),
        provider: PROVIDER.to_string(),
        interval: interval.to_string(),
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    })
}

pub async fn instruments(client: &Client, instrument: CryptoInstrument) -> Result<Value> {
    get_json(
        client,
        "/api/v5/public/instruments",
        vec![("instType", inst_type(instrument)?.to_string())],
    )
    .await
}

pub async fn tickers(client: &Client, instrument: CryptoInstrument) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/tickers",
        vec![("instType", inst_type(instrument)?.to_string())],
    )
    .await
}

pub async fn ticker(client: &Client, symbol: &str, instrument: CryptoInstrument) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/ticker",
        vec![("instId", instrument_id(symbol, instrument)?)],
    )
    .await
}

pub async fn book(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    limit: usize,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/books",
        vec![
            ("instId", instrument_id(symbol, instrument)?),
            ("sz", limit.clamp(1, 400).to_string()),
        ],
    )
    .await
}

pub async fn trades(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    limit: usize,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/trades",
        vec![
            ("instId", instrument_id(symbol, instrument)?),
            ("limit", limit.clamp(1, 500).to_string()),
        ],
    )
    .await
}

pub async fn candles(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    interval: &str,
    limit: usize,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/candles",
        vec![
            ("instId", instrument_id(symbol, instrument)?),
            ("bar", okx_bar(interval)?.to_string()),
            ("limit", limit.clamp(1, 300).to_string()),
        ],
    )
    .await
}

pub async fn history_candles(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    interval: &str,
    limit: usize,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/market/history-candles",
        vec![
            ("instId", instrument_id(symbol, instrument)?),
            ("bar", okx_bar(interval)?.to_string()),
            ("limit", limit.clamp(1, 300).to_string()),
        ],
    )
    .await
}

pub async fn mark_price(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/public/mark-price",
        vec![
            ("instType", inst_type(instrument)?.to_string()),
            ("instId", instrument_id(symbol, instrument)?),
        ],
    )
    .await
}

pub async fn funding_rate(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/public/funding-rate",
        vec![("instId", instrument_id(symbol, instrument)?)],
    )
    .await
}

pub async fn funding_rate_history(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
    limit: usize,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/public/funding-rate-history",
        vec![
            ("instId", instrument_id(symbol, instrument)?),
            ("limit", limit.clamp(1, 100).to_string()),
        ],
    )
    .await
}

pub async fn open_interest(
    client: &Client,
    symbol: &str,
    instrument: CryptoInstrument,
) -> Result<Value> {
    get_json(
        client,
        "/api/v5/public/open-interest",
        vec![
            ("instType", inst_type(instrument)?.to_string()),
            ("instId", instrument_id(symbol, instrument)?),
        ],
    )
    .await
}

async fn get_json(
    client: &Client,
    path: &str,
    params: Vec<(&'static str, String)>,
) -> Result<Value> {
    let base_url = std::env::var("OKX_BASE_URL").unwrap_or_else(|_| BASE_URL.to_string());
    let url = build_url(&base_url, path, &params)
        .with_context(|| format!("invalid OKX API URL: {base_url}{path}"))?;
    let (status, body) = send_get_text(client, "OKX", &url, &[]).await?;
    if !status.is_success() {
        return Err(anyhow!(
            "OKX request failed status={} body={body}",
            status.as_u16()
        ));
    }
    let payload: Value = serde_json::from_str(&body)
        .with_context(|| format!("OKX response JSON decode failed: {url}"))?;
    validate_okx_payload(&payload)?;
    Ok(payload)
}

fn validate_okx_payload(payload: &Value) -> Result<()> {
    let code = payload
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("OKX response missing business code"))?;
    if code != "0" {
        return Err(anyhow!(
            "OKX business error code={} message={}",
            code,
            payload
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("missing message")
        ));
    }
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("OKX response missing data array"))?;
    if data.is_empty() {
        return Err(anyhow!("OKX response data array was empty"));
    }
    Ok(())
}

fn first_data(payload: &Value) -> Option<&Value> {
    payload.get("data")?.as_array()?.first()
}

fn instrument_id(input: &str, instrument: CryptoInstrument) -> Result<String> {
    let upper = input.trim().to_uppercase().replace(['/', '_', ':'], "-");
    let pair = if upper.contains('-') {
        upper
    } else {
        let mut value = None;
        for quote in ["USDT", "USDC", "USD", "BTC", "ETH"] {
            if let Some(base) = upper.strip_suffix(quote).filter(|base| !base.is_empty()) {
                value = Some(format!("{base}-{quote}"));
                break;
            }
        }
        value.unwrap_or_else(|| format!("{upper}-USDT"))
    };
    match instrument {
        CryptoInstrument::Auto | CryptoInstrument::Spot => Ok(pair),
        CryptoInstrument::Swap => Ok(if pair.ends_with("-SWAP") {
            pair
        } else {
            format!("{pair}-SWAP")
        }),
        CryptoInstrument::Futures | CryptoInstrument::Option => {
            if pair.split('-').count() >= 3 {
                Ok(pair)
            } else {
                Err(anyhow!(
                    "OKX {} requires full instrument id, for example BTC-USDT-240329",
                    instrument.label()
                ))
            }
        }
    }
}

fn inst_type(instrument: CryptoInstrument) -> Result<&'static str> {
    match instrument {
        CryptoInstrument::Auto | CryptoInstrument::Spot => Ok("SPOT"),
        CryptoInstrument::Swap => Ok("SWAP"),
        CryptoInstrument::Futures => Ok("FUTURES"),
        CryptoInstrument::Option => Ok("OPTION"),
    }
}

fn quote_currency(inst_id: &str) -> Option<String> {
    let id = inst_id.strip_suffix("-SWAP").unwrap_or(inst_id);
    id.rsplit_once('-').map(|(_, quote)| quote.to_string())
}

fn okx_bar(interval: &str) -> Result<&'static str> {
    match interval {
        "1m" => Ok("1m"),
        "3m" => Ok("3m"),
        "5m" => Ok("5m"),
        "15m" => Ok("15m"),
        "30m" => Ok("30m"),
        "1h" => Ok("1H"),
        "2h" => Ok("2H"),
        "4h" => Ok("4H"),
        "6h" => Ok("6H"),
        "12h" => Ok("12H"),
        "1d" => Ok("1D"),
        "2d" => Ok("2D"),
        "3d" => Ok("3D"),
        _ => Err(anyhow!("unsupported OKX candle interval: {interval}")),
    }
}

fn candle_to_bar(inst_id: &str, row: &Value) -> Option<OhlcBar> {
    let row = row.as_array()?;
    let open_time = value_i64(row.first())?;
    Some(OhlcBar {
        symbol: inst_id.replace('-', ""),
        provider: PROVIDER.to_string(),
        open_time: timestamp_ms_to_utc(open_time)?,
        close_time: None,
        open: value_f64(row.get(1)),
        high: value_f64(row.get(2)),
        low: value_f64(row.get(3)),
        close: value_f64(row.get(4))?,
        adj_close: None,
        volume: value_f64(row.get(5)),
        quote_volume: value_f64(row.get(7)),
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
