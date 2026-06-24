use std::collections::BTreeMap;

use anyhow::Result;

use crate::model::{
    DerivedIndicator, HistoryBatch, PredictionMarketReport, PredictionMarketSummary,
    PredictionOutcome, PredictionSearchReport, PricePoint, PriceSummary, ProviderProfile,
    ResearchReport, SearchReport, StreamQuote,
};
use crate::page_read::PageReadReport;
use crate::providers::binance::{CryptoSentimentReport, CryptoSnapshotReport, CryptoStreamReport};
use crate::time::utc_to_local;

pub fn print_price_summary(summary: &PriceSummary, show_all: bool) {
    println!(
        "{} price summary  fetched={}  tz={}",
        summary.symbol, summary.fetched_at_local, summary.timezone
    );
    if let Some(current) = summary.current.as_ref() {
        println!(
            "Current: {} {}  session={}  source={}  change={}  time={}",
            currency(current.currency.as_deref()),
            money_value(current.price),
            current.session.as_deref().unwrap_or("-"),
            current.provider,
            pct_value(current.change_pct),
            current.market_time_local.as_deref().unwrap_or("-")
        );
    } else {
        println!("Current: no quote available");
    }
    println!(
        "Regular basis: prev_close={} open={} high={} low={} volume={}",
        money_value(summary.regular_basis.previous_close),
        money_value(summary.regular_basis.open),
        money_value(summary.regular_basis.high),
        money_value(summary.regular_basis.low),
        number_value(summary.regular_basis.volume.map(|value| value as f64))
    );
    if let Some(proxy) = summary.proxy.as_ref() {
        println!(
            "Proxy: {} {} via {} time={} note={}",
            currency(proxy.currency.as_deref()),
            money_value(proxy.price),
            proxy.provider,
            proxy.market_time_local.as_deref().unwrap_or("-"),
            proxy.note.as_deref().unwrap_or("-")
        );
    }
    if show_all {
        println!();
        println!("Session / provider split");
        let headers = [
            "label", "price", "chg%", "session", "provider", "time", "open", "high", "low",
            "volume",
        ];
        let rows = summary
            .sessions
            .iter()
            .map(price_point_row)
            .collect::<Vec<_>>();
        print_table(&headers, &rows);
    } else if summary.sessions.len() > 1 {
        println!(
            "Note: fetched {} session/provider rows; use sessions to inspect the split.",
            summary.sessions.len()
        );
    }
    if !summary.errors.is_empty() {
        println!();
        println!("Quote errors");
        for (provider, error) in &summary.errors {
            println!("{provider}: {error}");
        }
    }
}

