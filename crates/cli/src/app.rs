use anyhow::{Result, anyhow};
use clap::Parser;

use agent_finance_market::service::{self, MarketRuntime, PriceResponse, WatchResponse};
use agent_finance_market::time::resolve_timezone;

use crate::cli::{
    Cli, Command, HistoryArgs, IndicatorsArgs, MarketArgs, MarketCommand, NewsArgs, OptionsArgs,
    OptionsProvider, PolymarketArgs, PolymarketCommand, ProviderResearchArgs, ProvidersArgs,
    ReadUrlArgs, ResearchArgs, ScreenArgs, SearchArgs, SessionsArgs, StooqArgs, StooqCommand,
    WatchArgs,
};
use crate::crypto_app;
use crate::output;
use crate::skills;

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let proxy = cli.proxy.as_deref();
    let no_proxy = cli.no_proxy;
    let timezone = resolve_timezone(cli.timezone.as_deref())?;
    let timezone = timezone.as_str();
    let timeout_seconds = cli.timeout_seconds;
    match cli.command {
        Command::Market(args) => run_market(args, proxy, no_proxy, timeout_seconds, timezone).await,
        Command::Tui(args) => {
            let dump_state = args
                .dump_state
                .then_some(agent_finance_tui::TuiDumpOptions {
                    wait_seconds: args.wait_seconds,
                    json: args.json,
                });
            agent_finance_tui::run(
                agent_finance_tui::TuiLaunch::with_market_runtime(
                    args.symbols,
                    args.config,
                    args.no_persist,
                    proxy,
                    no_proxy,
                    timeout_seconds,
                    timezone,
                )
                .with_profile(args.profile)
                .with_workspace(args.workspace)
                .with_dump_state(dump_state),
            )
        }
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
    let runtime = MarketRuntime::new(proxy, no_proxy, timeout_seconds, timezone);
    match args.command {
        MarketCommand::Price(args) => run_price(&runtime, args).await,
        MarketCommand::Sessions(args) => run_sessions(&runtime, args).await,
        MarketCommand::History(args) => run_history(&runtime, args).await,
        MarketCommand::Indicators(args) => run_indicators(&runtime, args).await,
        MarketCommand::Fundamentals(args) => {
            run_provider_quote_summary(
                &runtime,
                args,
                service::MarketQuoteSummaryKind::Fundamentals,
            )
            .await
        }
        MarketCommand::Analysis(args) => {
            run_quote_summary(
                &runtime,
                args,
                service::MarketQuoteSummaryKind::Analysis,
                crate::cli::ResearchProvider::Yahoo,
            )
            .await
        }
        MarketCommand::Options(args) => run_options(&runtime, args).await,
        MarketCommand::Ownership(args) => {
            run_quote_summary(
                &runtime,
                args,
                service::MarketQuoteSummaryKind::Ownership,
                crate::cli::ResearchProvider::Yahoo,
            )
            .await
        }
        MarketCommand::Events(args) => {
            run_provider_quote_summary(&runtime, args, service::MarketQuoteSummaryKind::Events)
                .await
        }
        MarketCommand::News(args) => run_news(&runtime, args).await,
        MarketCommand::ReadUrl(args) => run_read_url(&runtime, args).await,
        MarketCommand::Search(args) => run_search(&runtime, args).await,
        MarketCommand::Crypto(args) => {
            crypto_app::run(args, proxy, no_proxy, timeout_seconds, timezone).await
        }
        MarketCommand::Polymarket(args) => run_polymarket(&runtime, args).await,
        MarketCommand::Screen(args) => run_screen(&runtime, args).await,
        MarketCommand::Stooq(args) => run_stooq(&runtime, args).await,
        MarketCommand::Providers(args) => run_providers(args),
        MarketCommand::Watch(args) => run_watch(&runtime, args).await,
        MarketCommand::Stream(args) => run_stream(&runtime, args).await,
    }
}

