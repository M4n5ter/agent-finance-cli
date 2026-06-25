use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use anyhow::{Result, anyhow};
use futures_util::StreamExt;

use crate::args::{
    AssetClass, CryptoInstrument, CryptoProvider, HistoryAdjustment, HistorySession,
    OptionsProvider, Provider, ReadUrlProvider, ResearchProvider, SessionMode, StooqAsset,
    StooqFrequency, StooqMarket,
};
use crate::crypto_capability::{CryptoCapability, resolve_instrument};
use crate::crypto_market_data::{
    CryptoIndicatorOptions, CryptoPriceBatch, fetch_history as fetch_crypto_history,
    fetch_price_batch,
};
use crate::crypto_runtime::{
    CryptoEvidenceSources, EvidenceEngine, EvidenceRequest, evidence_report,
};
use crate::http::http_client;
use crate::indicators::compute_indicator;
use crate::model::{
    HistoryBatch, PredictionMarketReport, PredictionSearchReport, PriceSummary, ResearchReport,
    SearchReport, StooqCatalog, StooqSyncReport, StreamQuote,
};
use crate::page_read;
use crate::price;
use crate::providers::{self, binance, stooq};
use crate::research::{self, QuoteSummaryKind};
use crate::stream;

pub use crate::crypto_runtime::CryptoEvidenceReport;
pub use crate::page_read::PageReadReport;
pub use crate::providers::binance::{
    CryptoSentimentReport, CryptoSnapshotReport, CryptoStreamReport,
};
pub use crate::research::QuoteSummaryKind as MarketQuoteSummaryKind;

#[derive(Clone)]
pub struct MarketRuntime {
    proxy: Option<String>,
    no_proxy: bool,
    timeout_seconds: u64,
    timezone: String,
}

impl fmt::Debug for MarketRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MarketRuntime")
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("no_proxy", &self.no_proxy)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("timezone", &self.timezone)
            .finish()
    }
}

impl MarketRuntime {
    pub fn new(proxy: Option<&str>, no_proxy: bool, timeout_seconds: u64, timezone: &str) -> Self {
        Self {
            proxy: proxy.map(ToString::to_string),
            no_proxy,
            timeout_seconds,
            timezone: timezone.to_string(),
        }
    }

    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    pub(crate) fn proxy(&self) -> Option<&str> {
        self.proxy.as_deref()
    }

    pub(crate) fn client(&self) -> Result<wreq::Client> {
        http_client(self.timeout_seconds, self.proxy(), self.no_proxy)
    }

    pub(crate) fn binance_config(&self) -> binance::BinanceConfig {
        binance::BinanceConfig::from_env(self.timeout_seconds, self.proxy(), self.no_proxy)
    }
}

#[derive(Debug, Clone)]
pub struct PriceRequest {
    pub symbols: Vec<String>,
    pub asset: AssetClass,
    pub instrument: CryptoInstrument,
    pub crypto_provider: CryptoProvider,
    pub session: SessionMode,
    pub proxy_symbol: Option<String>,
}

#[derive(Debug)]
pub enum PriceResponse {
    Equity(Vec<PriceSummary>),
    Crypto(CryptoPriceBatch),
}

impl PriceResponse {
    pub fn is_complete(&self) -> bool {
        match self {
            PriceResponse::Equity(summaries) => {
                summaries.iter().all(|summary| summary.current.is_some())
            }
            PriceResponse::Crypto(batch) => batch.errors.is_empty(),
        }
    }

    pub fn completion_error(&self) -> anyhow::Error {
        match self {
            PriceResponse::Equity(_) => anyhow!("one or more price summaries had no current quote"),
            PriceResponse::Crypto(_) => anyhow!("one or more crypto price quotes failed"),
        }
    }
}

