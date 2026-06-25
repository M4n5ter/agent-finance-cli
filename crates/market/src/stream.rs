use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use futures_util::{SinkExt, StreamExt};
use prost::Message as _;
use serde::Serialize;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::http::selected_proxy;
use crate::http::timestamp_ms_to_utc;
use crate::model::StreamQuote;
use crate::time::utc_to_local;
use crate::websocket;

#[derive(Serialize)]
struct SubscribeMessage {
    subscribe: Vec<String>,
}

pub async fn stream_quotes(options: StreamOptions<'_>) -> Result<Vec<StreamQuote>> {
    let mut updates = Vec::new();
    stream_quotes_each(options, |quote| {
        updates.push(quote);
        Ok(())
    })
    .await?;
    Ok(updates)
}

#[derive(Debug)]
pub struct StreamOptions<'a> {
    pub url: &'a str,
    pub symbols: Vec<String>,
    pub message_limit: usize,
    pub read_timeout: Duration,
    pub timezone: &'a str,
    pub proxy: Option<&'a str>,
    pub no_proxy: bool,
}

pub async fn stream_quotes_each<F>(options: StreamOptions<'_>, mut on_quote: F) -> Result<()>
where
    F: FnMut(StreamQuote) -> Result<()>,
{
    let _ = rustls::crypto::ring::default_provider().install_default();
    let symbols = options
        .symbols
        .into_iter()
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect::<Vec<_>>();
    if symbols.is_empty() {
        return Err(anyhow!("at least one symbol is required"));
    }

    let mut request = options
        .url
        .into_client_request()
        .with_context(|| format!("invalid websocket URL: {}", options.url))?;
    request.headers_mut().insert(
        "Origin",
        "https://finance.yahoo.com"
            .parse()
            .context("invalid websocket Origin header")?,
    );
    request.headers_mut().insert(
        "User-Agent",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36"
            .parse()
            .context("invalid websocket User-Agent header")?,
    );

    let proxy = selected_proxy(options.proxy, options.no_proxy);
    let (socket, _) = websocket::connect(request, proxy.as_deref())
        .await
        .context("Yahoo websocket connect failed")?;
    let (mut write, mut read) = socket.split();
    write
        .send(Message::Text(
            serde_json::to_string(&SubscribeMessage { subscribe: symbols })?.into(),
        ))
        .await
        .context("Yahoo websocket subscribe failed")?;

    let mut seen = 0usize;
    while options.message_limit == 0 || seen < options.message_limit {
        let Some(message) = tokio::time::timeout(options.read_timeout, read.next())
            .await
            .context("Yahoo websocket read timed out")?
        else {
            break;
        };
        let message = message.context("Yahoo websocket frame failed")?;
        match message {
            Message::Text(text) => {
                if let Ok(update) = decode_text_message(&text, options.timezone) {
                    on_quote(update)?;
                    seen += 1;
                }
            }
            Message::Binary(bytes) => {
                if let Ok(text) = std::str::from_utf8(&bytes)
                    && let Ok(update) = decode_text_message(text, options.timezone)
                {
                    on_quote(update)?;
                    seen += 1;
                    continue;
                }
                if let Ok(update) = decode_pricing_data(&bytes, options.timezone) {
                    on_quote(update)?;
                    seen += 1;
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
            _ => {}
        }
    }

    if seen == 0 {
        return Err(anyhow!(
            "Yahoo websocket returned no pricing updates before timeout"
        ));
    }
    Ok(())
}

pub fn decode_text_message(text: &str, timezone: &str) -> Result<StreamQuote> {
    let text = text.trim();
    let message = if text.starts_with('{') {
        serde_json::from_str::<serde_json::Value>(text)?
            .get("message")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("websocket JSON frame missing message field"))?
            .to_string()
    } else {
        text.to_string()
    };
    let bytes = general_purpose::STANDARD
        .decode(message.as_bytes())
        .context("websocket base64 decode failed")?;
    decode_pricing_data(&bytes, timezone)
}

fn decode_pricing_data(bytes: &[u8], timezone: &str) -> Result<StreamQuote> {
    let data = PricingData::decode(bytes).context("websocket protobuf decode failed")?;
    let time_utc = (data.time > 0)
        .then_some(data.time)
        .and_then(timestamp_ms_to_utc);
    Ok(StreamQuote {
        symbol: data.id,
        price: data.price as f64,
        time_local: utc_to_local(time_utc.as_deref(), timezone),
        time_utc,
        currency: non_empty(data.currency),
        exchange: non_empty(data.exchange),
        quote_type: Some(data.quote_type).filter(|value| *value != 0),
        market_hours: Some(data.market_hours).filter(|value| *value != 0),
        change_pct: non_zero_f32(data.change_percent),
        day_volume: Some(data.day_volume).filter(|value| *value != 0),
        day_high: non_zero_f32(data.day_high),
        day_low: non_zero_f32(data.day_low),
        change: non_zero_f32(data.change),
        short_name: non_empty(data.short_name),
        open: non_zero_f32(data.open_price),
        previous_close: non_zero_f32(data.previous_close),
        provider: "yahoo-websocket".to_string(),
    })
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn non_zero_f32(value: f32) -> Option<f64> {
    (value != 0.0).then_some(value as f64)
}

#[derive(Clone, PartialEq, ::prost::Message)]
struct PricingData {
    #[prost(string, tag = "1")]
    id: String,
    #[prost(float, tag = "2")]
    price: f32,
    #[prost(sint64, tag = "3")]
    time: i64,
    #[prost(string, tag = "4")]
    currency: String,
    #[prost(string, tag = "5")]
    exchange: String,
    #[prost(int32, tag = "6")]
    quote_type: i32,
    #[prost(int32, tag = "7")]
    market_hours: i32,
    #[prost(float, tag = "8")]
    change_percent: f32,
    #[prost(sint64, tag = "9")]
    day_volume: i64,
    #[prost(float, tag = "10")]
    day_high: f32,
    #[prost(float, tag = "11")]
    day_low: f32,
    #[prost(float, tag = "12")]
    change: f32,
    #[prost(string, tag = "13")]
    short_name: String,
    #[prost(sint64, tag = "14")]
    expire_date: i64,
    #[prost(float, tag = "15")]
    open_price: f32,
    #[prost(float, tag = "16")]
    previous_close: f32,
    #[prost(float, tag = "17")]
    strike_price: f32,
    #[prost(string, tag = "18")]
    underlying_symbol: String,
    #[prost(sint64, tag = "19")]
    open_interest: i64,
    #[prost(sint64, tag = "20")]
    options_type: i64,
    #[prost(sint64, tag = "21")]
    mini_option: i64,
    #[prost(sint64, tag = "22")]
    last_size: i64,
    #[prost(float, tag = "23")]
    bid: f32,
    #[prost(sint64, tag = "24")]
    bid_size: i64,
    #[prost(float, tag = "25")]
    ask: f32,
    #[prost(sint64, tag = "26")]
    ask_size: i64,
    #[prost(sint64, tag = "27")]
    price_hint: i64,
    #[prost(sint64, tag = "28")]
    vol_24hr: i64,
    #[prost(sint64, tag = "29")]
    vol_all_currencies: i64,
    #[prost(string, tag = "30")]
    from_currency: String,
    #[prost(string, tag = "31")]
    last_market: String,
    #[prost(double, tag = "32")]
    circulating_supply: f64,
    #[prost(double, tag = "33")]
    market_cap: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_yahoo_json_wrapped_base64_pricing_frame() {
        let frame = r#"{"message":"CgRDUkRPFQCATkMYgLaO8tBnIgNVU0QqCE5hc2RhcUdTMAg4AkVcjwrBSPa4ggFVwzVzQ109il9DZc3MnMFqEENyZWRvIFRlY2hub2xvZ3l9mhluQ4UBrgdsQw=="}"#;
        let quote = decode_text_message(frame, "Asia/Singapore").unwrap();
        assert_eq!(quote.symbol, "CRDO");
        assert_eq!(quote.price, 206.5);
        assert_eq!(quote.currency.as_deref(), Some("USD"));
        assert_eq!(quote.exchange.as_deref(), Some("NasdaqGS"));
        assert_eq!(quote.market_hours, Some(2));
        assert_eq!(quote.day_volume, Some(1_068_603));
        assert_eq!(quote.short_name.as_deref(), Some("Credo Technology"));
        assert_eq!(quote.time_utc.as_deref(), Some("2026-06-02T07:00:00Z"));
        assert_eq!(
            quote.time_local.as_deref(),
            Some("2026-06-02T15:00:00+08:00")
        );
    }
}
