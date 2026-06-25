use std::fmt;

use anyhow::Result;
use wreq::Client;

use crate::http::{http_client, selected_proxy};

use super::RestMarket;

const SPOT_BASE_URL: &str = "https://api.binance.com";
const SPOT_DATA_BASE_URL: &str = "https://data-api.binance.vision";
const FUTURES_BASE_URL: &str = "https://fapi.binance.com";
const SPOT_WS_URL: &str = "wss://data-stream.binance.vision/ws";
const FUTURES_WS_URL: &str = "wss://fstream.binance.com";

#[derive(Clone)]
pub struct BinanceConfig {
    pub timeout_seconds: u64,
    pub proxy: Option<String>,
    pub no_proxy: bool,
    pub spot_base_url: String,
    pub spot_base_url_overridden: bool,
    pub futures_base_url: String,
    pub spot_ws_url: String,
    pub futures_ws_url: String,
    pub api_key: Option<String>,
}

impl fmt::Debug for BinanceConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BinanceConfig")
            .field("timeout_seconds", &self.timeout_seconds)
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("no_proxy", &self.no_proxy)
            .field("spot_base_url", &self.spot_base_url)
            .field("spot_base_url_overridden", &self.spot_base_url_overridden)
            .field("futures_base_url", &self.futures_base_url)
            .field("spot_ws_url", &self.spot_ws_url)
            .field("futures_ws_url", &self.futures_ws_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl BinanceConfig {
    pub fn from_env(timeout_seconds: u64, proxy: Option<&str>, no_proxy: bool) -> Self {
        let spot_base_url = std::env::var("BINANCE_SPOT_BASE_URL").ok();
        Self {
            timeout_seconds,
            proxy: selected_proxy(proxy, no_proxy),
            no_proxy,
            spot_base_url: spot_base_url
                .clone()
                .unwrap_or_else(|| SPOT_BASE_URL.to_string()),
            spot_base_url_overridden: spot_base_url.is_some(),
            futures_base_url: std::env::var("BINANCE_FUTURES_BASE_URL")
                .unwrap_or_else(|_| FUTURES_BASE_URL.to_string()),
            spot_ws_url: std::env::var("BINANCE_SPOT_WS_URL")
                .unwrap_or_else(|_| SPOT_WS_URL.to_string()),
            futures_ws_url: std::env::var("BINANCE_FUTURES_WS_URL")
                .unwrap_or_else(|_| FUTURES_WS_URL.to_string()),
            api_key: std::env::var("BINANCE_API_KEY").ok(),
        }
    }

    pub(super) fn client(&self) -> Result<Client> {
        http_client(
            self.timeout_seconds,
            self.proxy.as_deref(),
            self.no_proxy || self.proxy.is_none(),
        )
    }

    pub(super) fn base_urls(&self, market: RestMarket) -> Vec<&str> {
        match market {
            RestMarket::Spot if self.spot_base_url_overridden => vec![&self.spot_base_url],
            RestMarket::Spot => vec![&self.spot_base_url, SPOT_DATA_BASE_URL],
            RestMarket::Futures => vec![&self.futures_base_url],
        }
    }

    pub(super) fn websocket_url(&self, market: RestMarket, path: &str) -> String {
        let base = match market {
            RestMarket::Spot => &self.spot_ws_url,
            RestMarket::Futures => &self.futures_ws_url,
        };
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_credentials_and_proxy() {
        let config = BinanceConfig {
            timeout_seconds: 10,
            proxy: Some("http://user:secret@127.0.0.1:7890".to_string()),
            no_proxy: false,
            spot_base_url: "https://api.binance.com".to_string(),
            spot_base_url_overridden: false,
            futures_base_url: "https://fapi.binance.com".to_string(),
            spot_ws_url: "wss://data-stream.binance.vision/ws".to_string(),
            futures_ws_url: "wss://fstream.binance.com".to_string(),
            api_key: Some("live-api-key".to_string()),
        };

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("live-api-key"));
        assert!(!debug.contains("user:secret"));
    }
}
