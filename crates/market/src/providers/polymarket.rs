use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::{SecondsFormat, TimeZone, Utc};
use polymarket_client_sdk_v2::clob::Client as ClobClient;
use polymarket_client_sdk_v2::clob::types::request::{
    OrderBookSummaryRequest, PriceHistoryRequest,
};
use polymarket_client_sdk_v2::clob::types::response::PricePoint as ClobPricePoint;
use polymarket_client_sdk_v2::clob::types::{Interval, TimeRange};
use polymarket_client_sdk_v2::data::Client as DataClient;
use polymarket_client_sdk_v2::data::types::request::{HoldersRequest, OpenInterestRequest};
use polymarket_client_sdk_v2::gamma::Client as GammaClient;
use polymarket_client_sdk_v2::gamma::types::request::{
    MarketByIdRequest, MarketBySlugRequest, SearchRequest,
};
use polymarket_client_sdk_v2::types::{B256, U256};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use wreq::Client;

use crate::cache;
use crate::model::{
    PredictionMarketReport, PredictionMarketSummary, PredictionOutcome, PredictionPricePoint,
    PredictionSearchReport,
};
use crate::time::{format_local, utc_to_local};

const PROVIDER: &str = "polymarket";
const INTERPRETATION_NOTE: &str = "Polymarket prices are prediction-market probabilities backed by user capital. Treat them as quantifiable sentiment/event-probability signals, not as confirmed facts, inside information, or equity quotes.";
const SEARCH_URL: &str = "https://gamma-api.polymarket.com/public-search";
const GAMMA_MARKET_URL: &str = "https://gamma-api.polymarket.com/markets";
const CLOB_URL: &str = "https://clob-v2.polymarket.com";
const DATA_URL: &str = "https://data-api.polymarket.com";

#[derive(Debug, Clone)]
pub struct SearchRequestOptions {
    pub query: String,
    pub limit: usize,
    pub include_closed: bool,
    pub min_volume: Option<f64>,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
    pub timeout_seconds: u64,
    pub use_http_transport: bool,
}

#[derive(Debug, Clone)]
pub struct MarketRequestOptions {
    pub identifier: String,
    pub limit: usize,
    pub include_closed: bool,
    pub min_volume: Option<f64>,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
    pub timeout_seconds: u64,
    pub use_http_transport: bool,
}

pub async fn search_report(
    client: &Client,
    options: &SearchRequestOptions,
    timezone: &str,
) -> Result<PredictionSearchReport> {
    let (fetched_at_utc, cache_status, payload) = cached_json(
        "polymarket-search",
        &format!(
            "{}:{}:{}",
            options.query, options.limit, options.include_closed
        ),
        options.cache_ttl_seconds,
        options.refresh,
        || fetch_search_payload(client, options),
    )
    .await?;
    let fetched_at_local = utc_to_local(Some(&fetched_at_utc), timezone)
        .unwrap_or_else(|| format_local(Utc::now(), timezone));
    let markets = collect_search_markets(
        &payload,
        options.limit,
        options.include_closed,
        options.min_volume,
        timezone,
    )?;

    Ok(PredictionSearchReport {
        provider: PROVIDER.to_string(),
        query: options.query.clone(),
        fetched_at_utc,
        fetched_at_local,
        cache_status,
        source_urls: vec![
            search_source_url(&options.query),
            "https://polymarket.com/search".to_string(),
        ],
        interpretation_note: INTERPRETATION_NOTE.to_string(),
        markets,
        payload,
    })
}

