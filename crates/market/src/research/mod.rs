use anyhow::{Result, anyhow};
use serde_json::Value;
use wreq::Client;

mod fetchers;
mod highlights;

use fetchers::*;
use highlights::*;

use crate::args::{OptionsProvider, ResearchProvider};
use crate::cache;
use crate::model::{
    ResearchCoverageGap, ResearchHighlight, ResearchModule, ResearchReport, ResearchSource,
    SearchReport,
};
use crate::providers::{cnbc, robinhood, sec_edgar};
use crate::time::{now_local, utc_to_local};

const FUNDAMENTALS_MODULES: &[&str] = &[
    "price",
    "summaryProfile",
    "summaryDetail",
    "defaultKeyStatistics",
    "financialData",
    "incomeStatementHistory",
    "incomeStatementHistoryQuarterly",
    "balanceSheetHistory",
    "balanceSheetHistoryQuarterly",
    "cashflowStatementHistory",
    "cashflowStatementHistoryQuarterly",
    "earnings",
];

const ANALYSIS_MODULES: &[&str] = &[
    "financialData",
    "recommendationTrend",
    "upgradeDowngradeHistory",
    "earningsTrend",
    "earningsHistory",
];

const OWNERSHIP_MODULES: &[&str] = &[
    "majorHoldersBreakdown",
    "institutionOwnership",
    "fundOwnership",
    "insiderHolders",
    "insiderTransactions",
    "netSharePurchaseActivity",
];

const EVENTS_MODULES: &[&str] = &[
    "calendarEvents",
    "secFilings",
    "summaryDetail",
    "earnings",
    "price",
];

#[derive(Debug, Clone, Copy)]
pub enum QuoteSummaryKind {
    Fundamentals,
    Analysis,
    Ownership,
    Events,
}

impl QuoteSummaryKind {
    pub fn label(self) -> &'static str {
        match self {
            QuoteSummaryKind::Fundamentals => "fundamentals",
            QuoteSummaryKind::Analysis => "analysis",
            QuoteSummaryKind::Ownership => "ownership",
            QuoteSummaryKind::Events => "events",
        }
    }

    fn modules(self) -> &'static [&'static str] {
        match self {
            QuoteSummaryKind::Fundamentals => FUNDAMENTALS_MODULES,
            QuoteSummaryKind::Analysis => ANALYSIS_MODULES,
            QuoteSummaryKind::Ownership => OWNERSHIP_MODULES,
            QuoteSummaryKind::Events => EVENTS_MODULES,
        }
    }

    fn highlights(self, root: Option<&Value>) -> Vec<ResearchHighlight> {
        match self {
            QuoteSummaryKind::Fundamentals => fundamentals_highlights(root),
            QuoteSummaryKind::Analysis => analysis_highlights(root),
            QuoteSummaryKind::Ownership => ownership_highlights(root),
            QuoteSummaryKind::Events => events_highlights(root),
        }
    }

    fn sec_supported(self) -> bool {
        matches!(
            self,
            QuoteSummaryKind::Fundamentals | QuoteSummaryKind::Events
        )
    }
}

pub async fn quote_summary_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    provider: ResearchProvider,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    match provider {
        ResearchProvider::Yahoo => {
            yahoo_quote_summary_report(client, symbol, kind, timezone, refresh, ttl_seconds).await
        }
        ResearchProvider::SecEdgar => {
            sec_edgar_report(client, symbol, kind, timezone, refresh, ttl_seconds).await
        }
        ResearchProvider::Robinhood => {
            robinhood_report(client, symbol, kind, timezone, refresh, ttl_seconds).await
        }
        ResearchProvider::Cnbc => {
            cnbc_report(client, symbol, kind, timezone, refresh, ttl_seconds).await
        }
        ResearchProvider::Auto => {
            auto_quote_summary_report(client, symbol, kind, timezone, refresh, ttl_seconds).await
        }
    }
}