pub fn print_history_table(history: &HistoryBatch, timezone: &str) {
    println!(
        "{} history via {} interval={} adjustment={} actions={} repair_requested={} repair_applied={}",
        history.symbol,
        history.provider,
        history.interval,
        history.adjustment,
        history.actions_included,
        history.repair_requested,
        history.repair_applied
    );
    let headers = [
        "time",
        "open",
        "high",
        "low",
        "close",
        "adj_close",
        "volume",
        "dividend",
        "split",
        "gain",
        "repair",
    ];
    let rows = history
        .bars
        .iter()
        .map(|bar| {
            vec![
                local_or_original(&bar.open_time, timezone),
                money_value(bar.open),
                money_value(bar.high),
                money_value(bar.low),
                money_value(Some(bar.close)),
                money_value(bar.adj_close),
                number_value(bar.volume),
                money_value(bar.dividend),
                number_value(bar.stock_split),
                money_value(bar.capital_gain),
                if bar.repaired { "yes" } else { "-" }.to_string(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
}

pub fn print_indicator_table(indicators: &[DerivedIndicator], errors: &BTreeMap<String, String>) {
    let headers = [
        "symbol", "close", "1bar", "5bar", "20bar", "sma20", "sma50", "hi20", "lo20", "rv20",
    ];
    let mut rows = indicators
        .iter()
        .map(|indicator| {
            vec![
                indicator.symbol.clone(),
                money_value(indicator.latest_close),
                pct_value(indicator.return_1_bar_pct),
                pct_value(indicator.return_5_bar_pct),
                pct_value(indicator.return_20_bar_pct),
                money_value(indicator.sma_20),
                money_value(indicator.sma_50),
                money_value(indicator.high_20),
                money_value(indicator.low_20),
                pct_value(indicator.realized_vol_20_annualized_pct),
            ]
        })
        .collect::<Vec<_>>();

    for (symbol, error) in errors {
        rows.push(vec![
            symbol.clone(),
            "ERROR".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            error.clone(),
        ]);
    }
    print_table(&headers, &rows);
}

pub fn print_crypto_snapshot(
    report: &CryptoSnapshotReport,
    timezone: &str,
    raw: bool,
) -> Result<()> {
    println!(
        "{} crypto snapshot via {} fetched={}",
        report.symbol,
        report.provider,
        local_or_original(&report.fetched_at_utc, timezone)
    );
    print_crypto_section("spot", &report.spot);
    print_crypto_section("futures", &report.futures);
    print_crypto_errors(&report.errors);
    if raw {
        println!();
        println!("{}", serde_json::to_string_pretty(report)?);
    }
    Ok(())
}

pub fn print_crypto_sentiment(
    report: &CryptoSentimentReport,
    timezone: &str,
    raw: bool,
) -> Result<()> {
    println!(
        "{} crypto sentiment via {} fetched={}",
        report.symbol,
        report.provider,
        local_or_original(&report.fetched_at_utc, timezone)
    );
    print_crypto_section("futures", &report.futures);
    print_crypto_errors(&report.errors);
    if raw {
        println!();
        println!("{}", serde_json::to_string_pretty(report)?);
    }
    Ok(())
}

pub fn print_page_read_report(report: &PageReadReport) {
    println!(
        "URL reader via {} fetched={} words={} chars={} truncated={}",
        report.provider,
        report.fetched_at_utc,
        report.word_count,
        report.char_count,
        if report.truncated { "yes" } else { "no" }
    );
    println!("url: {}", report.url);
    println!("source_url: {}", report.source_url);
    if let Some(title) = report.title.as_deref() {
        println!("title: {title}");
    }
    if !report.errors.is_empty() {
        println!();
        println!("Fallback errors:");
        for error in &report.errors {
            println!("{}: {}", error.provider, error.error);
        }
    }
    println!();
    println!("{}", report.content);
}

pub fn print_research_report(report: &ResearchReport, raw: bool) -> Result<()> {
    println!(
        "{} {} fetched={}",
        report.symbol, report.category, report.fetched_at_local
    );
    if !report.sources.is_empty() {
        println!(
            "sources: {}",
            report
                .sources
                .iter()
                .map(|source| format!("{}:{}", source.provider, source.cache_status))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !report.modules.is_empty() {
        println!(
            "modules: {}",
            report
                .modules
                .iter()
                .map(|module| format!("{}:{}:{}", module.provider, module.name, module.status))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if report.highlights.is_empty() {
        println!("No highlights extracted; use --json to inspect the raw payload.");
    } else {
        let headers = ["source", "module", "field", "value"];
        let rows = report
            .highlights
            .iter()
            .map(|row| {
                vec![
                    row.provider.clone(),
                    row.module.clone(),
                    row.label.clone(),
                    row.value.clone(),
                ]
            })
            .collect::<Vec<_>>();
        print_table(&headers, &rows);
    }
    if !report.coverage_gaps.is_empty() {
        println!();
        println!("Coverage Gaps");
        println!("-------------");
        for gap in &report.coverage_gaps {
            println!("{}: {}", gap.module, gap.reason);
        }
    }
    if raw {
        println!();
        println!("{}", serde_json::to_string_pretty(&report.payload)?);
    }
    Ok(())
}

pub fn print_search_report(report: &SearchReport, raw: bool) -> Result<()> {
    println!(
        "{} {} via {} fetched={} cache={}",
        report.category,
        report.query,
        report.provider,
        report.fetched_at_local,
        report.cache_status
    );
    if report.highlights.is_empty() {
        println!("No highlights extracted; use --json to inspect the raw payload.");
    } else {
        let headers = ["source", "item", "value"];
        let rows = report
            .highlights
            .iter()
            .map(|row| vec![row.provider.clone(), row.label.clone(), row.value.clone()])
            .collect::<Vec<_>>();
        print_table(&headers, &rows);
    }
    if raw {
        println!();
        println!("{}", serde_json::to_string_pretty(&report.payload)?);
    }
    Ok(())
}

pub fn print_prediction_search_report(report: &PredictionSearchReport) {
    println!(
        "{} search '{}' fetched={} cache={}",
        report.provider, report.query, report.fetched_at_local, report.cache_status
    );
    println!("{}", report.interpretation_note);
    print_source_urls(&report.source_urls);
    print_prediction_markets(&report.markets);
}

pub fn print_prediction_market_report(report: &PredictionMarketReport) {
    println!(
        "{} market '{}' fetched={} cache={} enrichment={} enrichment_fetched={}",
        report.provider,
        report.identifier,
        report.fetched_at_local,
        report.cache_status,
        report.enrichment_status,
        report.enrichment_fetched_at_local
    );
    println!("{}", report.interpretation_note);
    print_source_urls(&report.source_urls);
    print_prediction_markets(std::slice::from_ref(&report.market));
    println!();
    println!("Outcomes");
    print_prediction_outcomes(&report.outcomes);
    if !report.price_history.is_empty() {
        println!();
        println!("Price history");
        let headers = ["time", "price"];
        let rows = report
            .price_history
            .iter()
            .rev()
            .take(12)
            .map(|point| {
                vec![
                    point
                        .time_local
                        .as_deref()
                        .or(point.time_utc.as_deref())
                        .unwrap_or("-")
                        .to_string(),
                    money_value(Some(point.price)),
                ]
            })
            .collect::<Vec<_>>();
        print_table(&headers, &rows);
    }
    if report.open_interest.is_some() || report.holder_preview_count.is_some() {
        println!();
        println!(
            "Data API: open_interest={} holder_preview_rows={}",
            number_value(report.open_interest),
            report
                .holder_preview_count
                .map_or_else(|| "-".to_string(), |value| value.to_string())
        );
    }
    if !report.data_errors.is_empty() {
        println!();
        println!("Partial data errors");
        for (source, error) in &report.data_errors {
            println!("{source}: {error}");
        }
    }
}

fn print_source_urls(urls: &[String]) {
    if urls.is_empty() {
        return;
    }
    println!("Sources:");
    for url in urls {
        println!("- {url}");
    }
}

fn print_prediction_markets(markets: &[PredictionMarketSummary]) {
    if markets.is_empty() {
        println!("No markets matched after local filtering.");
        return;
    }
    let headers = [
        "market", "active", "closed", "prob", "bid", "ask", "spread", "vol24h", "liq", "end",
    ];
    let rows = markets
        .iter()
        .map(|market| {
            vec![
                market.title.clone(),
                bool_text(market.active),
                bool_text(market.closed),
                market
                    .outcomes
                    .first()
                    .and_then(|outcome| outcome.implied_probability)
                    .map(pct_from_unit)
                    .unwrap_or_else(|| "-".to_string()),
                money_value(market.best_bid),
                money_value(market.best_ask),
                money_value(market.spread),
                number_value(market.volume_24hr),
                number_value(market.liquidity),
                market
                    .end_time_local
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
}

fn print_prediction_outcomes(outcomes: &[PredictionOutcome]) {
    if outcomes.is_empty() {
        println!("No outcomes found.");
        return;
    }
    let headers = [
        "outcome", "prob", "bid", "ask", "spread", "last", "bids", "asks", "token",
    ];
    let rows = outcomes
        .iter()
        .map(|outcome| {
            vec![
                outcome.label.clone(),
                outcome
                    .implied_probability
                    .map(pct_from_unit)
                    .unwrap_or_else(|| "-".to_string()),
                money_value(outcome.best_bid),
                money_value(outcome.best_ask),
                money_value(outcome.spread),
                money_value(outcome.last_trade_price),
                outcome.bid_count.to_string(),
                outcome.ask_count.to_string(),
                outcome
                    .clob_token_id
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
}

pub fn print_provider_profiles(profiles: &[ProviderProfile]) {
    let headers = [
        "provider",
        "key",
        "official",
        "stability",
        "large",
        "best_for",
    ];
    let rows = profiles
        .iter()
        .map(|profile| {
            vec![
                profile.provider.clone(),
                if profile.requires_api_key {
                    "required".to_string()
                } else {
                    "no".to_string()
                },
                profile.official_status.clone(),
                profile.stability.clone(),
                profile.large_download.to_string(),
                profile.best_for.clone(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);

    println!();
    println!("Capabilities");
    println!("------------");
    let headers = ["provider", "module", "status", "implemented", "note"];
    let rows = profiles
        .iter()
        .flat_map(|profile| {
            profile.capabilities.iter().map(|capability| {
                vec![
                    profile.provider.clone(),
                    capability.module.clone(),
                    capability.status.clone(),
                    capability.implemented.to_string(),
                    capability.note.clone(),
                ]
            })
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
}

pub fn print_stooq_catalog(catalog: &crate::model::StooqCatalog) {
    println!(
        "Stooq bulk catalog fetched={} source={}",
        catalog.fetched_at_utc, catalog.source_url
    );
    let headers = [
        "frequency",
        "market",
        "asset",
        "size_mb",
        "cached",
        "cache_key",
        "label",
    ];
    let rows = catalog
        .entries
        .iter()
        .map(|entry| {
            vec![
                entry.frequency.clone(),
                entry.market.clone(),
                entry.asset.clone(),
                number_value(entry.approx_size_mb),
                entry
                    .cached_zip_path
                    .clone()
                    .unwrap_or_else(|| "no".to_string()),
                entry.cache_key.clone(),
                entry.label.clone(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
    println!();
    println!(
        "Download note: Stooq bulk download links are captcha-authorized. Use `market stooq sync --zip-path <file>` or `market stooq sync --url <authorized-url>`."
    );
}

pub fn print_stooq_sync_report(report: &crate::model::StooqSyncReport) {
    println!(
        "Stooq synced {} {} {} bytes={} path={}",
        report.frequency, report.market, report.asset, report.bytes, report.zip_path
    );
    println!("source: {}", report.source);
    println!("imported_at_utc: {}", report.imported_at_utc);
}

pub fn print_stream_quotes(updates: &[StreamQuote]) {
    let headers = [
        "symbol",
        "price",
        "chg%",
        "market_hours",
        "time",
        "exchange",
        "volume",
        "name",
    ];
    let rows = updates
        .iter()
        .map(|quote| {
            vec![
                quote.symbol.clone(),
                money_value(Some(quote.price)),
                pct_value(quote.change_pct),
                quote
                    .market_hours
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                quote.time_local.clone().unwrap_or_else(|| "-".to_string()),
                quote.exchange.clone().unwrap_or_else(|| "-".to_string()),
                quote
                    .day_volume
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                quote.short_name.clone().unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
}

pub fn print_crypto_stream(report: &CryptoStreamReport) -> Result<()> {
    println!(
        "{} {} stream  provider={}  kind={}  fetched={}",
        report.symbol, report.market, report.provider, report.kind, report.fetched_at_utc
    );
    if let Some(interval) = report.interval.as_deref() {
        println!("interval: {interval}");
    }
    for (index, message) in report.messages.iter().enumerate() {
        println!("{}. {}", index + 1, serde_json::to_string_pretty(message)?);
    }
    Ok(())
}

pub fn print_crypto_price_points(points: &[PricePoint], errors: &BTreeMap<String, String>) {
    let headers = ["symbol", "price", "provider", "session", "time", "note"];
    let rows = points
        .iter()
        .map(|point| {
            vec![
                point.symbol.clone(),
                money_value(point.price),
                point.provider.clone(),
                point.session.clone().unwrap_or_else(|| "-".to_string()),
                point
                    .market_time_local
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                point.note.clone().unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows);
    if !errors.is_empty() {
        println!();
        println!("errors");
        println!("------");
        for (symbol, error) in errors {
            println!("{symbol}: {error}");
        }
    }
}

fn price_point_row(point: &PricePoint) -> Vec<String> {
    vec![
        point.label.clone(),
        money_value(point.price),
        pct_value(point.change_pct),
        point.session.clone().unwrap_or_else(|| "-".to_string()),
        point.provider.clone(),
        point
            .market_time_local
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        money_value(point.open),
        money_value(point.high),
        money_value(point.low),
        number_value(point.volume.map(|value| value as f64)),
    ]
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    println!("{}", table_row(headers.iter().copied(), &widths));
    println!(
        "{}",
        table_row(widths.iter().map(|width| "-".repeat(*width)), &widths)
    );
    for row in rows {
        println!("{}", table_row(row.iter().map(String::as_str), &widths));
    }
}

fn table_row<I, S>(values: I, widths: &[usize]) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .zip(widths.iter())
        .map(|(value, width)| format!("{:<width$}", value.as_ref()))
        .collect::<Vec<_>>()
        .join("  ")
}

fn local_or_original(value: &str, timezone: &str) -> String {
    utc_to_local(Some(value), timezone).unwrap_or_else(|| value.to_string())
}

fn print_crypto_section(name: &str, values: &BTreeMap<String, serde_json::Value>) {
    if values.is_empty() {
        return;
    }
    println!();
    println!("{name}");
    println!("{}", "-".repeat(name.len()));
    for (key, value) in values {
        print!("{key}: ");
        print_crypto_value_preview(value);
    }
}

fn print_crypto_errors(errors: &BTreeMap<String, String>) {
    if errors.is_empty() {
        return;
    }
    println!();
    println!("errors");
    println!("------");
    for (key, error) in errors {
        println!("{key}: {error}");
    }
}

fn print_crypto_value_preview(value: &serde_json::Value) {
    if let Some(object) = value.as_object() {
        let fields = [
            "symbol",
            "price",
            "lastPrice",
            "markPrice",
            "indexPrice",
            "lastFundingRate",
            "openInterest",
            "priceChangePercent",
            "volume",
            "quoteVolume",
            "count",
        ];
        let preview = fields
            .iter()
            .filter_map(|field| {
                object
                    .get(*field)
                    .map(|value| format!("{field}={}", compact_json(value)))
            })
            .collect::<Vec<_>>();
        if preview.is_empty() {
            println!("object keys={}", object.len());
        } else {
            println!("{}", preview.join(" "));
        }
    } else if let Some(values) = value.as_array() {
        println!("array len={}", values.len());
        if let Some(first) = values.first() {
            print!("  first: ");
            print_crypto_value_preview(first);
        }
    } else {
        println!("{}", compact_json(value));
    }
}

fn compact_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn money_value(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("${value:.2}"),
        None => "-".to_string(),
    }
}

fn currency(value: Option<&str>) -> &str {
    value.unwrap_or("USD")
}

fn number_value(value: Option<f64>) -> String {
    match value {
        Some(value) => {
            let formatted = format!("{value:.4}");
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
        None => "-".to_string(),
    }
}

fn bool_text(value: Option<bool>) -> String {
    match value {
        Some(true) => "yes".to_string(),
        Some(false) => "no".to_string(),
        None => "-".to_string(),
    }
}

fn pct_from_unit(value: f64) -> String {
    format!("{:.2}%", value * 100.0)
}

fn pct_value(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:+.2}%"),
        None => "-".to_string(),
    }
}