pub async fn market_report(
    client: &Client,
    options: &MarketRequestOptions,
    timezone: &str,
) -> Result<PredictionMarketReport> {
    let (fetched_at_utc, cache_status, market_payload) = cached_json(
        "polymarket-market",
        &options.identifier,
        options.cache_ttl_seconds,
        options.refresh,
        || fetch_market_payload(client, options),
    )
    .await?;
    let fetched_at_local = utc_to_local(Some(&fetched_at_utc), timezone)
        .unwrap_or_else(|| format_local(Utc::now(), timezone));
    let mut market = market_summary_from_value(&market_payload, None, timezone)?;
    reject_filtered_market(&market, options.include_closed, options.min_volume)?;

    let mut data_errors = BTreeMap::new();
    let enrichment_fetched_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let enrichment_fetched_at_local = utc_to_local(Some(&enrichment_fetched_at_utc), timezone)
        .unwrap_or_else(|| format_local(Utc::now(), timezone));
    hydrate_outcome_books(client, &mut market, options, &mut data_errors).await;
    let price_history =
        fetch_first_history(client, &market, options, timezone, &mut data_errors).await;
    let open_interest = fetch_open_interest(client, &market, options, &mut data_errors).await;
    let holder_preview_count =
        fetch_holder_preview_count(client, &market, options, &mut data_errors).await;
    market.open_interest = open_interest.or(market.open_interest);
    let enrichment_status = if data_errors.is_empty() {
        "live".to_string()
    } else {
        "live_partial".to_string()
    };

    let outcomes = market.outcomes.clone();
    Ok(PredictionMarketReport {
        provider: PROVIDER.to_string(),
        identifier: options.identifier.clone(),
        fetched_at_utc,
        fetched_at_local,
        cache_status: format!("gamma_{cache_status}"),
        enrichment_status,
        enrichment_fetched_at_utc,
        enrichment_fetched_at_local,
        source_urls: market_source_urls(&market),
        interpretation_note: INTERPRETATION_NOTE.to_string(),
        market,
        outcomes,
        price_history,
        open_interest,
        holder_preview_count,
        data_errors,
        payload: market_payload,
    })
}

async fn cached_json<F, Fut>(
    namespace: &str,
    key: &str,
    ttl_seconds: u64,
    refresh: bool,
    fetch: F,
) -> Result<(String, String, Value)>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Value>>,
{
    if !refresh
        && let Some((fetched_at_utc, payload)) = cache::read_json(namespace, key, ttl_seconds)
    {
        return Ok((fetched_at_utc, "hit".to_string(), payload));
    }

    let payload = fetch().await?;
    let fetched_at_utc = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    cache::write_json(namespace, key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "miss".to_string(), payload))
}

async fn fetch_search_payload(client: &Client, options: &SearchRequestOptions) -> Result<Value> {
    if options.use_http_transport {
        let params = vec![
            ("q", options.query.clone()),
            ("limit_per_type", options.limit.to_string()),
            ("search_profiles", "false".to_string()),
            (
                "keep_closed_markets",
                if options.include_closed { "1" } else { "0" }.to_string(),
            ),
        ];
        return client
            .get(url_with_query(SEARCH_URL, &params))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await
            .context("failed to decode Polymarket search response");
    }
    let request = SearchRequest::builder()
        .q(options.query.clone())
        .limit_per_type(i32::try_from(options.limit.min(i32::MAX as usize))?)
        .search_profiles(false)
        .keep_closed_markets(i32::from(options.include_closed))
        .build();
    let response = with_timeout(
        options.timeout_seconds,
        GammaClient::default().search(&request),
        "polymarket gamma search",
    )
    .await?;
    Ok(serde_json::to_value(response)?)
}

async fn fetch_market_payload(client: &Client, options: &MarketRequestOptions) -> Result<Value> {
    let identifier = options.identifier.as_str();
    if options.use_http_transport {
        let url = if identifier.chars().all(|ch| ch.is_ascii_digit()) {
            format!("{GAMMA_MARKET_URL}/{identifier}")
        } else {
            format!("{GAMMA_MARKET_URL}/slug/{identifier}")
        };
        return client
            .get(url_with_query(&url, &[("include_tag", "true".to_string())]))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await
            .context("failed to decode Polymarket market response");
    }
    let client = GammaClient::default();
    let response = if identifier.chars().all(|ch| ch.is_ascii_digit()) {
        let request = MarketByIdRequest::builder()
            .id(identifier.to_string())
            .include_tag(true)
            .build();
        with_timeout(
            options.timeout_seconds,
            client.market_by_id(&request),
            "polymarket gamma market by id",
        )
        .await
    } else {
        let request = MarketBySlugRequest::builder()
            .slug(identifier.to_string())
            .include_tag(true)
            .build();
        with_timeout(
            options.timeout_seconds,
            client.market_by_slug(&request),
            "polymarket gamma market by slug",
        )
        .await
    }?;
    Ok(serde_json::to_value(response)?)
}