async fn yahoo_quote_summary_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    let normalized = symbol.to_uppercase();
    let category = kind.label();
    let modules = kind.modules();
    let key = format!("{}:{}", category, normalized);
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("yahoo-quote-summary", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_quote_summary_live(client, &normalized, modules, &key).await?
        }
    } else {
        fetch_quote_summary_live(client, &normalized, modules, &key).await?
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: category.to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::Yahoo.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "Yahoo Finance quoteSummary payload",
        )],
        modules: modules
            .iter()
            .map(|module| module_status(module, ResearchProvider::Yahoo.label(), "requested", None))
            .collect(),
        coverage_gaps: Vec::new(),
        highlights: kind.highlights(quote_summary_root(&payload)),
        payload,
    })
}

async fn auto_quote_summary_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    let mut reports = Vec::new();
    let mut provider_gaps = Vec::new();

    match kind {
        QuoteSummaryKind::Fundamentals => {
            let (yahoo_result, sec_result, robinhood_result, cnbc_result) = tokio::join!(
                yahoo_quote_summary_report(client, symbol, kind, timezone, refresh, ttl_seconds),
                sec_edgar_report(client, symbol, kind, timezone, refresh, ttl_seconds),
                robinhood_report(client, symbol, kind, timezone, refresh, ttl_seconds),
                cnbc_report(client, symbol, kind, timezone, refresh, ttl_seconds)
            );
            collect_report_result(
                ResearchProvider::Yahoo.label(),
                yahoo_result,
                &mut reports,
                &mut provider_gaps,
            );
            collect_report_result(
                ResearchProvider::SecEdgar.label(),
                sec_result,
                &mut reports,
                &mut provider_gaps,
            );
            collect_report_result(
                ResearchProvider::Robinhood.label(),
                robinhood_result,
                &mut reports,
                &mut provider_gaps,
            );
            collect_report_result(
                ResearchProvider::Cnbc.label(),
                cnbc_result,
                &mut reports,
                &mut provider_gaps,
            );
        }
        QuoteSummaryKind::Events => {
            let (yahoo_result, sec_result, robinhood_result) = tokio::join!(
                yahoo_quote_summary_report(client, symbol, kind, timezone, refresh, ttl_seconds),
                sec_edgar_report(client, symbol, kind, timezone, refresh, ttl_seconds),
                robinhood_report(client, symbol, kind, timezone, refresh, ttl_seconds)
            );
            collect_report_result(
                ResearchProvider::Yahoo.label(),
                yahoo_result,
                &mut reports,
                &mut provider_gaps,
            );
            collect_report_result(
                ResearchProvider::SecEdgar.label(),
                sec_result,
                &mut reports,
                &mut provider_gaps,
            );
            collect_report_result(
                ResearchProvider::Robinhood.label(),
                robinhood_result,
                &mut reports,
                &mut provider_gaps,
            );
        }
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => {
            collect_report_result(
                ResearchProvider::Yahoo.label(),
                yahoo_quote_summary_report(client, symbol, kind, timezone, refresh, ttl_seconds)
                    .await,
                &mut reports,
                &mut provider_gaps,
            );
        }
    }

    if reports.is_empty() {
        return Err(anyhow!(
            "all research providers failed for {} {}: {}",
            symbol.to_uppercase(),
            kind.label(),
            provider_gaps
                .iter()
                .map(|gap| format!("{}={}", gap.module, gap.reason))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    Ok(merge_reports(reports, provider_gaps, timezone))
}

fn collect_report_result(
    provider: &str,
    result: Result<ResearchReport>,
    reports: &mut Vec<ResearchReport>,
    provider_gaps: &mut Vec<ResearchCoverageGap>,
) {
    match result {
        Ok(report) => reports.push(report),
        Err(error) => provider_gaps.push(gap(
            provider,
            format!("{provider} supplement failed: {error:#}"),
        )),
    }
}

async fn sec_edgar_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    if !kind.sec_supported() {
        return Err(anyhow!(
            "sec-edgar does not support {}; use --provider yahoo or --provider auto",
            kind.label()
        ));
    }

    let normalized = symbol.to_uppercase();
    let key = format!("{}:company:{}", kind.label(), normalized);
    let include_companyfacts = matches!(kind, QuoteSummaryKind::Fundamentals);
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("sec-edgar-company", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_sec_company_live(client, &normalized, include_companyfacts, &key).await?
        }
    } else {
        fetch_sec_company_live(client, &normalized, include_companyfacts, &key).await?
    };

    let highlights = match kind {
        QuoteSummaryKind::Fundamentals => sec_edgar::fundamentals_highlights(&payload),
        QuoteSummaryKind::Events => sec_edgar::events_highlights(&payload),
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => unreachable!(),
    };
    let modules = match kind {
        QuoteSummaryKind::Fundamentals => vec![
            module_status(
                "companyfacts",
                ResearchProvider::SecEdgar.label(),
                "available",
                Some("official XBRL companyfacts"),
            ),
            module_status(
                "submissions",
                ResearchProvider::SecEdgar.label(),
                "available",
                Some("official company submissions"),
            ),
        ],
        QuoteSummaryKind::Events => vec![module_status(
            "submissions",
            ResearchProvider::SecEdgar.label(),
            "available",
            Some("official recent filings"),
        )],
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => unreachable!(),
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: kind.label().to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::SecEdgar.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "SEC official submissions and XBRL companyfacts",
        )],
        modules,
        coverage_gaps: sec_coverage_gaps(kind),
        highlights,
        payload,
    })
}

