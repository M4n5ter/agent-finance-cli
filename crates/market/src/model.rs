use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

pub const SESSION_REGULAR: &str = "regular";
pub const SESSION_PRE: &str = "pre";
pub const SESSION_POST: &str = "post";
pub const SESSION_EXTENDED: &str = "extended";
pub const SESSION_OVERNIGHT: &str = "overnight";
pub const SESSION_24H_PROXY: &str = "24h_proxy";

#[derive(Debug, Clone, Serialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub currency: Option<String>,
    pub provider: String,
    pub session: Option<String>,
    pub fetched_at_utc: String,
    pub market_time: Option<String>,
    pub previous_close: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<u64>,
    pub exchange: Option<String>,
    pub provider_symbol: Option<String>,
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OhlcBar {
    pub symbol: String,
    pub provider: String,
    pub open_time: String,
    pub close_time: Option<String>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: f64,
    pub adj_close: Option<f64>,
    pub volume: Option<f64>,
    pub quote_volume: Option<f64>,
    pub trades: Option<u64>,
    pub dividend: Option<f64>,
    pub stock_split: Option<f64>,
    pub capital_gain: Option<f64>,
    pub repaired: bool,
}

#[derive(Debug, Serialize)]
pub struct HistoryBatch {
    pub symbol: String,
    pub provider: String,
    pub interval: String,
    pub adjustment: String,
    pub actions_included: bool,
    pub repair_requested: bool,
    pub repair_applied: bool,
    pub bars: Vec<OhlcBar>,
}

#[derive(Debug, Serialize)]
pub struct DerivedIndicator {
    pub symbol: String,
    pub provider: String,
    pub latest_close: Option<f64>,
    pub latest_time: Option<String>,
    pub return_1_bar_pct: Option<f64>,
    pub return_5_bar_pct: Option<f64>,
    pub return_20_bar_pct: Option<f64>,
    pub sma_20: Option<f64>,
    pub sma_50: Option<f64>,
    pub high_20: Option<f64>,
    pub low_20: Option<f64>,
    pub realized_vol_20_annualized_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricePoint {
    pub label: String,
    pub symbol: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub provider: String,
    pub session: Option<String>,
    pub market_time_utc: Option<String>,
    pub market_time_local: Option<String>,
    pub change_pct: Option<f64>,
    pub previous_close: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<u64>,
    pub exchange: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegularBasis {
    pub previous_close: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct PriceSummary {
    pub symbol: String,
    pub timezone: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub current: Option<PricePoint>,
    pub regular_basis: RegularBasis,
    pub sessions: Vec<PricePoint>,
    pub proxy: Option<PricePoint>,
    pub errors: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct ResearchReport {
    pub symbol: String,
    pub category: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub sources: Vec<ResearchSource>,
    pub modules: Vec<ResearchModule>,
    pub coverage_gaps: Vec<ResearchCoverageGap>,
    pub highlights: Vec<ResearchHighlight>,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct ResearchSource {
    pub provider: String,
    pub cache_status: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResearchModule {
    pub name: String,
    pub provider: String,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResearchCoverageGap {
    pub module: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ResearchHighlight {
    pub label: String,
    pub value: String,
    pub provider: String,
    pub module: String,
}

impl ResearchHighlight {
    pub fn new(label: &str, value: impl Into<String>, provider: &str, module: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.into(),
            provider: provider.to_string(),
            module: module.to_string(),
        }
    }

    pub fn from_path(
        root: Option<&Value>,
        label: &str,
        path: &str,
        provider: &str,
        module: &str,
    ) -> Option<Self> {
        let value = research_value_string(root?.pointer(path))?;
        Some(Self::new(label, value, provider, module))
    }
}

pub fn research_value_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(|value| research_value_string(Some(value)))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Value::Object(map) => map
            .get("fmt")
            .and_then(|value| value.as_str().map(ToString::to_string))
            .or_else(|| {
                map.get("raw")
                    .and_then(|value| research_value_string(Some(value)))
            }),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderProfile {
    pub provider: String,
    pub requires_api_key: bool,
    pub official_status: String,
    pub stability: String,
    pub best_for: String,
    pub large_download: bool,
    pub capabilities: Vec<ProviderCapability>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderCapability {
    pub module: String,
    pub status: String,
    pub note: String,
    pub implemented: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StooqCatalog {
    pub fetched_at_utc: String,
    pub source_url: String,
    pub entries: Vec<StooqCatalogEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StooqCatalogEntry {
    pub frequency: String,
    pub market: String,
    pub asset: String,
    pub label: String,
    pub approx_size_mb: Option<f64>,
    pub listing_url: String,
    pub direct_download_requires_captcha: bool,
    pub cache_key: String,
    pub cached_zip_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StooqSyncReport {
    pub provider: String,
    pub frequency: String,
    pub market: String,
    pub asset: String,
    pub cache_key: String,
    pub zip_path: String,
    pub bytes: u64,
    pub imported_at_utc: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct SearchReport {
    pub category: String,
    pub query: String,
    pub provider: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub cache_status: String,
    pub highlights: Vec<ResearchHighlight>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictionSearchReport {
    pub provider: String,
    pub query: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub cache_status: String,
    pub source_urls: Vec<String>,
    pub interpretation_note: String,
    pub markets: Vec<PredictionMarketSummary>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictionMarketReport {
    pub provider: String,
    pub identifier: String,
    pub fetched_at_utc: String,
    pub fetched_at_local: String,
    pub cache_status: String,
    pub enrichment_status: String,
    pub enrichment_fetched_at_utc: String,
    pub enrichment_fetched_at_local: String,
    pub source_urls: Vec<String>,
    pub interpretation_note: String,
    pub market: PredictionMarketSummary,
    pub outcomes: Vec<PredictionOutcome>,
    pub price_history: Vec<PredictionPricePoint>,
    pub open_interest: Option<f64>,
    pub holder_preview_count: Option<usize>,
    pub data_errors: BTreeMap<String, String>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictionMarketSummary {
    pub id: Option<String>,
    pub condition_id: Option<String>,
    pub slug: Option<String>,
    pub event_id: Option<String>,
    pub event_slug: Option<String>,
    pub title: String,
    pub question: Option<String>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub accepting_orders: Option<bool>,
    pub end_time_utc: Option<String>,
    pub end_time_local: Option<String>,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub liquidity: Option<f64>,
    pub open_interest: Option<f64>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub one_hour_price_change: Option<f64>,
    pub one_day_price_change: Option<f64>,
    pub one_week_price_change: Option<f64>,
    pub market_url: Option<String>,
    pub outcomes: Vec<PredictionOutcome>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictionOutcome {
    pub label: String,
    pub implied_probability: Option<f64>,
    pub clob_token_id: Option<String>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub bid_count: usize,
    pub ask_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PredictionPricePoint {
    pub time_utc: Option<String>,
    pub time_local: Option<String>,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamQuote {
    pub symbol: String,
    pub price: f64,
    pub time_utc: Option<String>,
    pub time_local: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub quote_type: Option<i32>,
    pub market_hours: Option<i32>,
    pub change_pct: Option<f64>,
    pub day_volume: Option<i64>,
    pub day_high: Option<f64>,
    pub day_low: Option<f64>,
    pub change: Option<f64>,
    pub short_name: Option<String>,
    pub open: Option<f64>,
    pub previous_close: Option<f64>,
    pub provider: String,
}