async fn with_timeout<T>(
    timeout_seconds: u64,
    future: impl std::future::Future<Output = Result<T, polymarket_client_sdk_v2::error::Error>>,
    label: &str,
) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(timeout_seconds), future)
        .await
        .with_context(|| format!("{label} timed out after {timeout_seconds}s"))?
        .map_err(Into::into)
}

fn collect_search_markets(
    payload: &Value,
    limit: usize,
    include_closed: bool,
    min_volume: Option<f64>,
    timezone: &str,
) -> Result<Vec<PredictionMarketSummary>> {
    let mut markets = Vec::new();
    let mut seen = BTreeSet::new();
    for event in payload
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let context = EventContext {
            id: string_value(event, "id"),
            slug: string_value(event, "slug"),
            title: string_value(event, "title"),
        };
        for market in event
            .get("markets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let summary = market_summary_from_value(market, Some(&context), timezone)?;
            let key = summary
                .id
                .clone()
                .or_else(|| summary.slug.clone())
                .unwrap_or_else(|| summary.title.clone());
            if seen.insert(key) && market_passes(&summary, include_closed, min_volume) {
                markets.push(summary);
            }
        }
    }

    markets.sort_by(compare_markets);
    markets.truncate(limit);
    Ok(markets)
}

fn reject_filtered_market(
    market: &PredictionMarketSummary,
    include_closed: bool,
    min_volume: Option<f64>,
) -> Result<()> {
    if market_passes(market, include_closed, min_volume) {
        Ok(())
    } else {
        Err(anyhow!(
            "market was filtered out by include_closed={} min_volume={:?}",
            include_closed,
            min_volume
        ))
    }
}

fn market_passes(
    market: &PredictionMarketSummary,
    include_closed: bool,
    min_volume: Option<f64>,
) -> bool {
    if !include_closed && market.closed == Some(true) {
        return false;
    }
    let volume = market.volume.or(market.volume_24hr).unwrap_or(0.0);
    min_volume.is_none_or(|threshold| volume >= threshold)
}

fn compare_markets(left: &PredictionMarketSummary, right: &PredictionMarketSummary) -> Ordering {
    active_rank(right).cmp(&active_rank(left)).then_with(|| {
        score(right)
            .partial_cmp(&score(left))
            .unwrap_or(Ordering::Equal)
    })
}

fn active_rank(market: &PredictionMarketSummary) -> u8 {
    match (market.active, market.closed) {
        (Some(true), Some(false)) => 2,
        (Some(true), _) => 1,
        _ => 0,
    }
}

fn score(market: &PredictionMarketSummary) -> f64 {
    market.volume_24hr.unwrap_or(0.0) * 3.0
        + market.liquidity.unwrap_or(0.0) * 2.0
        + market.volume.unwrap_or(0.0)
}

