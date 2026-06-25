use crate::model::{ProviderCapability, ProviderProfile};

pub fn crypto_provider_profiles() -> Vec<ProviderProfile> {
    vec![
        crypto_profile(
            "coinbase",
            "exchange-api",
            "Coinbase Exchange no-key crypto market data for products, tickers, stats, books, trades, and candles.",
            &[
                crypto_cap("quote", "yes", "ticker plus 24h stats and product metadata"),
                crypto_cap("history", "yes", "candles"),
                crypto_cap("order book", "yes", "level 1/2/3 product book"),
                crypto_cap("trades", "yes", "product trades"),
                crypto_cap("markets", "yes", "products and volume summary"),
                crypto_cap(
                    "funding/open interest",
                    "no",
                    "spot exchange; no perps funding/OI in this provider",
                ),
                crypto_cap("research", "no", "No issuer fundamentals/analysis"),
            ],
            &[
                "Use BTC-USD style symbols when a USD product exists; BTC/USDT may not exist on Coinbase.",
                "Good independent cross-check for spot price, book depth, and recent prints.",
            ],
        ),
        crypto_profile(
            "okx",
            "exchange-api",
            "OKX no-key crypto market data for spot/swap/futures instruments, tickers, books, trades, candles, funding, mark price, and open interest.",
            &[
                crypto_cap("quote", "yes", "ticker last price"),
                crypto_cap("history", "yes", "candles and historical candles"),
                crypto_cap("order book", "yes", "books"),
                crypto_cap("trades", "yes", "recent trades"),
                crypto_cap("markets", "yes", "instruments and tickers by instType"),
                crypto_cap("funding", "yes", "current and historical funding rates"),
                crypto_cap("open interest", "yes", "open interest plus mark price"),
                crypto_cap("research", "no", "No issuer fundamentals/analysis"),
            ],
            &[
                "Use --instrument spot/swap/futures/option on discovery and derivatives commands.",
                "Useful as a non-Binance derivative sentiment and price-discovery cross-check.",
            ],
        ),
        crypto_profile(
            "coingecko",
            "market-aggregator",
            "CoinGecko no-key aggregate crypto data for simple price, coin metadata, markets, tickers, OHLC, market charts, trending, global, exchanges, and derivatives discovery.",
            &[
                crypto_cap(
                    "quote",
                    "yes",
                    "simple price with market cap, volume, 24h change, plus coin metadata",
                ),
                crypto_cap("history", "yes", "OHLC and market chart windows"),
                crypto_cap(
                    "markets",
                    "yes",
                    "coins markets, coins list, and exchange tickers by coin",
                ),
                crypto_cap(
                    "trending/global",
                    "yes",
                    "trending search and global market data",
                ),
                crypto_cap("exchanges", "yes", "spot and derivatives exchange lists"),
                crypto_cap(
                    "funding/open interest",
                    "partial",
                    "derivatives discovery, not normalized per-symbol OI/funding",
                ),
                crypto_cap(
                    "research",
                    "partial",
                    "coin metadata, links, categories, and market aggregates",
                ),
            ],
            &[
                "Aggregator data is useful for breadth, trending, and cross-exchange context; verify execution-sensitive prices against exchange APIs.",
                "Free/no-key endpoints can be rate limited; COINGECKO_API_KEY or COINGECKO_DEMO_API_KEY is honored when present.",
            ],
        ),
    ]
}

fn crypto_profile(
    provider: &str,
    stability: &str,
    best_for: &str,
    capabilities: &[ProviderCapability],
    limitations: &[&str],
) -> ProviderProfile {
    ProviderProfile {
        provider: provider.to_string(),
        requires_api_key: false,
        official_status: "official-public-api".to_string(),
        stability: stability.to_string(),
        best_for: best_for.to_string(),
        large_download: false,
        capabilities: capabilities.to_vec(),
        limitations: limitations.iter().map(|value| value.to_string()).collect(),
    }
}

fn crypto_cap(module: &str, status: &str, note: &str) -> ProviderCapability {
    ProviderCapability {
        module: module.to_string(),
        status: status.to_string(),
        note: note.to_string(),
        implemented: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{CryptoDiscoverKind, CryptoInstrument, CryptoProvider};
    use crate::crypto_capability::{CryptoCapability, provider_supports};

    #[test]
    fn provider_profiles_do_not_claim_missing_crypto_runtime_capabilities() {
        for profile in crypto_provider_profiles() {
            let provider = match profile.provider.as_str() {
                "coinbase" => CryptoProvider::Coinbase,
                "okx" => CryptoProvider::Okx,
                "coingecko" => CryptoProvider::Coingecko,
                other => panic!("unmapped crypto provider profile: {other}"),
            };

            for capability in profile
                .capabilities
                .iter()
                .filter(|capability| capability.status == "yes")
            {
                for (instrument, runtime_capability) in profile_runtime_claims(&capability.module)
                    .unwrap_or_else(|| {
                        panic!(
                            "unmapped crypto profile capability {} for {}",
                            capability.module, profile.provider
                        )
                    })
                {
                    assert!(
                        provider_supports(provider, instrument, runtime_capability),
                        "{} claims {} but runtime does not support {:?} {:?}",
                        profile.provider,
                        capability.module,
                        instrument,
                        runtime_capability,
                    );
                }
            }
        }

        assert!(!provider_supports(
            CryptoProvider::Coingecko,
            CryptoInstrument::Spot,
            CryptoCapability::Trades,
        ));
    }

    fn profile_runtime_claims(module: &str) -> Option<Vec<(CryptoInstrument, CryptoCapability)>> {
        Some(match module {
            "quote" => vec![(CryptoInstrument::Spot, CryptoCapability::Quote)],
            "history" => vec![(CryptoInstrument::Spot, CryptoCapability::Candles)],
            "order book" => vec![(CryptoInstrument::Spot, CryptoCapability::Book)],
            "trades" => vec![(CryptoInstrument::Spot, CryptoCapability::Trades)],
            "markets" => vec![(
                CryptoInstrument::Spot,
                CryptoCapability::Discover(CryptoDiscoverKind::Markets),
            )],
            "funding" => vec![(CryptoInstrument::Swap, CryptoCapability::Funding)],
            "open interest" => vec![(CryptoInstrument::Swap, CryptoCapability::OpenInterest)],
            "trending/global" => vec![
                (
                    CryptoInstrument::Spot,
                    CryptoCapability::Discover(CryptoDiscoverKind::Trending),
                ),
                (
                    CryptoInstrument::Spot,
                    CryptoCapability::Discover(CryptoDiscoverKind::Global),
                ),
            ],
            "exchanges" => vec![(
                CryptoInstrument::Spot,
                CryptoCapability::Discover(CryptoDiscoverKind::Exchanges),
            )],
            _ => return None,
        })
    }
}
