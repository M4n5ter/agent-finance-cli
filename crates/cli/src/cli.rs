use std::path::PathBuf;
use std::str::FromStr;

pub use agent_finance_market::{
    AssetClass, HistoryAdjustment, HistorySession, OptionsProvider, Provider, ReadUrlProvider,
    ResearchProvider, SessionMode, StooqAsset, StooqFrequency, StooqMarket,
};
use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand};

pub use crate::crypto_cli::*;
pub use crate::terminal_cli::*;

pub const HISTORY_INTERVAL_HELP: &str = "Bar interval. Provider-specific values: Yahoo 1m/2m/5m/15m/30m/60m/90m/1h/1d/5d/1wk/1mo/3mo; Robinhood 5m/10m/1h/1d/1w; Stooq live 1d/1w/1mo; Stooq bulk 5m/1h after sync; Binance 1m/3m/5m/15m/30m/1h/2h/4h/6h/8h/12h/1d/3d/1w/1M; Coinbase 1m/5m/15m/1h/6h/1d; OKX 1m/3m/5m/15m/30m/1h/2h/4h/6h/12h/1d/2d/3d; CoinGecko maps common intraday/daily requests to supported day windows.";

pub(crate) fn enum_value_parser<T>(
    labels: &'static [&'static str],
) -> impl TypedValueParser<Value = T>
where
    T: FromStr + Clone + Send + Sync + 'static,
    T::Err: std::fmt::Display + Send + Sync + 'static,
{
    PossibleValuesParser::new(labels).map(|value| {
        value
            .parse::<T>()
            .unwrap_or_else(|_| unreachable!("possible values must parse"))
    })
}

#[derive(Parser, Debug)]
#[command(name = "agent-finance", version)]
#[command(
    about = "Fetch financial market data and research context for humans and AI agents.",
    after_help = "AI agents: start with `agent-finance skills get core`; prefer capability-first commands, then force providers only for cross-checks."
)]
pub struct Cli {
    /// Explicit proxy URL, for example http://127.0.0.1:7890 or socks5h://127.0.0.1:7890.
    /// If omitted, AGENT_FINANCE_PROXY and standard proxy environment variables are checked.
    #[arg(long, global = true)]
    pub proxy: Option<String>,

    /// Disable proxy use for this invocation.
    #[arg(long, global = true)]
    pub no_proxy: bool,

    /// Human-output timezone. Defaults to the machine's local IANA timezone.
    /// UTC is still preserved in JSON fields.
    #[arg(long, global = true)]
    pub timezone: Option<String>,

    /// HTTP timeout in seconds.
    #[arg(long, global = true, default_value_t = 10)]
    pub timeout_seconds: u64,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Fetch read-only market data, research context, prediction signals, and streams.
    Market(MarketArgs),
    /// Print capability-first terminal surface for AI agents.
    Capabilities(CapabilitiesArgs),
    /// Inspect and explain trading profiles.
    Profile(ProfileArgs),
    /// Inspect signed account state.
    Account(AccountArgs),
    /// Create, submit, cancel, and query order intents.
    Order(OrderArgs),
    /// Create and submit internal transfer intents.
    Transfer(TransferArgs),
    /// Create and submit USD-M futures state-change intents.
    State(StateArgs),
    /// Check and explain profile risk policy.
    Risk(RiskArgs),
    /// Read local append-only trading audit events.
    Audit(AuditArgs),
    /// Print built-in AI-agent skill documents.
    Skills(SkillsArgs),
}

#[derive(Parser, Debug)]
pub struct MarketArgs {
    #[command(subcommand)]
    pub command: MarketCommand,
}