fn market_summary_from_value(
    market: &Value,
    event: Option<&EventContext>,
    timezone: &str,
) -> Result<PredictionMarketSummary> {
    let outcomes = aligned_outcomes(market)?;
    let end_time_utc =
        string_value(market, "endDate").or_else(|| string_value(market, "endDateIso"));
    let end_time_local = end_time_utc
        .as_deref()
        .and_then(|value| utc_to_local(Some(value), timezone));
    let event_id = event.and_then(|value| value.id.clone());
    let event_slug = event.and_then(|value| value.slug.clone());
    let slug = string_value(market, "slug");
    let title = string_value(market, "question")
        .or_else(|| event.and_then(|value| value.title.clone()))
        .or_else(|| slug.clone())
        .unwrap_or_else(|| "untitled market".to_string());

    Ok(PredictionMarketSummary {
        id: string_value(market, "id"),
        condition_id: string_value(market, "conditionId"),
        slug: slug.clone(),
        event_id,
        event_slug: event_slug.clone(),
        title,
        question: string_value(market, "question"),
        active: bool_value(market, "active"),
        closed: bool_value(market, "closed"),
        accepting_orders: bool_value(market, "acceptingOrders"),
        end_time_utc,
        end_time_local,
        volume: number_value(market, "volume").or_else(|| number_value(market, "volumeNum")),
        volume_24hr: number_value(market, "volume24hr")
            .or_else(|| number_value(market, "volume24hrClob")),
        liquidity: number_value(market, "liquidity")
            .or_else(|| number_value(market, "liquidityNum")),
        open_interest: number_value(market, "openInterest"),
        best_bid: number_value(market, "bestBid"),
        best_ask: number_value(market, "bestAsk"),
        spread: number_value(market, "spread"),
        last_trade_price: number_value(market, "lastTradePrice"),
        one_hour_price_change: number_value(market, "oneHourPriceChange"),
        one_day_price_change: number_value(market, "oneDayPriceChange"),
        one_week_price_change: number_value(market, "oneWeekPriceChange"),
        market_url: market_url(event_slug.as_deref(), slug.as_deref()),
        outcomes,
    })
}

fn aligned_outcomes(market: &Value) -> Result<Vec<PredictionOutcome>> {
    let labels = string_array(market.get("outcomes"))
        .context("market outcomes must be a JSON array or JSON-encoded array")?;
    let prices = optional_number_array(market.get("outcomePrices"))?;
    let token_ids = optional_string_array(market.get("clobTokenIds"))?;
    if let Some(prices) = &prices
        && prices.len() != labels.len()
    {
        return Err(anyhow!(
            "outcomePrices length {} does not match outcomes length {}",
            prices.len(),
            labels.len()
        ));
    }
    if let Some(token_ids) = &token_ids
        && token_ids.len() != labels.len()
    {
        return Err(anyhow!(
            "clobTokenIds length {} does not match outcomes length {}",
            token_ids.len(),
            labels.len()
        ));
    }

    Ok(labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| PredictionOutcome {
            label,
            implied_probability: prices
                .as_ref()
                .and_then(|values| values.get(index).copied()),
            clob_token_id: token_ids
                .as_ref()
                .and_then(|values| values.get(index).cloned()),
            best_bid: None,
            best_ask: None,
            spread: None,
            last_trade_price: None,
            bid_count: 0,
            ask_count: 0,
        })
        .collect())
}

async fn hydrate_outcome_books(
    client: &Client,
    market: &mut PredictionMarketSummary,
    options: &MarketRequestOptions,
    errors: &mut BTreeMap<String, String>,
) {
    let sdk_client = ClobClient::default();
    for outcome in &mut market.outcomes {
        let Some(token_id) = outcome.clob_token_id.as_deref() else {
            continue;
        };
        let book = if options.use_http_transport {
            fetch_order_book_http(client, token_id).await
        } else {
            fetch_order_book_sdk(&sdk_client, token_id, options.timeout_seconds).await
        };
        match book {
            Ok(value) => apply_book(outcome, &value, options.limit),
            Err(error) => {
                errors.insert(format!("orderbook:{token_id}"), error.to_string());
            }
        }
    }
    if let Some(first) = market.outcomes.first() {
        market.best_bid = market.best_bid.or(first.best_bid);
        market.best_ask = market.best_ask.or(first.best_ask);
        market.spread = market.spread.or(first.spread);
    }
}