async fn run_polymarket(runtime: &MarketRuntime, args: PolymarketArgs) -> Result<()> {
    match args.command {
        PolymarketCommand::Search(args) => {
            let json = args.json;
            let report = service::polymarket_search(
                runtime,
                service::PolymarketSearchRequest {
                    query: args.query,
                    limit: args.limit,
                    include_closed: args.include_closed,
                    min_volume: args.min_volume,
                    refresh: args.refresh,
                    cache_ttl_seconds: args.cache_ttl_seconds,
                },
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                output::print_prediction_search_report(&report);
            }
            Ok(())
        }
        PolymarketCommand::Market(args) => {
            let json = args.json;
            let report = service::polymarket_market(
                runtime,
                service::PolymarketMarketRequest {
                    identifier: args.identifier,
                    limit: args.limit,
                    include_closed: args.include_closed,
                    min_volume: args.min_volume,
                    refresh: args.refresh,
                    cache_ttl_seconds: args.cache_ttl_seconds,
                },
            )
            .await?;
            if json {
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

async fn run_price(runtime: &MarketRuntime, args: crate::cli::PriceArgs) -> Result<()> {
    let json = args.json;
    let show_all = matches!(args.session, crate::cli::SessionMode::All);
    let response = service::price(
        runtime,
        service::PriceRequest {
            symbols: args.symbols,
            asset: args.asset,
            instrument: args.instrument,
            crypto_provider: args.crypto_provider,
            provider: crate::cli::Provider::Auto,
            session: args.session,
            proxy_symbol: args.proxy_symbol,
        },
    )
    .await?;
    match &response {
        PriceResponse::Crypto(batch) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&batch)?);
            } else {
                output::print_crypto_price_points(&batch.points, &batch.errors);
            }
        }
        PriceResponse::Equity(summaries) if json => {
            println!("{}", serde_json::to_string_pretty(&summaries)?);
        }
        PriceResponse::Equity(summaries) => {
            for (index, summary) in summaries.iter().enumerate() {
                if index > 0 {
                    println!();
                }
                output::print_price_summary(summary, show_all);
            }
        }
    }

    if response.is_complete() {
        Ok(())
    } else {
        Err(response.completion_error())
    }
}

async fn run_sessions(runtime: &MarketRuntime, args: SessionsArgs) -> Result<()> {
    let summary = service::sessions(
        runtime,
        service::SessionsRequest {
            symbol: args.symbol,
            proxy_symbol: args.proxy_symbol,
        },
    )
    .await?;

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

async fn run_history(runtime: &MarketRuntime, args: HistoryArgs) -> Result<()> {
    let json = args.json;
    let history = service::history(
        runtime,
        service::HistoryRequest {
            symbol: args.symbol,
            asset: args.asset,
            instrument: args.instrument,
            crypto_provider: args.crypto_provider,
            provider: args.provider,
            session: args.session,
            adjustment: args.adjustment,
            no_actions: args.no_actions,
            repair: args.repair,
            interval: args.interval,
            range: args.range,
            limit: args.limit,
            stooq_market: args.stooq_market,
            stooq_asset: args.stooq_asset,
        },
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&history)?);
    } else {
        output::print_history_table(&history, runtime.timezone());
    }
    Ok(())
}

