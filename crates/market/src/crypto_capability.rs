use crate::args::{CryptoDiscoverKind, CryptoInstrument, CryptoMarket, CryptoProvider};

#[derive(Clone, Copy, Debug)]
pub enum CryptoCapability {
    Quote,
    Book,
    Trades,
    Candles,
    Funding,
    OpenInterest,
    Discover(CryptoDiscoverKind),
}

impl CryptoCapability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::Book => "book",
            Self::Trades => "trades",
            Self::Candles => "candles",
            Self::Funding => "funding",
            Self::OpenInterest => "open-interest",
            Self::Discover(_) => "discover",
        }
    }

    const fn default_instrument(self) -> CryptoInstrument {
        match self {
            Self::Funding | Self::OpenInterest => CryptoInstrument::Swap,
            _ => CryptoInstrument::Spot,
        }
    }
}

pub fn resolve_instrument(
    instrument: CryptoInstrument,
    capability: CryptoCapability,
) -> CryptoInstrument {
    match instrument {
        CryptoInstrument::Auto => capability.default_instrument(),
        instrument => instrument,
    }
}

pub fn selected_providers(
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    capability: CryptoCapability,
) -> Vec<CryptoProvider> {
    match provider {
        CryptoProvider::Auto => all_providers()
            .into_iter()
            .filter(|candidate| provider_supports(*candidate, instrument, capability))
            .collect(),
        provider => vec![provider],
    }
}

pub fn provider_supports(
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    capability: CryptoCapability,
) -> bool {
    match capability {
        CryptoCapability::Quote | CryptoCapability::Candles => match instrument {
            CryptoInstrument::Spot => matches!(
                provider,
                CryptoProvider::Binance
                    | CryptoProvider::Coinbase
                    | CryptoProvider::Okx
                    | CryptoProvider::Coingecko
            ),
            CryptoInstrument::Swap | CryptoInstrument::Futures => {
                matches!(provider, CryptoProvider::Binance | CryptoProvider::Okx)
            }
            CryptoInstrument::Option => matches!(provider, CryptoProvider::Okx),
            CryptoInstrument::Auto => false,
        },
        CryptoCapability::Book | CryptoCapability::Trades => match instrument {
            CryptoInstrument::Spot => matches!(
                provider,
                CryptoProvider::Binance | CryptoProvider::Coinbase | CryptoProvider::Okx
            ),
            CryptoInstrument::Swap | CryptoInstrument::Futures => {
                matches!(provider, CryptoProvider::Binance | CryptoProvider::Okx)
            }
            CryptoInstrument::Option => matches!(provider, CryptoProvider::Okx),
            CryptoInstrument::Auto => false,
        },
        CryptoCapability::Funding => {
            instrument == CryptoInstrument::Swap
                && matches!(provider, CryptoProvider::Binance | CryptoProvider::Okx)
        }
        CryptoCapability::OpenInterest => match instrument {
            CryptoInstrument::Swap => {
                matches!(provider, CryptoProvider::Binance | CryptoProvider::Okx)
            }
            CryptoInstrument::Futures | CryptoInstrument::Option => {
                matches!(provider, CryptoProvider::Okx)
            }
            CryptoInstrument::Auto | CryptoInstrument::Spot => false,
        },
        CryptoCapability::Discover(kind) => discover_provider_supports(provider, instrument, kind),
    }
}

pub fn binance_market(instrument: CryptoInstrument) -> CryptoMarket {
    match instrument {
        CryptoInstrument::Spot | CryptoInstrument::Auto => CryptoMarket::Spot,
        CryptoInstrument::Swap | CryptoInstrument::Futures => CryptoMarket::UsdsFutures,
        CryptoInstrument::Option => CryptoMarket::Spot,
    }
}

const fn all_providers() -> [CryptoProvider; 4] {
    [
        CryptoProvider::Binance,
        CryptoProvider::Coinbase,
        CryptoProvider::Okx,
        CryptoProvider::Coingecko,
    ]
}

fn discover_provider_supports(
    provider: CryptoProvider,
    instrument: CryptoInstrument,
    kind: CryptoDiscoverKind,
) -> bool {
    match kind {
        CryptoDiscoverKind::Markets => match instrument {
            CryptoInstrument::Spot => matches!(
                provider,
                CryptoProvider::Binance
                    | CryptoProvider::Coinbase
                    | CryptoProvider::Okx
                    | CryptoProvider::Coingecko
            ),
            CryptoInstrument::Swap | CryptoInstrument::Futures | CryptoInstrument::Option => {
                matches!(provider, CryptoProvider::Okx)
            }
            CryptoInstrument::Auto => false,
        },
        CryptoDiscoverKind::Instruments => match instrument {
            CryptoInstrument::Spot => matches!(
                provider,
                CryptoProvider::Binance | CryptoProvider::Coinbase | CryptoProvider::Okx
            ),
            CryptoInstrument::Swap | CryptoInstrument::Futures | CryptoInstrument::Option => {
                matches!(provider, CryptoProvider::Okx)
            }
            CryptoInstrument::Auto => false,
        },
        CryptoDiscoverKind::Tickers => matches!(provider, CryptoProvider::Okx),
        CryptoDiscoverKind::VolumeSummary => matches!(provider, CryptoProvider::Coinbase),
        CryptoDiscoverKind::Trending
        | CryptoDiscoverKind::Global
        | CryptoDiscoverKind::Exchanges
        | CryptoDiscoverKind::Derivatives
        | CryptoDiscoverKind::DerivativesExchanges
        | CryptoDiscoverKind::CoinsList => matches!(provider, CryptoProvider::Coingecko),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_quote_uses_exchange_and_aggregator_spot_sources() {
        let providers = selected_providers(
            CryptoProvider::Auto,
            CryptoInstrument::Spot,
            CryptoCapability::Quote,
        );

        assert_eq!(
            providers,
            vec![
                CryptoProvider::Binance,
                CryptoProvider::Coinbase,
                CryptoProvider::Okx,
                CryptoProvider::Coingecko,
            ]
        );
    }

    #[test]
    fn auto_swap_quote_uses_derivatives_exchanges_only() {
        let providers = selected_providers(
            CryptoProvider::Auto,
            CryptoInstrument::Swap,
            CryptoCapability::Quote,
        );

        assert_eq!(
            providers,
            vec![CryptoProvider::Binance, CryptoProvider::Okx]
        );
    }

    #[test]
    fn coingecko_is_not_a_trade_provider() {
        assert!(!provider_supports(
            CryptoProvider::Coingecko,
            CryptoInstrument::Spot,
            CryptoCapability::Trades,
        ));
    }

    #[test]
    fn derivative_market_discovery_uses_okx_only() {
        let providers = selected_providers(
            CryptoProvider::Auto,
            CryptoInstrument::Swap,
            CryptoCapability::Discover(CryptoDiscoverKind::Markets),
        );

        assert_eq!(providers, vec![CryptoProvider::Okx]);
    }

    #[test]
    fn default_instrument_follows_capability_shape() {
        assert_eq!(
            resolve_instrument(CryptoInstrument::Auto, CryptoCapability::Quote),
            CryptoInstrument::Spot
        );
        assert_eq!(
            resolve_instrument(CryptoInstrument::Auto, CryptoCapability::Funding),
            CryptoInstrument::Swap
        );
        assert_eq!(
            resolve_instrument(CryptoInstrument::Auto, CryptoCapability::OpenInterest),
            CryptoInstrument::Swap
        );
    }
}
