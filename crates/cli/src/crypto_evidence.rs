use anyhow::{Result, anyhow};

use agent_finance_market::service::{self, CryptoEvidenceReport, MarketRuntime};

pub async fn run_quote(
    args: crate::cli::CryptoEvidenceSymbolArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_quote(
        &runtime,
        service::CryptoEvidenceSymbolRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_book(
    args: crate::cli::CryptoEvidenceBookArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_book(
        &runtime,
        service::CryptoEvidenceLimitRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
            limit: args.limit,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_trades(
    args: crate::cli::CryptoEvidenceTradesArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_trades(
        &runtime,
        service::CryptoEvidenceTradesRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
            limit: args.limit,
            aggregate: args.aggregate,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_candles(
    args: crate::cli::CryptoEvidenceKlinesArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_candles(
        &runtime,
        service::CryptoEvidenceCandlesRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
            interval: args.interval,
            limit: args.limit,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_funding(
    args: crate::cli::CryptoEvidenceFundingArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_funding(
        &runtime,
        service::CryptoEvidenceLimitRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
            limit: args.limit,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_open_interest(
    args: crate::cli::CryptoEvidenceOpenInterestArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_open_interest(
        &runtime,
        service::CryptoEvidenceSymbolRequest {
            symbol: args.symbol,
            provider: args.provider,
            instrument: args.instrument,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

pub async fn run_discover(
    args: crate::cli::CryptoDiscoverArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, "UTC");
    let report = service::crypto_evidence_discover(
        &runtime,
        service::CryptoEvidenceDiscoverRequest {
            provider: args.provider,
            kind: args.kind,
            instrument: args.instrument,
            limit: args.limit,
            vs_currency: args.vs_currency,
        },
    )
    .await?;
    print_evidence_report(report, args.json, args.raw)
}

fn print_evidence_report(report: CryptoEvidenceReport, json: bool, raw: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "crypto {} instrument={} symbol={} fetched={}",
            report.capability,
            report.instrument,
            report.symbol.as_deref().unwrap_or("-"),
            report.fetched_at_utc
        );
        for result in &report.results {
            println!(
                "{} {}",
                result.provider,
                if result.ok { "ok" } else { "error" }
            );
            for endpoint in &result.endpoints {
                println!(
                    "  {} {}",
                    endpoint.endpoint,
                    if endpoint.ok { "ok" } else { "error" }
                );
                if let Some(error) = endpoint.error.as_deref() {
                    println!("    {error}");
                } else if let Some(payload) = endpoint.payload.as_ref() {
                    if raw {
                        println!("{}", serde_json::to_string_pretty(payload)?);
                    } else {
                        println!("    payload: {}", payload_summary(payload));
                    }
                }
            }
        }
    }
    if report.results.iter().any(|result| result.ok) {
        Ok(())
    } else {
        Err(anyhow!(
            "no provider returned crypto {} evidence for instrument={}",
            report.capability,
            report.instrument
        ))
    }
}

fn payload_summary(payload: &serde_json::Value) -> String {
    match payload {
        serde_json::Value::Array(rows) => format!("array rows={}", rows.len()),
        serde_json::Value::Object(fields) => format!("object fields={}", fields.len()),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "bool".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(value) => format!("string chars={}", value.chars().count()),
    }
}