async fn run_indicators(runtime: &MarketRuntime, args: IndicatorsArgs) -> Result<()> {
    let json = args.json;
    let batch = service::indicators(
        runtime,
        service::IndicatorsRequest {
            symbols: args.symbols,
            asset: args.asset,
            instrument: args.instrument,
            crypto_provider: args.crypto_provider,
            provider: args.provider,
            session: args.session,
            adjustment: args.adjustment,
            repair: args.repair,
            interval: args.interval,
            range: args.range,
            limit: args.limit,
            stooq_market: args.stooq_market,
            stooq_asset: args.stooq_asset,
        },
    )
    .await?;
    if json {
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
    runtime: &MarketRuntime,
    args: ResearchArgs,
    kind: service::MarketQuoteSummaryKind,
    provider: crate::cli::ResearchProvider,
) -> Result<()> {
    let report = service::quote_summary(
        runtime,
        service::QuoteSummaryRequest {
            symbol: args.symbol,
            kind,
            provider,
            refresh: args.refresh,
            cache_ttl_seconds: args.cache_ttl_seconds,
        },
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
    runtime: &MarketRuntime,
    args: ProviderResearchArgs,
    kind: service::MarketQuoteSummaryKind,
) -> Result<()> {
    let provider = args.provider;
    run_quote_summary(runtime, args.without_provider(), kind, provider).await
}

fn run_providers(args: ProvidersArgs) -> Result<()> {
    let profiles = service::provider_profiles();
    if args.json {
        println!("{}", serde_json::to_string_pretty(&profiles)?);
    } else {
        output::print_provider_profiles(&profiles);
    }
    Ok(())
}

async fn run_options(runtime: &MarketRuntime, args: OptionsArgs) -> Result<()> {
    if args.expiration_date.is_some() && !matches!(args.provider, OptionsProvider::Robinhood) {
        return Err(anyhow!(
            "--expiration-date is Robinhood-only; use --expiry for yahoo/auto or pass --provider robinhood"
        ));
    }
    let report = service::options(
        runtime,
        service::OptionsRequest {
            symbol: args.symbol,
            provider: args.provider,
            expiry: args.expiry,
            expiration_date: args.expiration_date,
            count: args.count,
            refresh: args.refresh,
            cache_ttl_seconds: args.cache_ttl_seconds,
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_research_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_stooq(runtime: &MarketRuntime, args: StooqArgs) -> Result<()> {
    match args.command {
        StooqCommand::Catalog(args) => {
            let catalog = service::stooq_catalog();
            if args.json {
                println!("{}", serde_json::to_string_pretty(&catalog)?);
            } else {
                output::print_stooq_catalog(&catalog);
            }
            Ok(())
        }
        StooqCommand::Sync(args) => {
            let report = service::stooq_sync(
                runtime,
                service::StooqSyncRequest {
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

async fn run_news(runtime: &MarketRuntime, args: NewsArgs) -> Result<()> {
    let report = service::news(
        runtime,
        service::NewsRequest {
            symbol: args.symbol,
            count: args.count,
            refresh: args.refresh,
            cache_ttl_seconds: args.cache_ttl_seconds,
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_read_url(runtime: &MarketRuntime, args: ReadUrlArgs) -> Result<()> {
    let max_chars = if args.json { 0 } else { args.max_chars };
    let report = service::read_url(
        runtime,
        service::ReadUrlRequest {
            url: args.url,
            provider: args.provider,
            max_chars,
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_page_read_report(&report);
    }
    Ok(())
}

async fn run_search(runtime: &MarketRuntime, args: SearchArgs) -> Result<()> {
    let report = service::search(
        runtime,
        service::SearchRequest {
            query: args.query,
            quotes_count: args.quotes_count,
            news_count: args.news_count,
            refresh: args.refresh,
            cache_ttl_seconds: args.cache_ttl_seconds,
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_screen(runtime: &MarketRuntime, args: ScreenArgs) -> Result<()> {
    let report = service::screen(
        runtime,
        service::ScreenRequest {
            screener: args.screener,
            count: args.count,
            refresh: args.refresh,
            cache_ttl_seconds: args.cache_ttl_seconds,
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        output::print_search_report(&report, args.raw)?;
    }
    Ok(())
}

async fn run_watch(runtime: &MarketRuntime, args: WatchArgs) -> Result<()> {
    let json = args.json;
    let mut had_errors = false;
    service::watch_each(
        runtime,
        service::WatchRequest {
            symbols: args.symbols,
            asset: args.asset,
            instrument: args.instrument,
            crypto_provider: args.crypto_provider,
            interval_seconds: args.interval_seconds,
            iterations: args.iterations,
        },
        |batch| {
            had_errors |= batch.has_errors();
            match batch {
                WatchResponse::Crypto(batch) if json => {
                    println!("{}", serde_json::to_string_pretty(&batch)?);
                }
                WatchResponse::Crypto(batch) => {
                    output::print_crypto_price_points(&batch.points, &batch.errors);
                    println!();
                }
                WatchResponse::Equity(summaries) if json => {
                    println!("{}", serde_json::to_string_pretty(&summaries)?);
                }
                WatchResponse::Equity(summaries) => {
                    for summary in &summaries {
                        output::print_price_summary(summary, false);
                        println!();
                    }
                }
            }
            Ok(())
        },
    )
    .await?;
    if had_errors {
        Err(anyhow!("one or more crypto watch quotes failed"))
    } else {
        Ok(())
    }
}

async fn run_stream(runtime: &MarketRuntime, args: crate::cli::StreamArgs) -> Result<()> {
    if args.messages == 0 {
        let json = args.json;
        service::stream_quotes_each(
            runtime,
            service::StreamRequest {
                url: args.url,
                symbols: args.symbols,
                messages: args.messages,
            },
            |quote| {
                if json {
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

    let json = args.json;
    let updates = service::stream_quotes(
        runtime,
        service::StreamRequest {
            url: args.url,
            symbols: args.symbols,
            messages: args.messages,
        },
    )
    .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&updates)?);
    } else {
        output::print_stream_quotes(&updates);
    }
    Ok(())
}