async fn fetch_order_book_sdk(
    client: &ClobClient,
    token_id: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let token = token_id.parse::<U256>()?;
    let request = OrderBookSummaryRequest::builder().token_id(token).build();
    let book = with_timeout(
        timeout_seconds,
        client.order_book(&request),
        "polymarket clob orderbook",
    )
    .await?;
    Ok(serde_json::to_value(book)?)
}

async fn fetch_order_book_http(client: &Client, token_id: &str) -> Result<Value> {
    client
        .get(url_with_query(
            &format!("{CLOB_URL}/book"),
            &[("token_id", token_id.to_string())],
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
        .context("failed to decode Polymarket orderbook response")
}

fn apply_book(outcome: &mut PredictionOutcome, book: &Value, limit: usize) {
    let bids = book.get("bids").and_then(Value::as_array);
    let asks = book.get("asks").and_then(Value::as_array);
    outcome.best_bid = best_price(bids, BookSide::Bid);
    outcome.best_ask = best_price(asks, BookSide::Ask);
    outcome.spread = match (outcome.best_bid, outcome.best_ask) {
        (Some(bid), Some(ask)) => Some((ask - bid).max(0.0)),
        _ => None,
    };
    outcome.last_trade_price = number_value(book, "lastTradePrice");
    outcome.bid_count = bids.map_or(0, |rows| rows.len().min(limit));
    outcome.ask_count = asks.map_or(0, |rows| rows.len().min(limit));
}

async fn fetch_first_history(
    client: &Client,
    market: &PredictionMarketSummary,
    options: &MarketRequestOptions,
    timezone: &str,
    errors: &mut BTreeMap<String, String>,
) -> Vec<PredictionPricePoint> {
    let Some(token_id) = market
        .outcomes
        .iter()
        .find_map(|outcome| outcome.clob_token_id.as_deref())
    else {
        return Vec::new();
    };
    let history = if options.use_http_transport {
        fetch_history_http(client, token_id, timezone).await
    } else {
        fetch_history_sdk(token_id, options.timeout_seconds, timezone).await
    };
    match history {
        Ok(points) => points,
        Err(error) => {
            errors.insert(format!("history:{token_id}"), error.to_string());
            Vec::new()
        }
    }
}

async fn fetch_history_sdk(
    token_id: &str,
    timeout_seconds: u64,
    timezone: &str,
) -> Result<Vec<PredictionPricePoint>> {
    let token = token_id.parse::<U256>()?;
    let request = PriceHistoryRequest::builder()
        .market(token)
        .time_range(TimeRange::from_interval(Interval::OneWeek))
        .fidelity(60)
        .build();
    let response = with_timeout(
        timeout_seconds,
        ClobClient::default().price_history(&request),
        "polymarket clob price history",
    )
    .await?;
    Ok(price_points_from_clob(response.history, timezone))
}

async fn fetch_history_http(
    client: &Client,
    token_id: &str,
    timezone: &str,
) -> Result<Vec<PredictionPricePoint>> {
    let value = client
        .get(url_with_query(
            &format!("{CLOB_URL}/prices-history"),
            &[
                ("market", token_id.to_string()),
                ("interval", "1w".to_string()),
                ("fidelity", "60".to_string()),
            ],
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
        .context("failed to decode Polymarket price history response")?;
    Ok(price_points_from_value(&value, timezone))
}

fn price_points_from_clob(
    points: Vec<ClobPricePoint>,
    timezone: &str,
) -> Vec<PredictionPricePoint> {
    points
        .into_iter()
        .filter_map(|point| {
            let price = point.p.to_string().parse::<f64>().ok()?;
            let time_utc = Utc
                .timestamp_opt(point.t, 0)
                .single()
                .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, true));
            let time_local = time_utc
                .as_deref()
                .and_then(|value| utc_to_local(Some(value), timezone));
            Some(PredictionPricePoint {
                time_utc,
                time_local,
                price,
            })
        })
        .collect()
}

fn price_points_from_value(value: &Value, timezone: &str) -> Vec<PredictionPricePoint> {
    value
        .get("history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|point| {
            let price = number_value(point, "p")?;
            let time_utc = point
                .get("t")
                .and_then(Value::as_i64)
                .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single())
                .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, true));
            let time_local = time_utc
                .as_deref()
                .and_then(|value| utc_to_local(Some(value), timezone));
            Some(PredictionPricePoint {
                time_utc,
                time_local,
                price,
            })
        })
        .collect()
}

