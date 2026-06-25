use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::args::{CryptoInstrument, CryptoProvider, Provider};
use crate::crypto_capability::{
    CryptoCapability, binance_market, provider_supports, resolve_instrument, selected_providers,
};
use crate::indicators::compute_indicator;
use crate::model::{DerivedIndicator, HistoryBatch, PricePoint, Quote};
use crate::price;
use crate::providers::{binance, coinbase, coingecko, okx};

pub async fn fetch_indicator_batch(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    options: CryptoIndicatorOptions<'_>,
) -> Result<IndicatorBatch> {
    let resolved_provider = provider_crypto_provider(options.provider, options.crypto_provider);
    let resolved_instrument = provider_instrument(options.provider, options.instrument);
    let mut indicators = Vec::new();
    let mut errors = BTreeMap::new();
    for symbol in options.symbols {
        match fetch_history(
            client,
            config,
            resolved_provider,
            resolved_instrument,
            &symbol,
            options.interval,
            options.limit,
        )
        .await
        {
            Ok(history) => indicators.push(compute_indicator(&history)),
            Err(error) => {
                errors.insert(symbol, format!("{error:#}"));
            }
        }
    }
    Ok(IndicatorBatch { indicators, errors })
}

pub async fn fetch_price_batch(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    symbols: Vec<String>,
    timezone: &str,
) -> CryptoPriceBatch {
    let mut points = Vec::new();
    let mut errors = BTreeMap::new();
    for symbol in symbols {
        match fetch_quote(client, config, provider, instrument, &symbol).await {
            Ok(quote) => points.push(price::quote_to_point(
                quote,
                "Crypto price",
                timezone,
                Some("Crypto markets trade 24/7; this is not an equity session quote".to_string()),
            )),
            Err(error) => {
                errors.insert(symbol, format!("{error:#}"));
            }
        }
    }
    CryptoPriceBatch { points, errors }
}

pub async fn fetch_quote(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    symbol: &str,
) -> Result<Quote> {
    if !provider_supports(provider, instrument, CryptoCapability::Quote)
        && provider != CryptoProvider::Auto
    {
        return Err(anyhow!(
            "provider {} does not support capability=quote instrument={}",
            provider.label(),
            instrument.label()
        ));
    }
    match provider {
        CryptoProvider::Auto => {
            let mut errors = Vec::new();
            for provider in selected_providers(provider, instrument, CryptoCapability::Quote) {
                match fetch_quote_one(client, config, provider, instrument, symbol).await {
                    Ok(quote) => return Ok(quote),
                    Err(error) => errors.push(format!("{}: {error:#}", provider.label())),
                }
            }
            Err(anyhow!("{}", errors.join("; ")))
        }
        provider => fetch_quote_one(client, config, provider, instrument, symbol).await,
    }
}

async fn fetch_quote_one(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    symbol: &str,
) -> Result<Quote> {
    match provider {
        CryptoProvider::Binance => {
            binance::fetch_quote(config, binance_market(instrument), symbol).await
        }
        CryptoProvider::Coinbase => coinbase::fetch_quote(client, symbol).await,
        CryptoProvider::Okx => okx::fetch_quote(client, symbol, instrument).await,
        CryptoProvider::Coingecko => coingecko::fetch_quote(client, symbol).await,
        CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
    }
}

pub async fn fetch_history(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    if !provider_supports(provider, instrument, CryptoCapability::Candles)
        && provider != CryptoProvider::Auto
    {
        return Err(anyhow!(
            "provider {} does not support capability=candles instrument={}",
            provider.label(),
            instrument.label()
        ));
    }
    match provider {
        CryptoProvider::Auto => {
            let mut errors = Vec::new();
            for provider in selected_providers(provider, instrument, CryptoCapability::Candles) {
                match fetch_history_one(
                    client, config, provider, instrument, symbol, interval, limit,
                )
                .await
                {
                    Ok(history) => return Ok(history),
                    Err(error) => errors.push(format!("{}: {error:#}", provider.label())),
                }
            }
            Err(anyhow!("{}", errors.join("; ")))
        }
        provider => {
            fetch_history_one(
                client, config, provider, instrument, symbol, interval, limit,
            )
            .await
        }
    }
}

async fn fetch_history_one(
    client: &wreq::Client,
    config: &binance::BinanceConfig,
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    symbol: &str,
    interval: &str,
    limit: usize,
) -> Result<HistoryBatch> {
    match provider {
        CryptoProvider::Binance => {
            binance::fetch_history(config, binance_market(instrument), symbol, interval, limit)
                .await
        }
        CryptoProvider::Coinbase => coinbase::fetch_history(client, symbol, interval, limit).await,
        CryptoProvider::Okx => {
            okx::fetch_history(client, symbol, instrument, interval, limit).await
        }
        CryptoProvider::Coingecko => {
            coingecko::fetch_history(client, symbol, interval, limit).await
        }
        CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
    }
}

fn provider_instrument(provider: Provider, instrument: CryptoInstrument) -> CryptoInstrument {
    match provider {
        Provider::BinanceSpot => CryptoInstrument::Spot,
        Provider::BinanceUsdsFutures => CryptoInstrument::Swap,
        _ => resolve_instrument(instrument, CryptoCapability::Candles),
    }
}

fn provider_crypto_provider(provider: Provider, crypto_provider: CryptoProvider) -> CryptoProvider {
    match provider {
        Provider::BinanceSpot | Provider::BinanceUsdsFutures => CryptoProvider::Binance,
        _ => crypto_provider,
    }
}

#[derive(Debug, Serialize)]
pub struct CryptoPriceBatch {
    pub points: Vec<PricePoint>,
    pub errors: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct IndicatorBatch {
    pub indicators: Vec<DerivedIndicator>,
    pub errors: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct CryptoIndicatorOptions<'a> {
    pub symbols: Vec<String>,
    pub provider: Provider,
    pub crypto_provider: CryptoProvider,
    pub instrument: CryptoInstrument,
    pub interval: &'a str,
    pub limit: usize,
}