#[derive(Subcommand, Debug)]
pub enum MarketCommand {
    /// Print the current investable price summary for one or more symbols.
    Price(PriceArgs),
    /// Print regular/pre/post/overnight/provider split for one symbol.
    Sessions(SessionsArgs),
    /// Fetch OHLCV history.
    History(HistoryArgs),
    /// Compute local derived indicators from history.
    Indicators(IndicatorsArgs),
    /// Fetch fundamentals, valuation, statements, cash-flow, and SEC official facts.
    Fundamentals(ProviderResearchArgs),
    /// Fetch Yahoo analyst targets, recommendations, estimates, and revisions.
    Analysis(ResearchArgs),
    /// Fetch Yahoo option expiries and nearest/full option chains.
    Options(OptionsArgs),
    /// Fetch Yahoo holder and insider ownership modules.
    Ownership(ResearchArgs),
    /// Fetch earnings/dividend/split/calendar modules and SEC filing events.
    Events(ProviderResearchArgs),
    /// Fetch Yahoo news/search articles for a ticker.
    News(NewsArgs),
    /// Read a URL into AI-friendly text/Markdown with extraction fallbacks.
    ReadUrl(ReadUrlArgs),
    /// Search Yahoo Finance for tickers and news.
    Search(SearchArgs),
    /// Inspect crypto market data and cross-provider capability evidence.
    Crypto(CryptoArgs),
    /// Inspect Polymarket prediction-market sentiment and event-probability signals.
    Polymarket(PolymarketArgs),
    /// Run Yahoo predefined screeners.
    Screen(ScreenArgs),
    /// Inspect or import Stooq bulk historical data packages.
    Stooq(StooqArgs),
    /// Print provider capability matrix.
    Providers(ProvidersArgs),
    /// Poll live price summaries repeatedly.
    Watch(WatchArgs),
    /// Stream Yahoo real-time price updates over WebSocket.
    Stream(StreamArgs),
}

#[derive(Parser, Debug)]
pub struct PriceArgs {
    #[arg(required = true)]
    pub symbols: Vec<String>,

    #[arg(long, default_value_t = AssetClass::Auto, value_parser = enum_value_parser::<AssetClass>(AssetClass::labels()))]
    pub asset: AssetClass,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub crypto_provider: CryptoProvider,

    #[arg(long, default_value_t = SessionMode::Smart, value_parser = enum_value_parser::<SessionMode>(SessionMode::labels()))]
    pub session: SessionMode,

