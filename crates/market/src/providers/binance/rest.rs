use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use super::config::BinanceConfig;
use super::types::{CryptoEndpointReport, RestMarket, market_name, report_from_value};
use crate::http::send_get_text_from_base_urls;

pub(super) async fn endpoint_report(
    config: &BinanceConfig,
    market: RestMarket,
    endpoint: &str,
    path: &str,
    symbol: Option<String>,
    params: Vec<(&'static str, String)>,
) -> Result<CryptoEndpointReport> {
    let payload = rest_get(config, market, path, params).await?;
    Ok(report_from_value(
        market,
        endpoint,
        symbol,
        200,
        Vec::new(),
        payload,
    ))
}

pub(super) async fn rest_get(
    config: &BinanceConfig,
    market: RestMarket,
    path: &str,
    params: Vec<(&'static str, String)>,
) -> Result<Value> {
    let client = config.client()?;
    let headers = config
        .api_key
        .as_ref()
        .map(|api_key| vec![("X-MBX-APIKEY", api_key.clone())])
        .unwrap_or_default();
    let (url, status, body) = send_get_text_from_base_urls(
        &client,
        "Binance",
        market_name(market),
        &config.base_urls(market),
        path,
        &params,
        &headers,
    )
    .await?;
    if !status.is_success() {
        return Err(anyhow!(
            "Binance request failed status={} body={body}",
            status.as_u16()
        ));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("Binance response JSON decode failed: {url}"))
}
