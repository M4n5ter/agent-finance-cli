use agent_finance_market::service;
use anyhow::{Result, anyhow};

use crate::cli::{CryptoArgs, CryptoCommand, CryptoInstrument, CryptoMarket};
use crate::output;

pub async fn run(
    args: CryptoArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let runtime = service::MarketRuntime::new(proxy, no_proxy, timeout_seconds, timezone);
    match args.command {
        CryptoCommand::Snapshot(args) => {
            let report = service::crypto_snapshot(
                &runtime,
                service::CryptoSymbolRequest {
                    symbol: args.symbol,
                },
            )
            .await;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_crypto_snapshot(&report, timezone, args.raw)?;
            }
            report.ensure_complete()
        }
        CryptoCommand::Sentiment(args) => {
            let report = service::crypto_sentiment(
                &runtime,
                service::CryptoSymbolRequest {
                    symbol: args.symbol,
                },
            )
            .await;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_crypto_sentiment(&report, timezone, args.raw)?;
            }
            report.ensure_complete()
        }
        CryptoCommand::Stream(args) => {
            let report = service::crypto_stream(
                &runtime,
                service::CryptoStreamRequest {
                    symbol: args.symbol,
                    market: stream_market(args.instrument, args.kind)?,
                    kind: args.kind,
                    interval: args.interval,
                    messages: args.messages,
                },
            )
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_crypto_stream(&report)?;
            }
            Ok(())
        }
        CryptoCommand::Quote(args) => {
            crate::crypto_evidence::run_quote(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::Book(args) => {
            crate::crypto_evidence::run_book(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::Trades(args) => {
            crate::crypto_evidence::run_trades(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::Candles(args) => {
            crate::crypto_evidence::run_candles(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::Funding(args) => {
            crate::crypto_evidence::run_funding(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::OpenInterest(args) => {
            crate::crypto_evidence::run_open_interest(args, proxy, no_proxy, timeout_seconds).await
        }
        CryptoCommand::Discover(args) => {
            crate::crypto_evidence::run_discover(args, proxy, no_proxy, timeout_seconds).await
        }
    }
}

fn stream_market(
    instrument: CryptoInstrument,
    kind: crate::cli::CryptoStreamKind,
) -> Result<CryptoMarket> {
    match (instrument, kind) {
        (CryptoInstrument::Auto, crate::cli::CryptoStreamKind::MarkPrice) => {
            Ok(CryptoMarket::UsdsFutures)
        }
        (CryptoInstrument::Auto | CryptoInstrument::Spot, _) => Ok(CryptoMarket::Spot),
        (CryptoInstrument::Swap | CryptoInstrument::Futures, _) => Ok(CryptoMarket::UsdsFutures),
        (CryptoInstrument::Option, _) => Err(anyhow!(
            "crypto stream does not support instrument=option; use spot or swap"
        )),
    }
}
