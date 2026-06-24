use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::Parser;
use futures_util::StreamExt;
use serde::Serialize;

use crate::cli::{
    AssetClass, Cli, Command, HistoryArgs, HistorySession, IndicatorsArgs, MarketArgs,
    MarketCommand, NewsArgs, OptionsArgs, OptionsProvider, PolymarketArgs, PolymarketCommand,
    Provider, ProviderResearchArgs, ProvidersArgs, ReadUrlArgs, ResearchArgs, ScreenArgs,
    SearchArgs, SessionsArgs, StooqArgs, StooqCommand, WatchArgs,
};
use crate::crypto_app;
use crate::crypto_market_data;
use crate::http::http_client;
use crate::indicators::compute_indicator;
use crate::model::DerivedIndicator;
use crate::output;
use crate::page_read;
use crate::price;
use crate::providers::{self, binance, stooq};
use crate::research;
use crate::skills;
use crate::stream;
use crate::time::resolve_timezone;

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let proxy = cli.proxy.as_deref();
    let no_proxy = cli.no_proxy;
    let timezone = resolve_timezone(cli.timezone.as_deref())?;
    let timezone = timezone.as_str();
    let timeout_seconds = cli.timeout_seconds;
    match cli.command {
        Command::Market(args) => run_market(args, proxy, no_proxy, timeout_seconds, timezone).await,
        Command::Capabilities(args) => crate::terminal_app::run_capabilities(args),
        Command::Profile(args) => crate::terminal_app::run_profile(args, timeout_seconds).await,
        Command::Account(args) => crate::terminal_app::run_account(args, timeout_seconds).await,
        Command::Order(args) => crate::terminal_app::run_order(args, timeout_seconds).await,
        Command::Transfer(args) => crate::terminal_app::run_transfer(args, timeout_seconds).await,
        Command::State(args) => crate::terminal_state::run(args, timeout_seconds).await,
        Command::Risk(args) => crate::terminal_app::run_risk(args),
        Command::Audit(args) => crate::terminal_app::run_audit(args),
        Command::Skills(args) => run_skills(args),
    }
}