async fn fetch_open_interest(
    client: &Client,
    market: &PredictionMarketSummary,
    options: &MarketRequestOptions,
    errors: &mut BTreeMap<String, String>,
) -> Option<f64> {
    let condition_id = condition_id(market)?;
    let value = if options.use_http_transport {
        fetch_open_interest_http(client, &condition_id.to_string()).await
    } else {
        fetch_open_interest_sdk(condition_id, options.timeout_seconds).await
    };
    match value {
        Ok(value) => value,
        Err(error) => {
            errors.insert("open_interest".to_string(), error.to_string());
            None
        }
    }
}

async fn fetch_open_interest_sdk(condition_id: B256, timeout_seconds: u64) -> Result<Option<f64>> {
    let request = OpenInterestRequest::builder()
        .markets(vec![condition_id])
        .build();
    let response = with_timeout(
        timeout_seconds,
        DataClient::default().open_interest(&request),
        "polymarket data open interest",
    )
    .await?;
    Ok(response
        .first()
        .and_then(|row| row.value.to_string().parse::<f64>().ok()))
}

async fn fetch_open_interest_http(client: &Client, condition_id: &str) -> Result<Option<f64>> {
    let value = client
        .get(url_with_query(
            &format!("{DATA_URL}/oi"),
            &[("market", condition_id.to_string())],
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
        .context("failed to decode Polymarket open-interest response")?;
    Ok(value
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| number_value(row, "value")))
}

async fn fetch_holder_preview_count(
    client: &Client,
    market: &PredictionMarketSummary,
    options: &MarketRequestOptions,
    errors: &mut BTreeMap<String, String>,
) -> Option<usize> {
    let condition_id = condition_id(market)?;
    let value = if options.use_http_transport {
        fetch_holder_preview_count_http(client, &condition_id.to_string(), options.limit).await
    } else {
        fetch_holder_preview_count_sdk(condition_id, options.limit, options.timeout_seconds).await
    };
    match value {
        Ok(value) => value,
        Err(error) => {
            errors.insert("holders".to_string(), error.to_string());
            None
        }
    }
}

async fn fetch_holder_preview_count_sdk(
    condition_id: B256,
    limit: usize,
    timeout_seconds: u64,
) -> Result<Option<usize>> {
    let builder = HoldersRequest::builder()
        .markets(vec![condition_id])
        .limit(i32::try_from(limit.min(20))?)?;
    let request = builder.build();
    let response = with_timeout(
        timeout_seconds,
        DataClient::default().holders(&request),
        "polymarket data holders",
    )
    .await?;
    Ok(Some(response.iter().map(|token| token.holders.len()).sum()))
}

async fn fetch_holder_preview_count_http(
    client: &Client,
    condition_id: &str,
    limit: usize,
) -> Result<Option<usize>> {
    let limit = limit.min(20).to_string();
    let value = client
        .get(url_with_query(
            &format!("{DATA_URL}/holders"),
            &[
                ("market", condition_id.to_string()),
                ("limit", limit.to_string()),
            ],
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
        .context("failed to decode Polymarket holders response")?;
    Ok(Some(
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|token| token.get("holders").and_then(Value::as_array))
            .map(Vec::len)
            .sum(),
    ))
}

fn condition_id(market: &PredictionMarketSummary) -> Option<B256> {
    market
        .condition_id
        .as_deref()
        .and_then(|value| B256::from_str(value).ok())
}

fn market_source_urls(market: &PredictionMarketSummary) -> Vec<String> {
    let mut urls = vec![
        GAMMA_MARKET_URL.to_string(),
        CLOB_URL.to_string(),
        DATA_URL.to_string(),
    ];
    if let Some(url) = &market.market_url {
        urls.push(url.clone());
    }
    urls
}

fn search_source_url(query: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", query)
        .finish();
    format!("{SEARCH_URL}?{query}")
}

fn url_with_query(base: &str, params: &[(&str, String)]) -> String {
    let query = params
        .iter()
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut builder, (key, value)| {
                builder.append_pair(key, value);
                builder
            },
        )
        .finish();
    format!("{base}?{query}")
}

fn market_url(event_slug: Option<&str>, market_slug: Option<&str>) -> Option<String> {
    event_slug
        .map(|slug| format!("https://polymarket.com/event/{slug}"))
        .or_else(|| market_slug.map(|slug| format!("https://polymarket.com/market/{slug}")))
}

#[derive(Debug)]
struct EventContext {
    id: Option<String>,
    slug: Option<String>,
    title: Option<String>,
}

#[derive(Clone, Copy)]
enum BookSide {
    Bid,
    Ask,
}

fn best_price(rows: Option<&Vec<Value>>, side: BookSide) -> Option<f64> {
    rows?
        .iter()
        .filter_map(|row| number_value(row, "price"))
        .fold(None, |best, price| match (best, side) {
            (None, _) => Some(price),
            (Some(best), BookSide::Bid) => Some(best.max(price)),
            (Some(best), BookSide::Ask) => Some(best.min(price)),
        })
}

fn string_array(value: Option<&Value>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    match value {
        Value::Array(rows) => Ok(rows
            .iter()
            .filter_map(|value| value.as_str().map(ToString::to_string))
            .collect()),
        Value::String(raw) => {
            let parsed = serde_json::from_str::<Value>(raw)?;
            string_array(Some(&parsed))
        }
        _ => Err(anyhow!("expected string array")),
    }
}

fn optional_string_array(value: Option<&Value>) -> Result<Option<Vec<String>>> {
    match value {
        Some(Value::Null) | None => Ok(None),
        Some(value) => string_array(Some(value)).map(Some),
    }
}

fn optional_number_array(value: Option<&Value>) -> Result<Option<Vec<f64>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array = match value {
        Value::Array(rows) => rows.iter().filter_map(number_scalar).collect(),
        Value::String(raw) => {
            let parsed = serde_json::from_str::<Value>(raw)?;
            return optional_number_array(Some(&parsed));
        }
        Value::Null => return Ok(None),
        _ => return Err(anyhow!("expected number array")),
    };
    Ok(Some(array))
}

