use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParseEnumError {
    enum_name: &'static str,
    input: String,
    expected: &'static [&'static str],
}

impl ParseEnumError {
    fn new(enum_name: &'static str, input: &str, expected: &'static [&'static str]) -> Self {
        Self {
            enum_name,
            input: input.to_string(),
            expected,
        }
    }
}

impl fmt::Display for ParseEnumError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid {} '{}'; expected one of: {}",
            self.enum_name,
            self.input,
            self.expected.join(", ")
        )
    }
}

impl std::error::Error for ParseEnumError {}

macro_rules! text_enum {
    ($enum:ident, $name:literal, [$($variant:ident => $label:literal $(| $alias:literal)*),+ $(,)?]) => {
        impl $enum {
            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                }
            }

            pub const fn labels() -> &'static [&'static str] {
                &[$($label),+]
            }
        }

        impl fmt::Display for $enum {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.label())
            }
        }

        impl FromStr for $enum {
            type Err = ParseEnumError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                let normalized = input.trim().to_ascii_lowercase();
                match normalized.as_str() {
                    $($label $(| $alias)* => Ok(Self::$variant),)+
                    _ => Err(ParseEnumError::new($name, input, Self::labels())),
                }
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMode {
    Smart,
    Regular,
    Extended,
    Overnight,
    All,
}

text_enum!(
    SessionMode,
    "session mode",
    [
        Smart => "smart",
        Regular => "regular",
        Extended => "extended",
        Overnight => "overnight",
        All => "all",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetClass {
    Auto,
    Equity,
    Crypto,
}

text_enum!(
    AssetClass,
    "asset class",
    [
        Auto => "auto",
        Equity => "equity",
        Crypto => "crypto",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistorySession {
    Regular,
    Extended,
}

text_enum!(
    HistorySession,
    "history session",
    [
        Regular => "regular",
        Extended => "extended",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StooqFrequency {
    Daily,
    Hourly,
    FiveMin,
}

text_enum!(
    StooqFrequency,
    "stooq frequency",
    [
        Daily => "daily" | "1d",
        Hourly => "hourly" | "1h" | "60m",
        FiveMin => "5m" | "five-min" | "5minute",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StooqMarket {
    Us,
    World,
    Macro,
}

text_enum!(
    StooqMarket,
    "stooq market",
    [
        Us => "us",
        World => "world",
        Macro => "macro",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StooqAsset {
    Stocks,
    Etfs,
    Currencies,
    Crypto,
    Macro,
}

text_enum!(
    StooqAsset,
    "stooq asset",
    [
        Stocks => "stocks",
        Etfs => "etfs",
        Currencies => "currencies",
        Crypto => "crypto",
        Macro => "macro",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryAdjustment {
    /// Raw Yahoo close/OHLC with Adj Close preserved separately when available.
    Raw,
    /// Adjust OHLC and Close by Adj Close / Close, matching yfinance auto_adjust.
    Auto,
    /// Adjust OHLC by Adj Close / Close while keeping raw Close, matching yfinance back_adjust.
    Back,
}

text_enum!(
    HistoryAdjustment,
    "history adjustment",
    [
        Raw => "raw",
        Auto => "auto",
        Back => "back",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Provider {
    Auto,
    Yahoo,
    YahooExtended,
    YahooBoats,
    Stooq,
    CnbcExtended,
    Robinhood,
    BinanceSpot,
    BinanceUsdsFutures,
}

text_enum!(
    Provider,
    "provider",
    [
        Auto => "auto",
        Yahoo => "yahoo",
        YahooExtended => "yahoo-extended",
        YahooBoats => "yahoo-boats",
        Stooq => "stooq",
        CnbcExtended => "cnbc-extended",
        Robinhood => "robinhood",
        BinanceSpot => "binance-spot",
        BinanceUsdsFutures => "binance-usds-futures",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionsProvider {
    Auto,
    Yahoo,
    Robinhood,
}

text_enum!(
    OptionsProvider,
    "options provider",
    [
        Auto => "auto",
        Yahoo => "yahoo",
        Robinhood => "robinhood",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchProvider {
    Auto,
    Yahoo,
    SecEdgar,
    Robinhood,
    Cnbc,
}

text_enum!(
    ResearchProvider,
    "research provider",
    [
        Auto => "auto",
        Yahoo => "yahoo",
        SecEdgar => "sec-edgar",
        Robinhood => "robinhood",
        Cnbc => "cnbc",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadUrlProvider {
    Auto,
    Direct,
    Defuddle,
    Jina,
}

text_enum!(
    ReadUrlProvider,
    "read-url provider",
    [
        Auto => "auto",
        Direct => "direct",
        Defuddle => "defuddle",
        Jina => "jina",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoDiscoverKind {
    Markets,
    Instruments,
    Tickers,
    Trending,
    Global,
    Exchanges,
    Derivatives,
    DerivativesExchanges,
    VolumeSummary,
    CoinsList,
}

text_enum!(
    CryptoDiscoverKind,
    "crypto discover kind",
    [
        Markets => "markets",
        Instruments => "instruments",
        Tickers => "tickers",
        Trending => "trending",
        Global => "global",
        Exchanges => "exchanges",
        Derivatives => "derivatives",
        DerivativesExchanges => "derivatives-exchanges",
        VolumeSummary => "volume-summary",
        CoinsList => "coins-list",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoProvider {
    Auto,
    Binance,
    Coinbase,
    Okx,
    Coingecko,
}

text_enum!(
    CryptoProvider,
    "crypto provider",
    [
        Auto => "auto",
        Binance => "binance",
        Coinbase => "coinbase",
        Okx => "okx",
        Coingecko => "coingecko",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoMarket {
    Auto,
    Spot,
    UsdsFutures,
}

text_enum!(
    CryptoMarket,
    "crypto market",
    [
        Auto => "auto",
        Spot => "spot",
        UsdsFutures => "usds-futures",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoInstrument {
    Auto,
    Spot,
    Swap,
    Futures,
    Option,
}

text_enum!(
    CryptoInstrument,
    "crypto instrument",
    [
        Auto => "auto",
        Spot => "spot",
        Swap => "swap",
        Futures => "futures",
        Option => "option",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CryptoStreamKind {
    Trade,
    Kline,
    Depth,
    BookTicker,
    MarkPrice,
}

text_enum!(
    CryptoStreamKind,
    "crypto stream kind",
    [
        Trade => "trade",
        Kline => "kline",
        Depth => "depth",
        BookTicker => "book-ticker",
        MarkPrice => "mark-price",
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuturesPeriod {
    FiveMin,
    FifteenMin,
    ThirtyMin,
    OneHour,
    TwoHour,
    FourHour,
    SixHour,
    TwelveHour,
    OneDay,
}

text_enum!(
    FuturesPeriod,
    "futures period",
    [
        FiveMin => "5m",
        FifteenMin => "15m",
        ThirtyMin => "30m",
        OneHour => "1h",
        TwoHour => "2h",
        FourHour => "4h",
        SixHour => "6h",
        TwelveHour => "12h",
        OneDay => "1d",
    ]
);