    /// Optional Binance USD-M futures or proxy symbol to display beside the quote.
    #[arg(long)]
    pub proxy_symbol: Option<String>,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct SessionsArgs {
    pub symbol: String,

    /// Optional Binance USD-M futures or proxy symbol to display beside the quote.
    #[arg(long)]
    pub proxy_symbol: Option<String>,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct HistoryArgs {
    pub symbol: String,

    #[arg(long, default_value_t = AssetClass::Auto, value_parser = enum_value_parser::<AssetClass>(AssetClass::labels()))]
    pub asset: AssetClass,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub crypto_provider: CryptoProvider,

    #[arg(long, default_value_t = Provider::Auto, value_parser = enum_value_parser::<Provider>(Provider::labels()))]
    pub provider: Provider,

    #[arg(long, default_value_t = HistorySession::Regular, value_parser = enum_value_parser::<HistorySession>(HistorySession::labels()))]
    pub session: HistorySession,

    #[arg(long, default_value_t = HistoryAdjustment::Auto, value_parser = enum_value_parser::<HistoryAdjustment>(HistoryAdjustment::labels()))]
    pub adjustment: HistoryAdjustment,

    #[arg(long)]
    pub no_actions: bool,

    #[arg(long)]
    pub repair: bool,

    #[arg(long, default_value = "1d", help = HISTORY_INTERVAL_HELP)]
    pub interval: String,

    #[arg(long, default_value = "6mo")]
    pub range: String,

    #[arg(long, default_value_t = 60)]
    pub limit: usize,

    /// Stooq bulk market scope for 5m/1h cache reads.
    #[arg(long, default_value_t = StooqMarket::Us, value_parser = enum_value_parser::<StooqMarket>(StooqMarket::labels()))]
    pub stooq_market: StooqMarket,

    /// Stooq bulk asset scope for 5m/1h cache reads.
    #[arg(long, default_value_t = StooqAsset::Stocks, value_parser = enum_value_parser::<StooqAsset>(StooqAsset::labels()))]
    pub stooq_asset: StooqAsset,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct IndicatorsArgs {
    #[arg(required = true)]
    pub symbols: Vec<String>,

    #[arg(long, default_value_t = AssetClass::Auto, value_parser = enum_value_parser::<AssetClass>(AssetClass::labels()))]
    pub asset: AssetClass,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub crypto_provider: CryptoProvider,

    #[arg(long, default_value_t = Provider::Auto, value_parser = enum_value_parser::<Provider>(Provider::labels()))]
    pub provider: Provider,

    #[arg(long, default_value_t = HistorySession::Regular, value_parser = enum_value_parser::<HistorySession>(HistorySession::labels()))]
    pub session: HistorySession,

    #[arg(long, default_value_t = HistoryAdjustment::Auto, value_parser = enum_value_parser::<HistoryAdjustment>(HistoryAdjustment::labels()))]
    pub adjustment: HistoryAdjustment,

    #[arg(long)]
    pub repair: bool,

    #[arg(long, default_value = "1d", help = HISTORY_INTERVAL_HELP)]
    pub interval: String,

    #[arg(long, default_value = "1y")]
    pub range: String,

    #[arg(long, default_value_t = 120)]
    pub limit: usize,

    /// Stooq bulk market scope for 5m/1h cache reads.
    #[arg(long, default_value_t = StooqMarket::Us, value_parser = enum_value_parser::<StooqMarket>(StooqMarket::labels()))]
    pub stooq_market: StooqMarket,

    /// Stooq bulk asset scope for 5m/1h cache reads.
    #[arg(long, default_value_t = StooqAsset::Stocks, value_parser = enum_value_parser::<StooqAsset>(StooqAsset::labels()))]
    pub stooq_asset: StooqAsset,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ResearchArgs {
    pub symbol: String,

    /// Print raw provider payload in human mode. JSON mode always includes payload.
    #[arg(long)]
    pub raw: bool,

    /// Ignore cache and fetch live.
    #[arg(long)]
    pub refresh: bool,

    /// Cache TTL for non-price data.
    #[arg(long, default_value_t = 3600)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ProviderResearchArgs {
    pub symbol: String,

    /// Research-data provider. auto merges no-key sources when they add useful coverage.
    #[arg(long, default_value_t = ResearchProvider::Auto, value_parser = enum_value_parser::<ResearchProvider>(ResearchProvider::labels()))]
    pub provider: ResearchProvider,

    /// Print raw provider payload in human mode. JSON mode always includes payload.
    #[arg(long)]
    pub raw: bool,

    /// Ignore cache and fetch live.
    #[arg(long)]
    pub refresh: bool,

    /// Cache TTL for non-price data.
    #[arg(long, default_value_t = 3600)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

impl ProviderResearchArgs {
    pub fn without_provider(self) -> ResearchArgs {
        ResearchArgs {
            symbol: self.symbol,
            raw: self.raw,
            refresh: self.refresh,
            cache_ttl_seconds: self.cache_ttl_seconds,
            json: self.json,
        }
    }
}

#[derive(Parser, Debug)]
pub struct OptionsArgs {
    pub symbol: String,

    /// Options-data provider.
    #[arg(long, default_value_t = OptionsProvider::Auto, value_parser = enum_value_parser::<OptionsProvider>(OptionsProvider::labels()))]
    pub provider: OptionsProvider,

    /// Expiration unix timestamp. If omitted, Yahoo returns the nearest chain and all expiries.
    #[arg(long)]
    pub expiry: Option<i64>,

    /// Robinhood expiration date in YYYY-MM-DD. Overrides --expiry for Robinhood.
    #[arg(long)]
    pub expiration_date: Option<String>,

    /// Maximum Robinhood option instruments to fetch for the selected expiration.
    #[arg(long, default_value_t = 80)]
    pub count: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 1800)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct NewsArgs {
    pub symbol: String,

    #[arg(long, default_value_t = 10)]
    pub count: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 900)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ReadUrlArgs {
    pub url: String,

    /// URL reader provider. auto tries direct/Jina/Defuddle; SEC Archives prefer reader fallbacks.
    #[arg(long, default_value_t = ReadUrlProvider::Auto, value_parser = enum_value_parser::<ReadUrlProvider>(ReadUrlProvider::labels()))]
    pub provider: ReadUrlProvider,

    /// Maximum content characters to print in human mode. 0 means no truncation.
    #[arg(long, default_value_t = 20000)]
    pub max_chars: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 8)]
    pub quotes_count: usize,

    #[arg(long, default_value_t = 5)]
    pub news_count: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 1800)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct PolymarketArgs {
    #[command(subcommand)]
    pub command: PolymarketCommand,
}

#[derive(Subcommand, Debug)]
pub enum PolymarketCommand {
    /// Search public Polymarket events and markets by relevance.
    Search(PolymarketSearchArgs),
    /// Inspect one Polymarket market by numeric id or slug.
    Market(PolymarketMarketArgs),
}

#[derive(Parser, Debug)]
pub struct PolymarketSearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 8)]
    pub limit: usize,