fn string_value(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| match value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn bool_value(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn number_value(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(number_scalar)
}

fn number_scalar(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_gamma_outcomes_with_prices_and_tokens() {
        let market = json!({
            "outcomes": "[\"Yes\",\"No\"]",
            "outcomePrices": "[\"0.61\",\"0.39\"]",
            "clobTokenIds": "[\"111\",\"222\"]"
        });

        let outcomes = aligned_outcomes(&market).unwrap();

        assert_eq!(outcomes[0].label, "Yes");
        assert_eq!(outcomes[0].implied_probability, Some(0.61));
        assert_eq!(outcomes[0].clob_token_id.as_deref(), Some("111"));
        assert_eq!(outcomes[1].label, "No");
        assert_eq!(outcomes[1].implied_probability, Some(0.39));
        assert_eq!(outcomes[1].clob_token_id.as_deref(), Some("222"));
    }

    #[test]
    fn rejects_misaligned_outcome_arrays() {
        let market = json!({
            "outcomes": "[\"Yes\",\"No\"]",
            "outcomePrices": "[\"0.61\"]",
            "clobTokenIds": "[\"111\",\"222\"]"
        });

        let error = aligned_outcomes(&market).unwrap_err().to_string();

        assert!(error.contains("outcomePrices length 1 does not match outcomes length 2"));
    }

    #[test]
    fn treats_null_outcome_prices_as_missing_probabilities() {
        let market = json!({
            "outcomes": "[\"Yes\",\"No\"]",
            "outcomePrices": null,
            "clobTokenIds": "[\"111\",\"222\"]"
        });

        let outcomes = aligned_outcomes(&market).unwrap();

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].implied_probability, None);
        assert_eq!(outcomes[1].implied_probability, None);
    }

    #[test]
    fn computes_best_prices_from_unsorted_orderbook() {
        let mut outcome = PredictionOutcome {
            label: "Yes".to_string(),
            implied_probability: Some(0.6),
            clob_token_id: Some("111".to_string()),
            best_bid: None,
            best_ask: None,
            spread: None,
            last_trade_price: None,
            bid_count: 0,
            ask_count: 0,
        };
        let book = json!({
            "bids": [{"price":"0.58","size":"100"}, {"price":"0.61","size":"10"}],
            "asks": [{"price":"0.66","size":"100"}, {"price":"0.64","size":"10"}],
            "lastTradePrice": "0.62"
        });

        apply_book(&mut outcome, &book, 10);

        assert_eq!(outcome.best_bid, Some(0.61));
        assert_eq!(outcome.best_ask, Some(0.64));
        assert_eq!(outcome.spread, Some(0.030000000000000027));
        assert_eq!(outcome.last_trade_price, Some(0.62));
        assert_eq!(outcome.bid_count, 2);
        assert_eq!(outcome.ask_count, 2);
    }

    #[test]
    fn filters_closed_and_low_volume_search_results_then_sorts_by_signal_strength() {
        let payload = json!({
            "events": [
                {
                    "id": "event-1",
                    "slug": "space-event",
                    "title": "Space Event",
                    "markets": [
                        {
                            "id": "1",
                            "question": "Low volume active",
                            "slug": "low",
                            "active": true,
                            "closed": false,
                            "volume": "10",
                            "liquidity": "10",
                            "outcomes": "[\"Yes\",\"No\"]",
                            "outcomePrices": "[\"0.5\",\"0.5\"]",
                            "clobTokenIds": "[\"1\",\"2\"]"
                        },
                        {
                            "id": "2",
                            "question": "High volume active",
                            "slug": "high",
                            "active": true,
                            "closed": false,
                            "volume": "100",
                            "volume24hr": "40",
                            "liquidity": "50",
                            "outcomes": "[\"Yes\",\"No\"]",
                            "outcomePrices": "[\"0.7\",\"0.3\"]",
                            "clobTokenIds": "[\"3\",\"4\"]"
                        },
                        {
                            "id": "3",
                            "question": "Closed market",
                            "slug": "closed",
                            "active": false,
                            "closed": true,
                            "volume": "1000",
                            "outcomes": "[\"Yes\",\"No\"]",
                            "outcomePrices": "[\"0.9\",\"0.1\"]",
                            "clobTokenIds": "[\"5\",\"6\"]"
                        }
                    ]
                }
            ]
        });

        let markets = collect_search_markets(&payload, 5, false, Some(20.0), "UTC").unwrap();

        assert_eq!(markets.len(), 1);
        assert_eq!(markets[0].slug.as_deref(), Some("high"));
        assert_eq!(markets[0].event_slug.as_deref(), Some("space-event"));
    }

    #[test]
    fn parses_price_history_points_with_local_time() {
        let payload = json!({
            "history": [
                {"t": 1760000000, "p": "0.52"},
                {"t": 1760000060, "p": 0.53}
            ]
        });

        let points = price_points_from_value(&payload, "Asia/Singapore");

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].price, 0.52);
        assert_eq!(points[0].time_utc.as_deref(), Some("2025-10-09T08:53:20Z"));
        assert_eq!(
            points[0].time_local.as_deref(),
            Some("2025-10-09T16:53:20+08:00")
        );
    }
}