async fn robinhood_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    if !matches!(
        kind,
        QuoteSummaryKind::Fundamentals | QuoteSummaryKind::Events
    ) {
        return Err(anyhow!(
            "robinhood does not support {}; use --provider yahoo or --provider auto",
            kind.label()
        ));
    }

    let normalized = symbol.to_uppercase();
    let key = format!("{}:{}", kind.label(), normalized);
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("robinhood-research", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_robinhood_live(client, &normalized, kind, &key).await?
        }
    } else {
        fetch_robinhood_live(client, &normalized, kind, &key).await?
    };

    let highlights = match kind {
        QuoteSummaryKind::Fundamentals => robinhood::fundamentals_highlights(&payload),
        QuoteSummaryKind::Events => robinhood::events_highlights(&payload),
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => unreachable!(),
    };
    let modules = match kind {
        QuoteSummaryKind::Fundamentals => vec![
            module_status(
                "instrument",
                ResearchProvider::Robinhood.label(),
                "available",
                Some("public instrument profile"),
            ),
            module_status(
                "fundamentals",
                ResearchProvider::Robinhood.label(),
                "available",
                Some("public Robinhood fundamentals endpoint"),
            ),
        ],
        QuoteSummaryKind::Events => vec![
            module_status(
                "splits",
                ResearchProvider::Robinhood.label(),
                "available",
                Some("instrument split records"),
            ),
            module_status(
                "market_hours",
                ResearchProvider::Robinhood.label(),
                "available",
                Some("market session hours"),
            ),
        ],
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => unreachable!(),
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: kind.label().to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::Robinhood.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "Robinhood public stock endpoints",
        )],
        modules,
        coverage_gaps: robinhood_coverage_gaps(kind),
        highlights,
        payload,
    })
}

async fn cnbc_report(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    if !matches!(kind, QuoteSummaryKind::Fundamentals) {
        return Err(anyhow!(
            "cnbc does not support {}; use --provider yahoo or --provider auto",
            kind.label()
        ));
    }

    let normalized = symbol.to_uppercase();
    let key = format!("fundamentals:{}", normalized);
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("cnbc-fundamentals-lite", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_cnbc_live(client, &normalized, &key).await?
        }
    } else {
        fetch_cnbc_live(client, &normalized, &key).await?
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: kind.label().to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::Cnbc.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "CNBC quote payload fundamentals-lite fields",
        )],
        modules: vec![module_status(
            "fundamentals-lite",
            ResearchProvider::Cnbc.label(),
            "available",
            Some("valuation and TTM summary fields from quote payload"),
        )],
        coverage_gaps: vec![gap(
            "statements/ownership/options",
            "CNBC public quote payload is a fundamentals-lite cross-check, not a full company research API",
        )],
        highlights: cnbc::fundamentals_highlights(&payload),
        payload,
    })
}