pub async fn price(runtime: &MarketRuntime, request: PriceRequest) -> Result<PriceResponse> {
    if request.asset == AssetClass::Crypto {
        let client = runtime.client()?;
        let config = runtime.binance_config();
        let batch = fetch_price_batch(
            &client,
            &config,
            request.crypto_provider,
            resolve_instrument(request.instrument, CryptoCapability::Quote),
            request.symbols,
            &runtime.timezone,
        )
        .await;
        return Ok(PriceResponse::Crypto(batch));
    }

    let client = runtime.client()?;
    let binance_config = runtime.binance_config();
    let summaries = futures_util::stream::iter(request.symbols)
        .map(|symbol| {
            let client = &client;
            let binance_config = &binance_config;
            let proxy_symbol = request.proxy_symbol.as_deref();
            async move {
                price::fetch_price_summary(
                    client,
                    &symbol,
                    &runtime.timezone,
                    request.session,
                    Some(binance_config),
                    proxy_symbol,
                )
                .await
            }
        })
        .buffered(4)
        .collect::<Vec<_>>()
        .await;
    Ok(PriceResponse::Equity(summaries))
}

#[derive(Debug, Clone)]
pub struct SessionsRequest {
    pub symbol: String,
    pub proxy_symbol: Option<String>,
}

pub async fn sessions(runtime: &MarketRuntime, request: SessionsRequest) -> Result<PriceSummary> {
    let client = runtime.client()?;
    let binance_config = runtime.binance_config();
    Ok(price::fetch_price_summary(
        &client,
        &request.symbol,
        &runtime.timezone,
        SessionMode::All,
        Some(&binance_config),
        request.proxy_symbol.as_deref(),
    )
    .await)
}

#[derive(Debug, Clone)]
pub struct HistoryRequest {
    pub symbol: String,
    pub asset: AssetClass,
    pub instrument: CryptoInstrument,
    pub crypto_provider: CryptoProvider,
    pub provider: Provider,
    pub session: HistorySession,
    pub adjustment: HistoryAdjustment,
    pub no_actions: bool,
    pub repair: bool,
    pub interval: String,
    pub range: String,
    pub limit: usize,
    pub stooq_market: StooqMarket,
    pub stooq_asset: StooqAsset,
}

pub async fn history(runtime: &MarketRuntime, request: HistoryRequest) -> Result<HistoryBatch> {
    if request.asset == AssetClass::Crypto
        || matches!(
            request.provider,
            Provider::BinanceSpot | Provider::BinanceUsdsFutures
        )
    {
        let client = runtime.client()?;
        let config = runtime.binance_config();
        return fetch_crypto_history(
            &client,
            &config,
            provider_crypto_provider(request.provider, request.crypto_provider),
            provider_instrument(request.provider, request.instrument),
            &request.symbol,
            &request.interval,
            request.limit,
        )
        .await;
    }

    let client = runtime.client()?;
    let provider = effective_history_provider(request.provider, request.session);
    let request = providers::HistoryRequest {
        symbol: request.symbol,
        interval: request.interval,
        range: request.range,
        limit: request.limit,
        extended_session: matches!(request.session, HistorySession::Extended),
        adjustment: request.adjustment,
        actions: !request.no_actions,
        repair: request.repair,
        stooq_market: request.stooq_market,
        stooq_asset: request.stooq_asset,
    };
    providers::fetch_history(&client, provider, &request).await
}

#[derive(Debug, Clone)]
pub struct IndicatorsRequest {
    pub symbols: Vec<String>,
    pub asset: AssetClass,
    pub instrument: CryptoInstrument,
    pub crypto_provider: CryptoProvider,
    pub provider: Provider,
    pub session: HistorySession,
    pub adjustment: HistoryAdjustment,
    pub repair: bool,
    pub interval: String,
    pub range: String,
    pub limit: usize,
    pub stooq_market: StooqMarket,
    pub stooq_asset: StooqAsset,
}