async fn run_market(
    args: MarketArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    match args.command {
        MarketCommand::Price(args) => {
            run_price(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Sessions(args) => {
            run_sessions(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::History(args) => {
            run_history(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Indicators(args) => {
            run_indicators(args, proxy, no_proxy, timeout_seconds).await
        }
        MarketCommand::Fundamentals(args) => {
            run_provider_quote_summary(
                args,
                research::QuoteSummaryKind::Fundamentals,
                proxy,
                no_proxy,
                timeout_seconds,
                timezone,
            )
            .await
        }
        MarketCommand::Analysis(args) => {
            run_quote_summary(
                args,
                research::QuoteSummaryKind::Analysis,
                crate::cli::ResearchProvider::Yahoo,
                proxy,
                no_proxy,
                timeout_seconds,
                timezone,
            )
            .await
        }
        MarketCommand::Options(args) => {
            run_options(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Ownership(args) => {
            run_quote_summary(
                args,
                research::QuoteSummaryKind::Ownership,
                crate::cli::ResearchProvider::Yahoo,
                proxy,
                no_proxy,
                timeout_seconds,
                timezone,
            )
            .await
        }
        MarketCommand::Events(args) => {
            run_provider_quote_summary(
                args,
                research::QuoteSummaryKind::Events,
                proxy,
                no_proxy,
                timeout_seconds,
                timezone,
            )
            .await
        }
        MarketCommand::News(args) => {
            run_news(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::ReadUrl(args) => run_read_url(args, proxy, no_proxy, timeout_seconds).await,
        MarketCommand::Search(args) => {
            run_search(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Crypto(args) => {
            crypto_app::run(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Polymarket(args) => {
            run_polymarket(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Screen(args) => {
            run_screen(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Stooq(args) => run_stooq(args, proxy, no_proxy, timeout_seconds).await,
        MarketCommand::Providers(args) => run_providers(args),
        MarketCommand::Watch(args) => {
            run_watch(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Stream(args) => {
            run_stream(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
    }
}

async fn run_polymarket(
    args: PolymarketArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let use_http_transport = proxy.is_some() || no_proxy;
    match args.command {
        PolymarketCommand::Search(args) => {
            let options = providers::polymarket::SearchRequestOptions {
                query: args.query,
                limit: args.limit,
                include_closed: args.include_closed,
                min_volume: args.min_volume,
                refresh: args.refresh,
                cache_ttl_seconds: args.cache_ttl_seconds,
                timeout_seconds,
                use_http_transport,
            };
            let report = providers::polymarket::search_report(&client, &options, timezone).await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_prediction_search_report(&report);
            }
            Ok(())
        }
        PolymarketCommand::Market(args) => {
            let options = providers::polymarket::MarketRequestOptions {
                identifier: args.identifier,
                limit: args.limit,
                include_closed: args.include_closed,
                min_volume: args.min_volume,
                refresh: args.refresh,
                cache_ttl_seconds: args.cache_ttl_seconds,
                timeout_seconds,
                use_http_transport,
            };
            let report = providers::polymarket::market_report(&client, &options, timezone).await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_prediction_market_report(&report);
            }
            Ok(())
        }
    }
}

fn run_skills(args: crate::cli::SkillsArgs) -> Result<()> {
    match args.command {
        crate::cli::SkillsCommand::List => {
            skills::print_list()?;
            Ok(())
        }
        crate::cli::SkillsCommand::Get(args) => {
            let Some(content) = skills::get(&args.name, args.full)? else {
                return Err(anyhow!(
                    "unknown skill '{}'; run `agent-finance skills list`",
                    args.name
                ));
            };
            println!("{content}");
            Ok(())
        }
    }
}

async fn run_price(
    args: crate::cli::PriceArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    if args.asset == AssetClass::Crypto {
        return crypto_market_data::run_price(args, proxy, no_proxy, timeout_seconds, timezone)
            .await;
    }
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let binance_config = binance::BinanceConfig::from_env(timeout_seconds, proxy, no_proxy);
    let summaries = futures_util::stream::iter(args.symbols)
        .map(|symbol| {
            let client = &client;
            let binance_config = &binance_config;
            let proxy_symbol = args.proxy_symbol.as_deref();
            async move {
                price::fetch_price_summary(
                    client,
                    &symbol,
                    timezone,
                    args.session,
                    Some(binance_config),
                    proxy_symbol,
                )
                .await
            }
        })
        .buffered(4)
        .collect::<Vec<_>>()
        .await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summaries)?);
    } else {
        for (index, summary) in summaries.iter().enumerate() {
            if index > 0 {
                println!();
            }
            output::print_price_summary(
                summary,
                matches!(args.session, crate::cli::SessionMode::All),
            );
        }
    }

    if summaries.iter().all(|summary| summary.current.is_some()) {
        Ok(())
    } else {
        Err(anyhow!("one or more price summaries had no current quote"))
    }
}

async fn run_sessions(
    args: SessionsArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let binance_config = binance::BinanceConfig::from_env(timeout_seconds, proxy, no_proxy);
    let summary = price::fetch_price_summary(
        &client,
        &args.symbol,
        timezone,
        crate::cli::SessionMode::All,
        Some(&binance_config),
        args.proxy_symbol.as_deref(),
    )
    .await;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        output::print_price_summary(&summary, true);
    }

    if summary.current.is_some() {
        Ok(())
    } else {
        Err(anyhow!("no current quote found for {}", summary.symbol))
    }
}

async fn run_history(
    args: HistoryArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    if args.asset == AssetClass::Crypto
        || matches!(
            args.provider,
            Provider::BinanceSpot | Provider::BinanceUsdsFutures
        )
    {
        return crypto_market_data::run_history(args, proxy, no_proxy, timeout_seconds, timezone)
            .await;
    }
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let provider = effective_history_provider(args.provider, args.session);
    let request = providers::HistoryRequest {
        symbol: args.symbol,
        interval: args.interval,
        range: args.range,
        limit: args.limit,
        extended_session: matches!(args.session, HistorySession::Extended),
        adjustment: args.adjustment,
        actions: !args.no_actions,
        repair: args.repair,
        stooq_market: args.stooq_market,
        stooq_asset: args.stooq_asset,
    };
    let history = providers::fetch_history(&client, provider, &request).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&history)?);
    } else {
        output::print_history_table(&history, timezone);
    }
    Ok(())
}

async fn run_indicators(
    args: IndicatorsArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    if args.asset == AssetClass::Crypto
        || matches!(
            args.provider,
            Provider::BinanceSpot | Provider::BinanceUsdsFutures
        )
    {
        return crypto_market_data::run_indicators(args, proxy, no_proxy, timeout_seconds).await;
    }
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let mut indicators = Vec::new();
    let mut errors = BTreeMap::new();
    let provider = effective_history_provider(args.provider, args.session);

    let symbols = args
        .symbols
        .into_iter()
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect::<Vec<_>>();
    let results = futures_util::stream::iter(symbols)
        .map(|normalized| {
            let client = &client;
            let interval = args.interval.clone();
            let range = args.range.clone();
            let stooq_market = args.stooq_market;
            let stooq_asset = args.stooq_asset;
            async move {
                let request = providers::HistoryRequest {
                    symbol: normalized.clone(),
                    interval,
                    range,
                    limit: args.limit,
                    extended_session: matches!(args.session, HistorySession::Extended),
                    adjustment: args.adjustment,
                    actions: false,
                    repair: args.repair,
                    stooq_market,
                    stooq_asset,
                };
                let result = providers::fetch_history(client, provider, &request).await;
                (normalized, result)
            }
        })
        .buffered(4)
        .collect::<Vec<_>>()
        .await;

    for (normalized, result) in results {
        match result {
            Ok(history) => indicators.push(compute_indicator(&history)),
            Err(error) => {
                errors.insert(normalized, format!("{error:#}"));
            }
        }
    }

    let batch = IndicatorBatch { indicators, errors };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&batch)?);
    } else {
        output::print_indicator_table(&batch.indicators, &batch.errors);
    }

    if batch.errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("one or more indicators failed"))
    }
}

async fn run_quote_summary(
    args: ResearchArgs,
    kind: research::QuoteSummaryKind,
    provider: crate::cli::ResearchProvider,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let report = research::quote_summary_report(
        &client,
        &args.symbol,
        kind,
        provider,
        timezone,
        args.refresh,
        args.cache_ttl_seconds,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_research_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_provider_quote_summary(
    args: ProviderResearchArgs,
    kind: research::QuoteSummaryKind,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let provider = args.provider;
    run_quote_summary(
        args.without_provider(),
        kind,
        provider,
        proxy,
        no_proxy,
        timeout_seconds,
        timezone,
    )
    .await
}

fn run_providers(args: ProvidersArgs) -> Result<()> {
    let profiles = providers::capabilities::profiles();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&profiles)?);
    } else {
        output::print_provider_profiles(&profiles);
    }
    Ok(())
}

async fn run_options(
    args: OptionsArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    if args.expiration_date.is_some() && !matches!(args.provider, OptionsProvider::Robinhood) {
        return Err(anyhow!(
            "--expiration-date is Robinhood-only; use --expiry for yahoo/auto or pass --provider robinhood"
        ));
    }
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let report = research::options_report(research::OptionsReportRequest {
        client: &client,
        symbol: &args.symbol,
        provider: args.provider,
        expiry: args.expiry,
        expiration_date: args.expiration_date.as_deref(),
        count: args.count,
        timezone,
        refresh: args.refresh,
        ttl_seconds: args.cache_ttl_seconds,
    })
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_research_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_stooq(
    args: StooqArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    match args.command {
        StooqCommand::Catalog(args) => {
            let catalog = stooq::catalog();
            if args.json {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                output::print_stooq_catalog(&catalog);
            }
            Ok(())
        }
        StooqCommand::Sync(args) => {
            let client = http_client(timeout_seconds, proxy, no_proxy)?;
            let report = stooq::sync_bulk(
                &client,
                stooq::StooqSyncRequest {
                    frequency: args.frequency,
                    market: args.market,
                    asset: args.asset,
                    url: args.url,
                    zip_path: args.zip_path,
                    force: args.force,
                },
            )
            .await?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_stooq_sync_report(&report);
            }
            Ok(())
        }
    }
}

async fn run_news(
    args: NewsArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let report = research::news_report(
        &client,
        &args.symbol,
        args.count,
        timezone,
        args.refresh,
        args.cache_ttl_seconds,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_read_url(
    args: ReadUrlArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let max_chars = if args.json { 0 } else { args.max_chars };
    let report = page_read::read_url(&client, &args.url, args.provider, max_chars).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_page_read_report(&report);
    }
    Ok(())
}

async fn run_search(
    args: SearchArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let report = research::search_report(
        &client,
        &args.query,
        args.quotes_count,
        args.news_count,
        timezone,
        args.refresh,
        args.cache_ttl_seconds,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_screen(
    args: ScreenArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let report = research::screen_report(
        &client,
        &args.screener,
        args.count,
        timezone,
        args.refresh,
        args.cache_ttl_seconds,
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_watch(
    args: WatchArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    if args.asset == AssetClass::Crypto {
        return crypto_market_data::run_watch(args, proxy, no_proxy, timeout_seconds, timezone)
            .await;
    }
    let client = http_client(timeout_seconds, proxy, no_proxy)?;
    let mut iteration = 0usize;
    loop {
        iteration += 1;
        let last_summaries = futures_util::stream::iter(args.symbols.iter())
            .map(|symbol| {
                let client = &client;
                async move {
                    price::fetch_price_summary(
                        client,
                        symbol,
                        timezone,
                        crate::cli::SessionMode::Smart,
                        None,
                        None,
                    )
                    .await
                }
            })
            .buffered(4)
            .collect::<Vec<_>>()
            .await;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&last_summaries)?);
        } else {
            for summary in &last_summaries {
                output::print_price_summary(summary, false);
                println!();
            }
        }
        if args.iterations != 0 && iteration >= args.iterations {
            break;
        }
        tokio::time::sleep(Duration::from_secs(args.interval_seconds.max(1))).await;
    }
    Ok(())
}

async fn run_stream(
    args: crate::cli::StreamArgs,
    proxy: Option<&str>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<()> {
    if args.messages == 0 {
        stream::stream_quotes_each(
            stream::StreamOptions {
                url: &args.url,
                symbols: args.symbols,
                message_limit: args.messages,
                read_timeout: Duration::from_secs(timeout_seconds.max(1)),
                timezone,
                proxy,
                no_proxy,
            },
            |quote| {
                if args.json {
                    println!("{}", serde_json::to_string(&quote)?);
                } else {
                    output::print_stream_quotes(std::slice::from_ref(&quote));
                }
                Ok(())
            },
        )
        .await?;
        return Ok(());
    }

    let updates = stream::stream_quotes(stream::StreamOptions {
        url: &args.url,
        symbols: args.symbols,
        message_limit: args.messages,
        read_timeout: Duration::from_secs(timeout_seconds.max(1)),
        timezone,
        proxy,
        no_proxy,
    })
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&updates)?);
    } else {
        output::print_stream_quotes(&updates);
    }
    Ok(())
}

fn effective_history_provider(provider: Provider, session: HistorySession) -> Provider {
    match (provider, session) {
        (Provider::Auto, HistorySession::Extended) => Provider::YahooExtended,
        (Provider::Yahoo, HistorySession::Extended) => Provider::YahooExtended,
        (provider, _) => provider,
    }
}

#[derive(Debug, Serialize)]
struct IndicatorBatch {
    indicators: Vec<DerivedIndicator>,
    errors: BTreeMap<String, String>,
}