pub struct OptionsReportRequest<'a> {
    pub client: &'a Client,
    pub symbol: &'a str,
    pub provider: OptionsProvider,
    pub expiry: Option<i64>,
    pub expiration_date: Option<&'a str>,
    pub count: usize,
    pub timezone: &'a str,
    pub refresh: bool,
    pub ttl_seconds: u64,
}

pub async fn options_report(request: OptionsReportRequest<'_>) -> Result<ResearchReport> {
    let OptionsReportRequest {
        client,
        symbol,
        provider,
        expiry,
        expiration_date,
        count,
        timezone,
        refresh,
        ttl_seconds,
    } = request;

    match provider {
        OptionsProvider::Yahoo => {
            yahoo_options_report(client, symbol, expiry, timezone, refresh, ttl_seconds).await
        }
        OptionsProvider::Robinhood => {
            let expiration_from_epoch = expiry.and_then(epoch_to_date);
            let expiration_date = expiration_date.or(expiration_from_epoch.as_deref());
            robinhood_options_report(
                client,
                symbol,
                expiration_date,
                count,
                timezone,
                refresh,
                ttl_seconds,
            )
            .await
        }
        OptionsProvider::Auto => {
            let expiration = expiration_date
                .map(ToString::to_string)
                .or_else(|| expiry.and_then(epoch_to_date));
            let (yahoo_result, robinhood_result) = tokio::join!(
                yahoo_options_report(client, symbol, expiry, timezone, refresh, ttl_seconds),
                robinhood_options_report(
                    client,
                    symbol,
                    expiration.as_deref(),
                    count,
                    timezone,
                    refresh,
                    ttl_seconds
                )
            );
            let mut reports = Vec::new();
            let mut gaps = Vec::new();
            for (provider, result) in [
                (OptionsProvider::Yahoo.label(), yahoo_result),
                (OptionsProvider::Robinhood.label(), robinhood_result),
            ] {
                match result {
                    Ok(report) => reports.push(report),
                    Err(error) => gaps.push(gap(
                        provider,
                        format!("{provider} options failed or unsupported: {error:#}"),
                    )),
                }
            }
            if reports.is_empty() {
                return Err(anyhow!(
                    "all options providers failed for {}: {}",
                    symbol.to_uppercase(),
                    gaps.iter()
                        .map(|gap| format!("{}={}", gap.module, gap.reason))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            Ok(merge_reports(reports, gaps, timezone))
        }
    }
}

async fn yahoo_options_report(
    client: &Client,
    symbol: &str,
    expiry: Option<i64>,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    let normalized = symbol.to_uppercase();
    let key = format!("{}:{}", normalized, expiry.unwrap_or_default());
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("yahoo-options", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_options_live(client, &normalized, expiry, &key).await?
        }
    } else {
        fetch_options_live(client, &normalized, expiry, &key).await?
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: "options".to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::Yahoo.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "Yahoo Finance optionChain payload",
        )],
        modules: vec![module_status(
            "options",
            ResearchProvider::Yahoo.label(),
            "available",
            None,
        )],
        coverage_gaps: vec![gap(
            ResearchProvider::SecEdgar.label(),
            "SEC EDGAR does not provide option chains",
        )],
        highlights: options_highlights(&payload),
        payload,
    })
}