pub async fn indicators(
    runtime: &MarketRuntime,
    request: IndicatorsRequest,
) -> Result<crate::crypto_market_data::IndicatorBatch> {
    if request.asset == AssetClass::Crypto
        || matches!(
            request.provider,
            Provider::BinanceSpot | Provider::BinanceUsdsFutures
        )
    {
        let client = runtime.client()?;
        let config = runtime.binance_config();
        let options = CryptoIndicatorOptions {
            symbols: request.symbols,
            provider: request.provider,
            crypto_provider: request.crypto_provider,
            instrument: request.instrument,
            interval: &request.interval,
            limit: request.limit,
        };
        return crate::crypto_market_data::fetch_indicator_batch(&client, &config, options).await;
    }

    let client = runtime.client()?;
    let provider = effective_history_provider(request.provider, request.session);
    let symbols = request
        .symbols
        .into_iter()
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect::<Vec<_>>();
    let results = futures_util::stream::iter(symbols)
        .map(|normalized| {
            let client = &client;
            let interval = request.interval.clone();
            let range = request.range.clone();
            async move {
                let history_request = providers::HistoryRequest {
                    symbol: normalized.clone(),
                    interval,
                    range,
                    limit: request.limit,
                    extended_session: matches!(request.session, HistorySession::Extended),
                    adjustment: request.adjustment,
                    actions: false,
                    repair: request.repair,
                    stooq_market: request.stooq_market,
                    stooq_asset: request.stooq_asset,
                };
                let result = providers::fetch_history(client, provider, &history_request).await;
                (normalized, result)
            }
        })
        .buffered(4)
        .collect::<Vec<_>>()
        .await;

    let mut indicators = Vec::new();
    let mut errors = BTreeMap::new();
    for (normalized, result) in results {
        match result {
            Ok(history) => indicators.push(compute_indicator(&history)),
            Err(error) => {
                errors.insert(normalized, format!("{error:#}"));
            }
        }
    }

    Ok(crate::crypto_market_data::IndicatorBatch { indicators, errors })
}

