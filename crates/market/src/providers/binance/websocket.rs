use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::args::{CryptoMarket, CryptoStreamKind};
use crate::http::utc_now;
use crate::websocket;

use super::config::BinanceConfig;
use super::symbol::{ensure_futures_interval, ensure_spot_interval, normalize_symbol, rest_market};
use super::types::{CryptoStreamReport, RestMarket, provider};

pub async fn stream_messages(
    config: &BinanceConfig,
    market: CryptoMarket,
    kind: CryptoStreamKind,
    symbol: &str,
    interval: &str,
    messages: usize,
) -> Result<CryptoStreamReport> {
    let symbol = normalize_symbol(symbol)?;
    let effective_market = match (market, kind) {
        (CryptoMarket::Auto, CryptoStreamKind::MarkPrice) => CryptoMarket::UsdsFutures,
        (CryptoMarket::Auto, _) => CryptoMarket::Spot,
        (market, _) => market,
    };
    let stream_path = stream_path(effective_market, kind, &symbol, interval)?;
    let market = rest_market(effective_market)?;
    let url = config.websocket_url(market, &stream_path);
    let messages = collect_websocket_messages(config, &url, messages).await?;
    Ok(CryptoStreamReport {
        symbol,
        provider: provider(market).to_string(),
        market: super::types::market_name(market).to_string(),
        kind: format!("{kind:?}").to_ascii_lowercase(),
        interval: matches!(kind, CryptoStreamKind::Kline).then(|| interval.to_string()),
        fetched_at_utc: utc_now(),
        messages,
    })
}

pub(super) fn stream_path(
    market: CryptoMarket,
    kind: CryptoStreamKind,
    symbol: &str,
    interval: &str,
) -> Result<String> {
    let symbol = symbol.to_ascii_lowercase();
    Ok(match (rest_market(market)?, kind) {
        (RestMarket::Spot, CryptoStreamKind::Trade) => format!("{symbol}@trade"),
        (RestMarket::Spot, CryptoStreamKind::Kline) => {
            ensure_spot_interval(interval)?;
            format!("{symbol}@kline_{interval}")
        }
        (RestMarket::Spot, CryptoStreamKind::Depth) => format!("{symbol}@depth@100ms"),
        (RestMarket::Spot, CryptoStreamKind::BookTicker) => format!("{symbol}@bookTicker"),
        (RestMarket::Spot, CryptoStreamKind::MarkPrice) => {
            return Err(anyhow!("mark-price is a Binance USD-M Futures stream"));
        }
        (RestMarket::Futures, CryptoStreamKind::Trade) => format!("market/ws/{symbol}@aggTrade"),
        (RestMarket::Futures, CryptoStreamKind::Kline) => {
            ensure_futures_interval(interval)?;
            format!("market/ws/{symbol}@kline_{interval}")
        }
        (RestMarket::Futures, CryptoStreamKind::Depth) => {
            format!("public/ws/{symbol}@depth@100ms")
        }
        (RestMarket::Futures, CryptoStreamKind::BookTicker) => {
            format!("public/ws/{symbol}@bookTicker")
        }
        (RestMarket::Futures, CryptoStreamKind::MarkPrice) => {
            format!("market/ws/{symbol}@markPrice@1s")
        }
    })
}

async fn collect_websocket_messages(
    config: &BinanceConfig,
    url: &str,
    message_limit: usize,
) -> Result<Vec<Value>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut request = url
        .into_client_request()
        .with_context(|| format!("invalid Binance websocket URL: {url}"))?;
    request.headers_mut().insert(
        "User-Agent",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36"
            .parse()
            .context("invalid websocket User-Agent header")?,
    );
    let (socket, _) = websocket::connect(request, config.proxy.as_deref())
        .await
        .context("Binance websocket connect failed")?;
    let (_, mut read) = socket.split();
    let target = message_limit.max(1);
    let mut messages = Vec::with_capacity(target);
    while messages.len() < target {
        let frame = timeout(
            Duration::from_secs(config.timeout_seconds.max(1)),
            read.next(),
        )
        .await
        .context("timed out waiting for Binance WebSocket message")?
        .context("Binance WebSocket closed before enough messages arrived")?
        .context("Binance WebSocket frame failed")?;
        match frame {
            Message::Text(text) => {
                let text = text.to_string();
                messages
                    .push(serde_json::from_str(&text).unwrap_or_else(|_| json!({ "text": text })));
            }
            Message::Binary(bytes) => {
                messages.push(json!({ "binary_base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes) }));
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
            _ => {}
        }
    }
    if messages.is_empty() {
        return Err(anyhow!(
            "Binance WebSocket returned no messages before timeout"
        ));
    }
    Ok(messages)
}