async fn robinhood_options_report(
    client: &Client,
    symbol: &str,
    expiration_date: Option<&str>,
    count: usize,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<ResearchReport> {
    let normalized = symbol.to_uppercase();
    let key = format!(
        "{}:{}:{}",
        normalized,
        expiration_date.unwrap_or("nearest"),
        count.max(1)
    );
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("robinhood-options", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_robinhood_options_live(client, &normalized, expiration_date, count, &key).await?
        }
    } else {
        fetch_robinhood_options_live(client, &normalized, expiration_date, count, &key).await?
    };

    Ok(ResearchReport {
        symbol: normalized,
        category: "options".to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc: fetched_at_utc.clone(),
        sources: vec![source(
            ResearchProvider::Robinhood.label(),
            &cache_status,
            &fetched_at_utc,
            timezone,
            "Robinhood public option chain and contract metadata",
        )],
        modules: vec![module_status(
            "option_instruments",
            ResearchProvider::Robinhood.label(),
            "available",
            Some("contract metadata; not quote/IV marketdata"),
        )],
        coverage_gaps: vec![gap(
            "option quotes/IV/greeks",
            "Robinhood anonymous chain metadata does not reliably expose option bid/ask, IV, or greeks",
        )],
        highlights: robinhood::options_highlights(&payload),
        payload,
    })
}

pub async fn news_report(
    client: &Client,
    symbol: &str,
    count: usize,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<SearchReport> {
    search_report_inner(SearchReportRequest {
        category: "news",
        client,
        query: symbol,
        quotes_count: 0,
        news_count: count,
        timezone,
        refresh,
        ttl_seconds,
    })
    .await
}

pub async fn search_report(
    client: &Client,
    query: &str,
    quotes_count: usize,
    news_count: usize,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<SearchReport> {
    search_report_inner(SearchReportRequest {
        category: "search",
        client,
        query,
        quotes_count,
        news_count,
        timezone,
        refresh,
        ttl_seconds,
    })
    .await
}

struct SearchReportRequest<'a> {
    category: &'static str,
    client: &'a Client,
    query: &'a str,
    quotes_count: usize,
    news_count: usize,
    timezone: &'a str,
    refresh: bool,
    ttl_seconds: u64,
}

async fn search_report_inner(request: SearchReportRequest<'_>) -> Result<SearchReport> {
    let quotes_count = request.quotes_count.clamp(0, 50);
    let news_count = request.news_count.clamp(0, 50);
    let key = format!(
        "{}:{}:{}:{}",
        request.category, request.query, quotes_count, news_count
    );
    let (fetched_at_utc, cache_status, payload) = if !request.refresh {
        if let Some((fetched_at_utc, payload)) =
            cache::read_json("yahoo-search", &key, request.ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_search_live(
                request.client,
                request.query,
                quotes_count,
                news_count,
                &key,
            )
            .await?
        }
    } else {
        fetch_search_live(
            request.client,
            request.query,
            quotes_count,
            news_count,
            &key,
        )
        .await?
    };

    Ok(SearchReport {
        category: request.category.to_string(),
        query: request.query.to_string(),
        provider: ResearchProvider::Yahoo.label().to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, request.timezone),
        fetched_at_utc,
        cache_status,
        highlights: search_highlights(&payload),
        payload,
    })
}

pub async fn screen_report(
    client: &Client,
    screener: &str,
    count: usize,
    timezone: &str,
    refresh: bool,
    ttl_seconds: u64,
) -> Result<SearchReport> {
    let count = count.clamp(1, 250);
    let key = format!("screen:{}:{}", screener, count);
    let (fetched_at_utc, cache_status, payload) = if !refresh {
        if let Some((fetched_at_utc, payload)) = cache::read_json("yahoo-screen", &key, ttl_seconds)
        {
            (fetched_at_utc, "hit".to_string(), payload)
        } else {
            fetch_screen_live(client, screener, count, &key).await?
        }
    } else {
        fetch_screen_live(client, screener, count, &key).await?
    };

    Ok(SearchReport {
        category: "screen".to_string(),
        query: screener.to_string(),
        provider: ResearchProvider::Yahoo.label().to_string(),
        fetched_at_local: fetched_local(&fetched_at_utc, timezone),
        fetched_at_utc,
        cache_status,
        highlights: screen_highlights(&payload),
        payload,
    })
}