#[derive(Debug, Clone)]
pub struct QuoteSummaryRequest {
    pub symbol: String,
    pub kind: QuoteSummaryKind,
    pub provider: ResearchProvider,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn quote_summary(
    runtime: &MarketRuntime,
    request: QuoteSummaryRequest,
) -> Result<ResearchReport> {
    let client = runtime.client()?;
    research::quote_summary_report(
        &client,
        &request.symbol,
        request.kind,
        request.provider,
        &runtime.timezone,
        request.refresh,
        request.cache_ttl_seconds,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct OptionsRequest {
    pub symbol: String,
    pub provider: OptionsProvider,
    pub expiry: Option<i64>,
    pub expiration_date: Option<String>,
    pub count: usize,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn options(runtime: &MarketRuntime, request: OptionsRequest) -> Result<ResearchReport> {
    let client = runtime.client()?;
    research::options_report(research::OptionsReportRequest {
        client: &client,
        symbol: &request.symbol,
        provider: request.provider,
        expiry: request.expiry,
        expiration_date: request.expiration_date.as_deref(),
        count: request.count,
        timezone: &runtime.timezone,
        refresh: request.refresh,
        ttl_seconds: request.cache_ttl_seconds,
    })
    .await
}

#[derive(Debug, Clone)]
pub struct NewsRequest {
    pub symbol: String,
    pub count: usize,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn news(runtime: &MarketRuntime, request: NewsRequest) -> Result<SearchReport> {
    let client = runtime.client()?;
    research::news_report(
        &client,
        &request.symbol,
        request.count,
        &runtime.timezone,
        request.refresh,
        request.cache_ttl_seconds,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub quotes_count: usize,
    pub news_count: usize,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn search(runtime: &MarketRuntime, request: SearchRequest) -> Result<SearchReport> {
    let client = runtime.client()?;
    research::search_report(
        &client,
        &request.query,
        request.quotes_count,
        request.news_count,
        &runtime.timezone,
        request.refresh,
        request.cache_ttl_seconds,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct ScreenRequest {
    pub screener: String,
    pub count: usize,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn screen(runtime: &MarketRuntime, request: ScreenRequest) -> Result<SearchReport> {
    let client = runtime.client()?;
    research::screen_report(
        &client,
        &request.screener,
        request.count,
        &runtime.timezone,
        request.refresh,
        request.cache_ttl_seconds,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct ReadUrlRequest {
    pub url: String,
    pub provider: ReadUrlProvider,
    pub max_chars: usize,
}

pub async fn read_url(runtime: &MarketRuntime, request: ReadUrlRequest) -> Result<PageReadReport> {
    let client = runtime.client()?;
    page_read::read_url(&client, &request.url, request.provider, request.max_chars).await
}

pub fn provider_profiles() -> Vec<crate::model::ProviderProfile> {
    providers::capabilities::profiles()
}

pub fn stooq_catalog() -> StooqCatalog {
    stooq::catalog()
}

#[derive(Debug, Clone)]
pub struct StooqSyncRequest {
    pub frequency: StooqFrequency,
    pub market: StooqMarket,
    pub asset: StooqAsset,
    pub url: Option<String>,
    pub zip_path: Option<std::path::PathBuf>,
    pub force: bool,
}

pub async fn stooq_sync(
    runtime: &MarketRuntime,
    request: StooqSyncRequest,
) -> Result<StooqSyncReport> {
    let client = runtime.client()?;
    stooq::sync_bulk(
        &client,
        stooq::StooqSyncRequest {
            frequency: request.frequency,
            market: request.market,
            asset: request.asset,
            url: request.url,
            zip_path: request.zip_path,
            force: request.force,
        },
    )
    .await
}

#[derive(Debug, Clone)]
pub struct WatchRequest {
    pub symbols: Vec<String>,
    pub asset: AssetClass,
    pub instrument: CryptoInstrument,
    pub crypto_provider: CryptoProvider,
    pub interval_seconds: u64,
    pub iterations: usize,
}

#[derive(Debug)]
pub enum WatchResponse {
    Equity(Vec<PriceSummary>),
    Crypto(CryptoPriceBatch),
}

impl WatchResponse {
    pub fn has_errors(&self) -> bool {
        match self {
            WatchResponse::Equity(_) => false,
            WatchResponse::Crypto(batch) => !batch.errors.is_empty(),
        }
    }
}

pub async fn watch_each<F>(
    runtime: &MarketRuntime,
    request: WatchRequest,
    mut on_batch: F,
) -> Result<()>
where
    F: FnMut(WatchResponse) -> Result<()>,
{
    let client = runtime.client()?;
    let config = runtime.binance_config();
    let mut iteration = 0usize;
    loop {
        iteration += 1;
        if request.asset == AssetClass::Crypto {
            let batch = fetch_price_batch(
                &client,
                &config,
                request.crypto_provider,
                resolve_instrument(request.instrument, CryptoCapability::Quote),
                request.symbols.clone(),
                &runtime.timezone,
            )
            .await;
            on_batch(WatchResponse::Crypto(batch))?;
        } else {
            let last_summaries = futures_util::stream::iter(request.symbols.iter())
                .map(|symbol| {
                    let client = &client;
                    async move {
                        price::fetch_price_summary(
                            client,
                            symbol,
                            &runtime.timezone,
                            SessionMode::Smart,
                            None,
                            None,
                        )
                        .await
                    }
                })
                .buffered(4)
                .collect::<Vec<_>>()
                .await;
            on_batch(WatchResponse::Equity(last_summaries))?;
        }
        if request.iterations != 0 && iteration >= request.iterations {
            break;
        }
        tokio::time::sleep(Duration::from_secs(request.interval_seconds.max(1))).await;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct StreamRequest {
    pub url: String,
    pub symbols: Vec<String>,
    pub messages: usize,
}

pub async fn stream_quotes(
    runtime: &MarketRuntime,
    request: StreamRequest,
) -> Result<Vec<StreamQuote>> {
    stream::stream_quotes(stream::StreamOptions {
        url: &request.url,
        symbols: request.symbols,
        message_limit: request.messages,
        read_timeout: Duration::from_secs(runtime.timeout_seconds.max(1)),
        timezone: &runtime.timezone,
        proxy: runtime.proxy(),
        no_proxy: runtime.no_proxy,
    })
    .await
}

pub async fn stream_quotes_each<F>(
    runtime: &MarketRuntime,
    request: StreamRequest,
    on_quote: F,
) -> Result<()>
where
    F: FnMut(StreamQuote) -> Result<()>,
{
    stream::stream_quotes_each(
        stream::StreamOptions {
            url: &request.url,
            symbols: request.symbols,
            message_limit: request.messages,
            read_timeout: Duration::from_secs(runtime.timeout_seconds.max(1)),
            timezone: &runtime.timezone,
            proxy: runtime.proxy(),
            no_proxy: runtime.no_proxy,
        },
        on_quote,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct CryptoSymbolRequest {
    pub symbol: String,
}

pub async fn crypto_snapshot(
    runtime: &MarketRuntime,
    request: CryptoSymbolRequest,
) -> CryptoSnapshotReport {
    let config = runtime.binance_config();
    binance::snapshot(&config, &request.symbol).await
}

pub async fn crypto_sentiment(
    runtime: &MarketRuntime,
    request: CryptoSymbolRequest,
) -> CryptoSentimentReport {
    let config = runtime.binance_config();
    binance::sentiment(&config, &request.symbol).await
}

#[derive(Debug, Clone)]
pub struct CryptoStreamRequest {
    pub symbol: String,
    pub market: crate::args::CryptoMarket,
    pub kind: crate::args::CryptoStreamKind,
    pub interval: String,
    pub messages: usize,
}

pub async fn crypto_stream(
    runtime: &MarketRuntime,
    request: CryptoStreamRequest,
) -> Result<CryptoStreamReport> {
    let config = runtime.binance_config();
    binance::stream_messages(
        &config,
        request.market,
        request.kind,
        &request.symbol,
        &request.interval,
        request.messages,
    )
    .await
}

#[derive(Debug, Clone)]
pub struct CryptoEvidenceSymbolRequest {
    pub symbol: String,
    pub provider: CryptoProvider,
    pub instrument: CryptoInstrument,
}

#[derive(Debug, Clone)]
pub struct CryptoEvidenceLimitRequest {
    pub symbol: String,
    pub provider: CryptoProvider,
    pub instrument: CryptoInstrument,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct CryptoEvidenceTradesRequest {
    pub symbol: String,
    pub provider: CryptoProvider,
    pub instrument: CryptoInstrument,
    pub limit: usize,
    pub aggregate: bool,
}

#[derive(Debug, Clone)]
pub struct CryptoEvidenceCandlesRequest {
    pub symbol: String,
    pub provider: CryptoProvider,
    pub instrument: CryptoInstrument,
    pub interval: String,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct CryptoEvidenceDiscoverRequest {
    pub provider: CryptoProvider,
    pub kind: crate::args::CryptoDiscoverKind,
    pub instrument: CryptoInstrument,
    pub limit: usize,
    pub vs_currency: String,
}

pub async fn crypto_evidence_quote(
    runtime: &MarketRuntime,
    request: CryptoEvidenceSymbolRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Quote;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        async move { sources.quote(provider, instrument, symbol).await }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_book(
    runtime: &MarketRuntime,
    request: CryptoEvidenceLimitRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Book;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let limit = request.limit;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        async move { sources.book(provider, instrument, symbol, limit).await }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_trades(
    runtime: &MarketRuntime,
    request: CryptoEvidenceTradesRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Trades;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let limit = request.limit;
    let aggregate = request.aggregate;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        async move {
            sources
                .trades(provider, instrument, symbol, limit, aggregate)
                .await
        }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_candles(
    runtime: &MarketRuntime,
    request: CryptoEvidenceCandlesRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Candles;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let interval = request.interval;
    let limit = request.limit;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        let interval = interval.clone();
        async move {
            sources
                .candles(provider, instrument, symbol, interval, limit)
                .await
        }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_funding(
    runtime: &MarketRuntime,
    request: CryptoEvidenceLimitRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Funding;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let limit = request.limit;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        async move { sources.funding(provider, instrument, symbol, limit).await }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_open_interest(
    runtime: &MarketRuntime,
    request: CryptoEvidenceSymbolRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::OpenInterest;
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let symbol = request.symbol;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let symbol = symbol.clone();
        async move { sources.open_interest(provider, instrument, symbol).await }
    })
    .await;
    Ok(evidence_report(
        capability,
        instrument,
        Some(&symbol),
        results,
    ))
}

pub async fn crypto_evidence_discover(
    runtime: &MarketRuntime,
    request: CryptoEvidenceDiscoverRequest,
) -> Result<CryptoEvidenceReport> {
    let sources =
        CryptoEvidenceSources::new(runtime.proxy(), runtime.no_proxy, runtime.timeout_seconds)?;
    let capability = CryptoCapability::Discover(request.kind);
    let instrument = resolve_instrument(request.instrument, capability);
    let evidence_request = EvidenceRequest::new(request.provider, instrument, capability);
    let kind = request.kind;
    let limit = request.limit;
    let vs_currency = request.vs_currency;
    let results = EvidenceEngine::collect(evidence_request, |provider| {
        let sources = sources.clone();
        let vs_currency = vs_currency.clone();
        async move {
            sources
                .discover(provider, instrument, kind, limit, vs_currency)
                .await
        }
    })
    .await;
    Ok(evidence_report(capability, instrument, None, results))
}

#[derive(Debug, Clone)]
pub struct PolymarketSearchRequest {
    pub query: String,
    pub limit: usize,
    pub include_closed: bool,
    pub min_volume: Option<f64>,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn polymarket_search(
    runtime: &MarketRuntime,
    request: PolymarketSearchRequest,
) -> Result<PredictionSearchReport> {
    let client = runtime.client()?;
    let options = crate::providers::polymarket::SearchRequestOptions {
        query: request.query,
        limit: request.limit,
        include_closed: request.include_closed,
        min_volume: request.min_volume,
        refresh: request.refresh,
        cache_ttl_seconds: request.cache_ttl_seconds,
        timeout_seconds: runtime.timeout_seconds,
        use_http_transport: runtime.proxy.is_some() || runtime.no_proxy,
    };
    crate::providers::polymarket::search_report(&client, &options, &runtime.timezone).await
}

#[derive(Debug, Clone)]
pub struct PolymarketMarketRequest {
    pub identifier: String,
    pub limit: usize,
    pub include_closed: bool,
    pub min_volume: Option<f64>,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

pub async fn polymarket_market(
    runtime: &MarketRuntime,
    request: PolymarketMarketRequest,
) -> Result<PredictionMarketReport> {
    let client = runtime.client()?;
    let options = crate::providers::polymarket::MarketRequestOptions {
        identifier: request.identifier,
        limit: request.limit,
        include_closed: request.include_closed,
        min_volume: request.min_volume,
        refresh: request.refresh,
        cache_ttl_seconds: request.cache_ttl_seconds,
        timeout_seconds: runtime.timeout_seconds,
        use_http_transport: runtime.proxy.is_some() || runtime.no_proxy,
    };
    crate::providers::polymarket::market_report(&client, &options, &runtime.timezone).await
}

fn effective_history_provider(provider: Provider, session: HistorySession) -> Provider {
    match (provider, session) {
        (Provider::Auto, HistorySession::Extended) => Provider::YahooExtended,
        (Provider::Yahoo, HistorySession::Extended) => Provider::YahooExtended,
        (provider, _) => provider,
    }
}

fn provider_instrument(provider: Provider, instrument: CryptoInstrument) -> CryptoInstrument {
    match provider {
        Provider::BinanceSpot => CryptoInstrument::Spot,
        Provider::BinanceUsdsFutures => CryptoInstrument::Swap,
        _ => resolve_instrument(instrument, CryptoCapability::Candles),
    }
}

fn provider_crypto_provider(provider: Provider, crypto_provider: CryptoProvider) -> CryptoProvider {
    match provider {
        Provider::BinanceSpot | Provider::BinanceUsdsFutures => CryptoProvider::Binance,
        _ => crypto_provider,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_debug_redacts_proxy_credentials() {
        let runtime = MarketRuntime::new(
            Some("http://user:password@127.0.0.1:7890"),
            false,
            10,
            "UTC",
        );

        let debug = format!("{runtime:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("user"));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("127.0.0.1"));
    }
}