    #[arg(long)]
    pub include_closed: bool,

    #[arg(long)]
    pub min_volume: Option<f64>,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 900)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct PolymarketMarketArgs {
    pub identifier: String,

    /// Maximum rows for holder/trade/orderbook detail previews.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,

    #[arg(long)]
    pub include_closed: bool,

    #[arg(long)]
    pub min_volume: Option<f64>,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 900)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ScreenArgs {
    /// Yahoo predefined screener id, for example most_actives, day_gainers, day_losers.
    #[arg(default_value = "most_actives")]
    pub screener: String,

    #[arg(long, default_value_t = 25)]
    pub count: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub refresh: bool,

    #[arg(long, default_value_t = 1800)]
    pub cache_ttl_seconds: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct StooqArgs {
    #[command(subcommand)]
    pub command: StooqCommand,
}

#[derive(Subcommand, Debug)]
pub enum StooqCommand {
    /// Print known Stooq bulk packages and current local cache state.
    Catalog(StooqCatalogArgs),
    /// Import a Stooq bulk ZIP from a captcha-authorized URL or local file into cache.
    Sync(StooqSyncArgs),
}

#[derive(Parser, Debug)]
pub struct StooqCatalogArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct StooqSyncArgs {
    #[arg(long, value_parser = enum_value_parser::<StooqFrequency>(StooqFrequency::labels()))]
    pub frequency: StooqFrequency,

    #[arg(long, value_parser = enum_value_parser::<StooqMarket>(StooqMarket::labels()))]
    pub market: StooqMarket,

    #[arg(long, value_parser = enum_value_parser::<StooqAsset>(StooqAsset::labels()))]
    pub asset: StooqAsset,

    /// Captcha-authorized Stooq ZIP download URL.
    #[arg(long)]
    pub url: Option<String>,

    /// Local Stooq ZIP file to import.
    #[arg(long)]
    pub zip_path: Option<PathBuf>,

    /// Overwrite an existing cached ZIP.
    #[arg(long)]
    pub force: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ProvidersArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct WatchArgs {
    #[arg(required = true)]
    pub symbols: Vec<String>,

    #[arg(long, default_value_t = AssetClass::Auto, value_parser = enum_value_parser::<AssetClass>(AssetClass::labels()))]
    pub asset: AssetClass,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub crypto_provider: CryptoProvider,

    #[arg(long, default_value_t = 15)]
    pub interval_seconds: u64,

    /// Number of polling iterations. 0 means run until interrupted.
    #[arg(long, default_value_t = 1)]
    pub iterations: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct StreamArgs {
    #[arg(required = true)]
    pub symbols: Vec<String>,

    /// Number of price updates to print before exiting. 0 streams until interrupted; JSON mode then prints JSON Lines.
    #[arg(long, default_value_t = 5)]
    pub messages: usize,

    /// Yahoo streamer URL.
    #[arg(long, default_value = "wss://streamer.finance.yahoo.com/?version=2")]
    pub url: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Subcommand, Debug)]
pub enum SkillsCommand {
    /// List built-in AI-agent skill documents served by this CLI.
    List,
    /// Print a built-in skill document.
    Get(SkillGetArgs),
}

#[derive(Parser, Debug)]
pub struct SkillGetArgs {
    /// Skill name. Start with "core".
    pub name: String,

    /// Include fuller command reference and templates when available.
    #[arg(long)]
    pub full: bool,
}
