pub use agent_finance_market::{
    CryptoDiscoverKind, CryptoInstrument, CryptoMarket, CryptoProvider, CryptoStreamKind,
};
use clap::{Parser, Subcommand};

use crate::cli::enum_value_parser;

const CRYPTO_INTERVAL_HELP: &str = "Crypto candle interval. Binance: 1m/3m/5m/15m/30m/1h/2h/4h/6h/8h/12h/1d/3d/1w/1M; Coinbase: 1m/5m/15m/1h/6h/1d; OKX: 1m/3m/5m/15m/30m/1h/2h/4h/6h/12h/1d/2d/3d.";

#[derive(Parser, Debug)]
pub struct CryptoArgs {
    #[command(subcommand)]
    pub command: CryptoCommand,
}

#[derive(Subcommand, Debug)]
pub enum CryptoCommand {
    /// Aggregate Binance spot and USD-M futures state for one symbol.
    Snapshot(CryptoSymbolArgs),
    /// Aggregate Binance USD-M funding, open interest, long/short, taker flow, and basis signals.
    Sentiment(CryptoSymbolArgs),
    /// Stream selected Binance WebSocket market events.
    Stream(CryptoStreamArgs),
    /// Fetch quote evidence across Binance, Coinbase, OKX, and CoinGecko.
    Quote(CryptoEvidenceSymbolArgs),
    /// Fetch order-book depth across providers where available.
    Book(CryptoEvidenceBookArgs),
    /// Fetch recent trade evidence across providers where available.
    Trades(CryptoEvidenceTradesArgs),
    /// Fetch OHLCV or OHLC candle evidence across providers where available.
    Candles(CryptoEvidenceKlinesArgs),
    /// Fetch derivatives funding-rate evidence across providers where available.
    Funding(CryptoEvidenceFundingArgs),
    /// Fetch derivatives open-interest evidence across providers where available.
    OpenInterest(CryptoEvidenceOpenInterestArgs),
    /// Discover provider markets, metadata, trending, global, or exchange data.
    Discover(CryptoDiscoverArgs),
}

#[derive(Parser, Debug)]
pub struct CryptoSymbolArgs {
    pub symbol: String,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoStreamArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = CryptoStreamKind::Trade, value_parser = enum_value_parser::<CryptoStreamKind>(CryptoStreamKind::labels()))]
    pub kind: CryptoStreamKind,

    #[arg(long, default_value = "1m", help = CRYPTO_INTERVAL_HELP)]
    pub interval: String,

    #[arg(long, default_value_t = 5)]
    pub messages: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceSymbolArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceBookArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceTradesArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub aggregate: bool,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceKlinesArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value = "1m", help = CRYPTO_INTERVAL_HELP)]
    pub interval: String,

    #[arg(long, default_value_t = 60)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceFundingArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value_t = 8)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoEvidenceOpenInterestArgs {
    pub symbol: String,

    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CryptoDiscoverArgs {
    #[arg(long, default_value_t = CryptoProvider::Auto, value_parser = enum_value_parser::<CryptoProvider>(CryptoProvider::labels()))]
    pub provider: CryptoProvider,

    #[arg(long, default_value_t = CryptoDiscoverKind::Markets, value_parser = enum_value_parser::<CryptoDiscoverKind>(CryptoDiscoverKind::labels()))]
    pub kind: CryptoDiscoverKind,

    #[arg(long, default_value_t = CryptoInstrument::Auto, value_parser = enum_value_parser::<CryptoInstrument>(CryptoInstrument::labels()))]
    pub instrument: CryptoInstrument,

    #[arg(long, default_value = "usd")]
    pub vs_currency: String,

    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub json: bool,
}
