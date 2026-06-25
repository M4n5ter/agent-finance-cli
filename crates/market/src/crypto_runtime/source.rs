use anyhow::Result;

use crate::args::{CryptoDiscoverKind, CryptoInstrument, CryptoProvider};
use crate::crypto_capability::binance_market;
use crate::http::http_client;
use crate::providers::{binance, coinbase, coingecko, okx};

use super::evidence::{
    ProviderEvidence, collect_endpoint_evidence, provider_from_endpoints, required_endpoint,
    required_payload, required_value, supplemental_endpoint,
};

#[derive(Clone)]
pub struct CryptoEvidenceSources {
    client: wreq::Client,
    binance: binance::BinanceConfig,
}

impl CryptoEvidenceSources {
    pub fn new(proxy: Option<&str>, no_proxy: bool, timeout_seconds: u64) -> Result<Self> {
        Ok(Self {
            client: http_client(timeout_seconds, proxy, no_proxy)?,
            binance: binance::BinanceConfig::from_env(timeout_seconds, proxy, no_proxy),
        })
    }

    pub async fn quote(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![required_payload(
                    "quote",
                    binance::fetch_quote(&self.binance, binance_market(instrument), &symbol).await,
                )],
            ),
            CryptoProvider::Coinbase => provider_from_endpoints(
                "coinbase",
                collect_endpoint_evidence(vec![
                    required_endpoint("ticker", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        async move { coinbase::ticker(&client, &symbol).await }
                    }),
                    supplemental_endpoint("stats", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        async move { coinbase::stats(&client, &symbol).await }
                    }),
                    supplemental_endpoint("product", {
                        let client = self.client.clone();
                        async move { coinbase::product(&client, &symbol).await }
                    }),
                ])
                .await,
            ),
            CryptoProvider::Okx => provider_from_endpoints(
                "okx",
                vec![required_value(
                    "ticker",
                    okx::ticker(&self.client, &symbol, instrument).await,
                )],
            ),
            CryptoProvider::Coingecko => provider_from_endpoints(
                "coingecko",
                collect_endpoint_evidence(vec![
                    required_endpoint("simple-price", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        async move { coingecko::simple_price(&client, &symbol).await }
                    }),
                    supplemental_endpoint("coin", {
                        let client = self.client.clone();
                        async move { coingecko::coin(&client, &symbol).await }
                    }),
                ])
                .await,
            ),
            CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
        }
    }

    pub async fn book(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
        limit: usize,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![match instrument {
                    CryptoInstrument::Swap | CryptoInstrument::Futures => required_payload(
                        "book",
                        binance::futures_book(&self.binance, &symbol, limit).await,
                    ),
                    _ => required_payload(
                        "book",
                        binance::spot_book(&self.binance, &symbol, limit).await,
                    ),
                }],
            ),
            CryptoProvider::Coinbase => provider_from_endpoints(
                "coinbase",
                vec![required_value(
                    "book",
                    coinbase::book(&self.client, &symbol, limit).await,
                )],
            ),
            CryptoProvider::Okx => provider_from_endpoints(
                "okx",
                vec![required_value(
                    "book",
                    okx::book(&self.client, &symbol, instrument, limit).await,
                )],
            ),
            CryptoProvider::Coingecko => {
                unreachable!("unsupported provider handled by evidence engine")
            }
            CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
        }
    }

    pub async fn trades(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
        limit: usize,
        aggregate: bool,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![match instrument {
                    CryptoInstrument::Swap | CryptoInstrument::Futures => required_payload(
                        "trades",
                        binance::futures_trades(&self.binance, &symbol, limit).await,
                    ),
                    _ => required_payload(
                        "trades",
                        binance::spot_trades(&self.binance, &symbol, limit, aggregate).await,
                    ),
                }],
            ),
            CryptoProvider::Coinbase => provider_from_endpoints(
                "coinbase",
                vec![required_value(
                    "trades",
                    coinbase::trades(&self.client, &symbol, limit).await,
                )],
            ),
            CryptoProvider::Okx => provider_from_endpoints(
                "okx",
                vec![required_value(
                    "trades",
                    okx::trades(&self.client, &symbol, instrument, limit).await,
                )],
            ),
            CryptoProvider::Coingecko => {
                unreachable!("unsupported provider handled by evidence engine")
            }
            CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
        }
    }

    pub async fn candles(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
        interval: String,
        limit: usize,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![match instrument {
                    CryptoInstrument::Swap | CryptoInstrument::Futures => required_payload(
                        "klines",
                        binance::futures_klines(&self.binance, &symbol, &interval, limit).await,
                    ),
                    _ => required_payload(
                        "klines",
                        binance::spot_klines(&self.binance, &symbol, &interval, limit).await,
                    ),
                }],
            ),
            CryptoProvider::Coinbase => provider_from_endpoints(
                "coinbase",
                vec![required_value(
                    "candles",
                    coinbase::candles(&self.client, &symbol, &interval, limit).await,
                )],
            ),
            CryptoProvider::Okx => {
                provider_from_endpoints(
                    "okx",
                    collect_endpoint_evidence(vec![
                        required_endpoint("candles", {
                            let client = self.client.clone();
                            let symbol = symbol.clone();
                            let interval = interval.clone();
                            async move {
                                okx::candles(&client, &symbol, instrument, &interval, limit).await
                            }
                        }),
                        supplemental_endpoint("history-candles", {
                            let client = self.client.clone();
                            async move {
                                okx::history_candles(&client, &symbol, instrument, &interval, limit)
                                    .await
                            }
                        }),
                    ])
                    .await,
                )
            }
            CryptoProvider::Coingecko => provider_from_endpoints(
                "coingecko",
                collect_endpoint_evidence(vec![
                    required_endpoint("ohlc", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        let interval = interval.clone();
                        async move { coingecko::ohlc(&client, &symbol, &interval, limit).await }
                    }),
                    supplemental_endpoint("market-chart", {
                        let client = self.client.clone();
                        async move { coingecko::market_chart(&client, &symbol, "1", limit).await }
                    }),
                ])
                .await,
            ),
            CryptoProvider::Auto => unreachable!("auto must be expanded before provider dispatch"),
        }
    }

    pub async fn funding(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
        limit: usize,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![required_payload(
                    "funding",
                    binance::futures_funding(&self.binance, &symbol, limit).await,
                )],
            ),
            CryptoProvider::Okx => provider_from_endpoints(
                "okx",
                collect_endpoint_evidence(vec![
                    required_endpoint("funding-rate", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        async move { okx::funding_rate(&client, &symbol, instrument).await }
                    }),
                    supplemental_endpoint("funding-rate-history", {
                        let client = self.client.clone();
                        async move {
                            okx::funding_rate_history(&client, &symbol, instrument, limit).await
                        }
                    }),
                ])
                .await,
            ),
            provider => unreachable!(
                "unsupported {} funding provider handled by evidence engine",
                provider.label()
            ),
        }
    }

    pub async fn open_interest(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        symbol: String,
    ) -> ProviderEvidence {
        match provider {
            CryptoProvider::Binance => provider_from_endpoints(
                "binance",
                vec![required_payload(
                    "open-interest",
                    binance::futures_open_interest(&self.binance, &symbol).await,
                )],
            ),
            CryptoProvider::Okx => provider_from_endpoints(
                "okx",
                collect_endpoint_evidence(vec![
                    required_endpoint("open-interest", {
                        let client = self.client.clone();
                        let symbol = symbol.clone();
                        async move { okx::open_interest(&client, &symbol, instrument).await }
                    }),
                    supplemental_endpoint("mark-price", {
                        let client = self.client.clone();
                        async move { okx::mark_price(&client, &symbol, instrument).await }
                    }),
                ])
                .await,
            ),
            provider => unreachable!(
                "unsupported {} open-interest provider handled by evidence engine",
                provider.label()
            ),
        }
    }

    pub async fn discover(
        &self,
        provider: CryptoProvider,
        instrument: CryptoInstrument,
        kind: CryptoDiscoverKind,
        limit: usize,
        vs_currency: String,
    ) -> ProviderEvidence {
        match (provider, kind) {
            (
                CryptoProvider::Binance,
                CryptoDiscoverKind::Markets | CryptoDiscoverKind::Instruments,
            ) => provider_from_endpoints(
                "binance",
                vec![required_payload(
                    "exchange-info",
                    binance::spot_exchange_info(&self.binance, None).await,
                )],
            ),
            (
                CryptoProvider::Coinbase,
                CryptoDiscoverKind::Markets | CryptoDiscoverKind::Instruments,
            ) => provider_from_endpoints(
                "coinbase",
                vec![required_value(
                    "products",
                    coinbase::products(&self.client).await,
                )],
            ),
            (CryptoProvider::Coinbase, CryptoDiscoverKind::VolumeSummary) => {
                provider_from_endpoints(
                    "coinbase",
                    vec![required_value(
                        "volume-summary",
                        coinbase::volume_summary(&self.client).await,
                    )],
                )
            }
            (
                CryptoProvider::Okx,
                CryptoDiscoverKind::Markets | CryptoDiscoverKind::Instruments,
            ) => provider_from_endpoints(
                "okx",
                vec![required_value(
                    "instruments",
                    okx::instruments(&self.client, instrument).await,
                )],
            ),
            (CryptoProvider::Okx, CryptoDiscoverKind::Tickers) => provider_from_endpoints(
                "okx",
                vec![required_value(
                    "tickers",
                    okx::tickers(&self.client, instrument).await,
                )],
            ),
            (CryptoProvider::Coingecko, CryptoDiscoverKind::Markets) => provider_from_endpoints(
                "coingecko",
                vec![required_value(
                    "markets",
                    coingecko::markets(&self.client, &vs_currency, limit).await,
                )],
            ),
            (CryptoProvider::Coingecko, CryptoDiscoverKind::Trending) => provider_from_endpoints(
                "coingecko",
                vec![required_value(
                    "trending",
                    coingecko::trending(&self.client).await,
                )],
            ),
            (CryptoProvider::Coingecko, CryptoDiscoverKind::Global) => provider_from_endpoints(
                "coingecko",
                vec![required_value(
                    "global",
                    coingecko::global(&self.client).await,
                )],
            ),
            (CryptoProvider::Coingecko, CryptoDiscoverKind::Exchanges) => provider_from_endpoints(
                "coingecko",
                vec![required_value(
                    "exchanges",
                    coingecko::exchanges(&self.client, limit).await,
                )],
            ),
            (CryptoProvider::Coingecko, CryptoDiscoverKind::Derivatives) => {
                provider_from_endpoints(
                    "coingecko",
                    vec![required_value(
                        "derivatives",
                        coingecko::derivatives(&self.client, limit).await,
                    )],
                )
            }
            (CryptoProvider::Coingecko, CryptoDiscoverKind::DerivativesExchanges) => {
                provider_from_endpoints(
                    "coingecko",
                    vec![required_value(
                        "derivatives-exchanges",
                        coingecko::derivatives_exchanges(&self.client, limit).await,
                    )],
                )
            }
            (CryptoProvider::Coingecko, CryptoDiscoverKind::CoinsList) => provider_from_endpoints(
                "coingecko",
                vec![required_value(
                    "coins-list",
                    coingecko::coins_list(&self.client, limit).await,
                )],
            ),
            (provider, _) => unreachable!(
                "unsupported {} discover provider handled by evidence engine",
                provider.label()
            ),
        }
    }
}