fn merge_reports(
    reports: Vec<ResearchReport>,
    provider_gaps: Vec<ResearchCoverageGap>,
    timezone: &str,
) -> ResearchReport {
    let mut reports = reports.into_iter();
    let mut primary = reports
        .next()
        .expect("merge_reports requires at least one report");
    let mut payloads = serde_json::Map::new();
    payloads.insert(provider_payload_key(&primary.sources), primary.payload);

    for report in reports {
        payloads.insert(provider_payload_key(&report.sources), report.payload);
        primary.sources.extend(report.sources);
        primary.modules.extend(report.modules);
        primary.coverage_gaps.extend(report.coverage_gaps);
        primary.highlights.extend(report.highlights);
    }
    primary.coverage_gaps.extend(provider_gaps);
    if let Some(latest_source_time) = primary
        .sources
        .iter()
        .map(|source| source.fetched_at_utc.as_str())
        .max()
    {
        primary.fetched_at_utc = latest_source_time.to_string();
        primary.fetched_at_local = fetched_local(latest_source_time, timezone);
    }
    primary.payload = Value::Object(payloads);
    primary
}

fn provider_payload_key(sources: &[ResearchSource]) -> String {
    sources
        .first()
        .map(|source| source.provider.replace('-', "_"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn epoch_to_date(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|dt| dt.date_naive().to_string())
}

fn source(
    provider: &str,
    cache_status: &str,
    fetched_at_utc: &str,
    timezone: &str,
    note: &str,
) -> ResearchSource {
    ResearchSource {
        provider: provider.to_string(),
        cache_status: cache_status.to_string(),
        fetched_at_utc: fetched_at_utc.to_string(),
        fetched_at_local: fetched_local(fetched_at_utc, timezone),
        note: Some(note.to_string()),
    }
}

fn module_status(name: &str, provider: &str, status: &str, note: Option<&str>) -> ResearchModule {
    ResearchModule {
        name: name.to_string(),
        provider: provider.to_string(),
        status: status.to_string(),
        note: note.map(ToString::to_string),
    }
}

fn gap(module: impl Into<String>, reason: impl Into<String>) -> ResearchCoverageGap {
    ResearchCoverageGap {
        module: module.into(),
        reason: reason.into(),
    }
}

fn sec_coverage_gaps(kind: QuoteSummaryKind) -> Vec<ResearchCoverageGap> {
    match kind {
        QuoteSummaryKind::Fundamentals => vec![gap(
            "valuation/analyst metrics",
            "SEC EDGAR is official filings data and does not provide market valuation, analyst targets, or forward estimates",
        )],
        QuoteSummaryKind::Events => vec![gap(
            "earnings calendar",
            "SEC EDGAR provides filings after dissemination; it does not predict earnings dates",
        )],
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => Vec::new(),
    }
}

fn robinhood_coverage_gaps(kind: QuoteSummaryKind) -> Vec<ResearchCoverageGap> {
    match kind {
        QuoteSummaryKind::Fundamentals => vec![gap(
            "statements/analyst estimates",
            "Robinhood fundamentals are profile and valuation snapshots; use Yahoo/SEC for statements and analyst estimates",
        )],
        QuoteSummaryKind::Events => vec![gap(
            "earnings/dividend calendar",
            "Robinhood public events coverage is splits and market hours; use Yahoo/SEC for earnings, dividends, and filings",
        )],
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => Vec::new(),
    }
}

fn fetched_local(fetched_at_utc: &str, timezone: &str) -> String {
    utc_to_local(Some(fetched_at_utc), timezone).unwrap_or_else(|| now_local(timezone))
}

#[cfg(test)]
mod tests;
