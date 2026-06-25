use anyhow::{Result, anyhow};
use wreq::Client;

use crate::args::{HistoryAdjustment, Provider, StooqAsset, StooqMarket};
use crate::model::{HistoryBatch, Quote};

pub mod binance;
pub mod capabilities;
pub mod cnbc;
pub mod coinbase;
pub mod coingecko;
pub mod okx;
pub mod polymarket;
pub mod robinhood;
pub mod sec_edgar;
pub mod stooq;
pub mod yahoo;

#[derive(Debug, Clone)]
pub struct HistoryRequest {
    pub symbol: String,
    pub interval: String,
    pub range: String,
    pub limit: usize,
    pub extended_session: bool,
    pub adjustment: HistoryAdjustment,
    pub actions: bool,
    pub repair: bool,
    pub stooq_market: StooqMarket,
    pub stooq_asset: StooqAsset,
}

pub async fn fetch_quote_without_boats(
    client: &Client,
    symbol: &str,
    label: &str,
) -> Result<Quote> {
    let extended_error = match yahoo::fetch_extended_quote(client, symbol).await {
        Ok(quote) => return Ok(quote),
        Err(error) => error.to_string(),
    };
    let yahoo_error = match yahoo::fetch_quote(client, symbol).await {
        Ok(quote) => return Ok(quote),
        Err(error) => error.to_string(),
    };
    match stooq::fetch_quote(client, symbol).await {
        Ok(quote) => Ok(quote),
        Err(stooq_error) => Err(anyhow!(
            "{}: yahoo-extended: {}; yahoo: {}; stooq: {}",
            label,
            extended_error,
            yahoo_error,
            stooq_error
        )),
    }
}

pub async fn fetch_history(
    client: &Client,
    provider: Provider,
    request: &HistoryRequest,
) -> Result<HistoryBatch> {
    match provider {
        Provider::Yahoo => yahoo::fetch_history(client, request).await,
        Provider::YahooExtended => yahoo::fetch_extended_history(client, request).await,
        Provider::YahooBoats => Err(anyhow!(
            "yahoo-boats does not support history; use yahoo-extended for pre/post chart bars"
        )),
        Provider::Stooq => {
            stooq::fetch_history(
                client,
                &request.symbol,
                &request.interval,
                request.limit,
                request.stooq_market,
                request.stooq_asset,
            )
            .await
        }
        Provider::CnbcExtended => Err(anyhow!("cnbc-extended does not support history")),
        Provider::Robinhood => {
            robinhood::fetch_history(
                client,
                &request.symbol,
                &request.interval,
                &request.range,
                request.extended_session,
                request.limit,
            )
            .await
        }
        Provider::BinanceSpot | Provider::BinanceUsdsFutures => Err(anyhow!(
            "Binance crypto history requires the crypto-aware app path; use --asset crypto"
        )),
        Provider::Auto => {
            let yahoo_error = match yahoo::fetch_history(client, request).await {
                Ok(history) => return Ok(history),
                Err(error) => error.to_string(),
            };
            match stooq::fetch_history(
                client,
                &request.symbol,
                &request.interval,
                request.limit,
                request.stooq_market,
                request.stooq_asset,
            )
            .await
            {
                Ok(history) => Ok(history),
                Err(stooq_error) => Err(anyhow!("yahoo: {}; stooq: {}", yahoo_error, stooq_error)),
            }
        }
    }
}
