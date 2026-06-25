use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;
use wreq::{Client, header::ACCEPT};

use crate::history::apply_history_adjustment_and_repair;
use crate::http::{change_pct, timestamp_sec_to_utc, utc_now};
use crate::model::{
    HistoryBatch, OhlcBar, PricePoint, Quote, SESSION_EXTENDED, SESSION_OVERNIGHT, SESSION_POST,
    SESSION_PRE, SESSION_REGULAR,
};
use crate::providers::HistoryRequest;
use crate::time::utc_to_local;

const YAHOO_COOKIE_URL: &str = "https://fc.yahoo.com/consent";
const YAHOO_CRUMB_URL: &str = "https://query1.finance.yahoo.com/v1/test/getcrumb";
const YAHOO_QUOTE_V7_URL: &str = "https://query1.finance.yahoo.com/v7/finance/quote";
const YAHOO_QUOTE_SUMMARY_BASE_URL: &str =
    "https://query2.finance.yahoo.com/v10/finance/quoteSummary";
const YAHOO_OPTIONS_BASE_URL: &str = "https://query2.finance.yahoo.com/v7/finance/options";
const YAHOO_SEARCH_URL: &str = "https://query1.finance.yahoo.com/v1/finance/search";
const YAHOO_SCREENER_URL: &str =
    "https://query1.finance.yahoo.com/v1/finance/screener/predefined/saved";

#[derive(Debug, Deserialize)]
struct YahooResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct YahooResult {
    meta: YahooMeta,
    timestamp: Option<Vec<i64>>,
    indicators: Option<YahooIndicators>,
    events: Option<YahooEvents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YahooMeta {
    currency: Option<String>,
    exchange_name: Option<String>,
    regular_market_price: Option<f64>,
    regular_market_time: Option<i64>,
    previous_close: Option<f64>,
    chart_previous_close: Option<f64>,
    regular_market_previous_close: Option<f64>,
    current_trading_period: Option<YahooCurrentTradingPeriod>,
}

#[derive(Debug, Deserialize)]
struct YahooCurrentTradingPeriod {
    pre: Option<YahooTradingPeriod>,
    regular: Option<YahooTradingPeriod>,
    post: Option<YahooTradingPeriod>,
}

#[derive(Debug, Deserialize)]
struct YahooTradingPeriod {
    start: Option<i64>,
    end: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Option<Vec<YahooQuoteBlock>>,
    adjclose: Option<Vec<YahooAdjCloseBlock>>,
}

#[derive(Debug, Deserialize)]
struct YahooAdjCloseBlock {
    adjclose: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct YahooQuoteBlock {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<u64>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YahooEvents {
    dividends: Option<std::collections::BTreeMap<String, YahooDividendEvent>>,
    splits: Option<std::collections::BTreeMap<String, YahooSplitEvent>>,
    capital_gains: Option<std::collections::BTreeMap<String, YahooCapitalGainEvent>>,
}

#[derive(Debug, Deserialize)]
struct YahooDividendEvent {
    amount: Option<f64>,
    date: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YahooSplitEvent {
    date: Option<i64>,
    numerator: Option<f64>,
    denominator: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct YahooCapitalGainEvent {
    amount: Option<f64>,
    date: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct YahooV7Envelope {
    #[serde(rename = "quoteResponse")]
    quote_response: YahooV7QuoteResponse,
}

#[derive(Debug, Deserialize)]
struct YahooV7QuoteResponse {
    result: Option<Vec<YahooV7QuoteNode>>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YahooV7QuoteNode {
    symbol: Option<String>,
    currency: Option<String>,
    full_exchange_name: Option<String>,
    exchange: Option<String>,
    regular_market_price: Option<f64>,
    regular_market_time: Option<i64>,
    regular_market_previous_close: Option<f64>,
    regular_market_open: Option<f64>,
    regular_market_day_high: Option<f64>,
    regular_market_day_low: Option<f64>,
    regular_market_volume: Option<u64>,
    pre_market_price: Option<f64>,
    pre_market_time: Option<i64>,
    pre_market_change_percent: Option<f64>,
    post_market_price: Option<f64>,
    post_market_time: Option<i64>,
    post_market_change_percent: Option<f64>,
    overnight_market_price: Option<f64>,
    overnight_market_time: Option<i64>,
    overnight_market_change_percent: Option<f64>,
}

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let provider_symbol = symbol.to_uppercase();
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{provider_symbol}?range=1d&interval=1m"
    );
    let response: YahooResponse = client
        .get(url)
        .send()
        .await
        .context("Yahoo request failed")?
        .error_for_status()
        .context("Yahoo returned HTTP error")?
        .json()
        .await
        .context("Yahoo JSON parse failed")?;

    let result = yahoo_result(response)?;
    let meta = result.meta;
    let price = meta
        .regular_market_price
        .or_else(|| {
            result
                .indicators
                .as_ref()
                .and_then(|indicators| indicators.quote.as_ref())
                .and_then(|blocks| blocks.first())
                .and_then(|block| block.close.as_ref())
                .and_then(|closes| closes.iter().rev().flatten().next().copied())
        })
        .ok_or_else(|| anyhow!("Yahoo response missing usable price"))?;
    let previous_close = meta
        .regular_market_previous_close
        .or(meta.previous_close)
        .or(meta.chart_previous_close);

    Ok(Quote {
        symbol: symbol.to_string(),
        price,
        currency: meta.currency,
        provider: "yahoo".to_string(),
        session: Some("regular".to_string()),
        fetched_at_utc: utc_now(),
        market_time: meta.regular_market_time.and_then(timestamp_sec_to_utc),
        previous_close,
        open: None,
        high: None,
        low: None,
        volume: None,
        exchange: meta.exchange_name,
        provider_symbol: Some(provider_symbol),
        change_pct: change_pct(price, previous_close),
    })
}

pub async fn fetch_extended_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let provider_symbol = symbol.to_uppercase();
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{provider_symbol}?range=2d&interval=1m&includePrePost=true"
    );
    let response: YahooResponse = client
        .get(url)
        .send()
        .await
        .context("Yahoo extended request failed")?
        .error_for_status()
        .context("Yahoo extended returned HTTP error")?
        .json()
        .await
        .context("Yahoo extended JSON parse failed")?;

    let result = yahoo_result(response)?;
    let meta = result.meta;
    let timestamps = result.timestamp;
    let quote_block = result
        .indicators
        .as_ref()
        .and_then(|indicators| indicators.quote.as_ref())
        .and_then(|blocks| blocks.first())
        .ok_or_else(|| anyhow!("Yahoo extended response missing quote block"))?;
    let (index, price) =
        last_close_index(quote_block).ok_or_else(|| anyhow!("Yahoo extended missing close"))?;
    let market_timestamp = timestamps
        .as_ref()
        .and_then(|timestamps| timestamps.get(index))
        .copied();
    let previous_close = meta
        .regular_market_previous_close
        .or(meta.previous_close)
        .or(meta.chart_previous_close);
    let session = market_timestamp
        .map(|timestamp| classify_session(&meta, timestamp))
        .unwrap_or_else(|| "extended".to_string());
    let market_time = market_timestamp
        .and_then(timestamp_sec_to_utc)
        .or_else(|| meta.regular_market_time.and_then(timestamp_sec_to_utc));

    Ok(Quote {
        symbol: symbol.to_uppercase(),
        price,
        currency: meta.currency,
        provider: "yahoo-extended".to_string(),
        session: Some(session),
        fetched_at_utc: utc_now(),
        market_time,
        previous_close,
        open: option_at_f64(quote_block.open.as_ref(), index),
        high: option_at_f64(quote_block.high.as_ref(), index),
        low: option_at_f64(quote_block.low.as_ref(), index),
        volume: option_at_u64(quote_block.volume.as_ref(), index),
        exchange: meta.exchange_name,
        provider_symbol: Some(provider_symbol),
        change_pct: change_pct(price, previous_close),
    })
}

pub async fn fetch_session_points(
    client: &Client,
    symbol: &str,
    timezone: &str,
) -> Result<Vec<PricePoint>> {
    let provider_symbol = symbol.to_uppercase();
    let response = fetch_yahoo_v7_quote(client, &provider_symbol)
        .await
        .with_context(|| format!("Yahoo session request failed for {provider_symbol}"))?;
    let node = yahoo_v7_result(response)?;
    let symbol = node
        .symbol
        .clone()
        .unwrap_or_else(|| provider_symbol.clone());
    let currency = node.currency.clone();
    let exchange = node.full_exchange_name.clone().or(node.exchange.clone());
    let previous_close = node.regular_market_previous_close;
    let mut points = Vec::new();

    push_session_point(
        &mut points,
        SessionPointInput {
            label: "Regular",
            symbol: &symbol,
            price: node.regular_market_price,
            currency: currency.clone(),
            session: SESSION_REGULAR,
            market_time: node.regular_market_time,
            change_pct_value: change_pct(
                node.regular_market_price.unwrap_or_default(),
                previous_close,
            ),
            previous_close,
            open: node.regular_market_open,
            high: node.regular_market_day_high,
            low: node.regular_market_day_low,
            volume: node.regular_market_volume,
            exchange: exchange.clone(),
            timezone,
            note: "Yahoo regular market",
        },
    );
    push_session_point(
        &mut points,
        SessionPointInput {
            label: "Premarket",
            symbol: &symbol,
            price: node.pre_market_price,
            currency: currency.clone(),
            session: SESSION_PRE,
            market_time: node.pre_market_time,
            change_pct_value: node.pre_market_change_percent,
            previous_close,
            open: node.regular_market_open,
            high: node.regular_market_day_high,
            low: node.regular_market_day_low,
            volume: node.regular_market_volume,
            exchange: exchange.clone(),
            timezone,
            note: "Yahoo pre-market",
        },
    );
    push_session_point(
        &mut points,
        SessionPointInput {
            label: "Postmarket",
            symbol: &symbol,
            price: node.post_market_price,
            currency: currency.clone(),
            session: SESSION_POST,
            market_time: node.post_market_time,
            change_pct_value: node.post_market_change_percent,
            previous_close,
            open: node.regular_market_open,
            high: node.regular_market_day_high,
            low: node.regular_market_day_low,
            volume: node.regular_market_volume,
            exchange: exchange.clone(),
            timezone,
            note: "Yahoo post-market",
        },
    );
    push_session_point(
        &mut points,
        SessionPointInput {
            label: "Overnight",
            symbol: &symbol,
            price: node.overnight_market_price,
            currency,
            session: SESSION_OVERNIGHT,
            market_time: node.overnight_market_time,
            change_pct_value: node.overnight_market_change_percent,
            previous_close,
            open: node.regular_market_open,
            high: node.regular_market_day_high,
            low: node.regular_market_day_low,
            volume: node.regular_market_volume,
            exchange,
            timezone,
            note: "Yahoo BOATS overnight",
        },
    );

    Ok(points)
}

pub async fn fetch_history(client: &Client, request: &HistoryRequest) -> Result<HistoryBatch> {
    fetch_history_inner(client, request, false, "yahoo").await
}

pub async fn fetch_quote_summary(client: &Client, symbol: &str, modules: &[&str]) -> Result<Value> {
    let provider_symbol = symbol.to_uppercase();
    let mut url = Url::parse(&format!("{YAHOO_QUOTE_SUMMARY_BASE_URL}/{provider_symbol}"))
        .context("invalid Yahoo quoteSummary URL")?;
    url.query_pairs_mut()
        .append_pair("modules", &modules.join(","))
        .append_pair("formatted", "false")
        .append_pair("lang", "en-US")
        .append_pair("region", "US")
        .append_pair("corsDomain", "finance.yahoo.com");
    fetch_json_with_crumb_retry(client, url.as_str(), "Yahoo quoteSummary").await
}

pub async fn fetch_options(client: &Client, symbol: &str, expiry: Option<i64>) -> Result<Value> {
    let provider_symbol = symbol.to_uppercase();
    let mut url = Url::parse(&format!("{YAHOO_OPTIONS_BASE_URL}/{provider_symbol}"))
        .context("invalid Yahoo options URL")?;
    if let Some(expiry) = expiry {
        url.query_pairs_mut()
            .append_pair("date", &expiry.to_string());
    }
    fetch_json_with_crumb_retry(client, url.as_str(), "Yahoo options").await
}

pub async fn fetch_search(
    client: &Client,
    query: &str,
    quotes_count: usize,
    news_count: usize,
) -> Result<Value> {
    let mut url = Url::parse(YAHOO_SEARCH_URL).context("invalid Yahoo search URL")?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("quotesCount", &quotes_count.clamp(0, 50).to_string())
        .append_pair("newsCount", &news_count.clamp(0, 50).to_string())
        .append_pair("enableFuzzyQuery", "false")
        .append_pair("quotesQueryId", "tss_match_phrase_query")
        .append_pair("newsQueryId", "news_cie_vespa")
        .append_pair("lang", "en-US")
        .append_pair("region", "US");
    fetch_json_with_crumb_retry(client, url.as_str(), "Yahoo search").await
}

pub async fn fetch_screen(client: &Client, screener: &str, count: usize) -> Result<Value> {
    let mut url = Url::parse(YAHOO_SCREENER_URL).context("invalid Yahoo screener URL")?;
    url.query_pairs_mut()
        .append_pair("scrIds", screener)
        .append_pair("count", &count.clamp(1, 250).to_string())
        .append_pair("formatted", "false")
        .append_pair("lang", "en-US")
        .append_pair("region", "US");
    fetch_json_with_crumb_retry(client, url.as_str(), "Yahoo screener").await
}

pub async fn fetch_extended_history(
    client: &Client,
    request: &HistoryRequest,
) -> Result<HistoryBatch> {
    fetch_history_inner(client, request, true, "yahoo-extended").await
}

async fn fetch_history_inner(
    client: &Client,
    request: &HistoryRequest,
    include_prepost: bool,
    provider: &str,
) -> Result<HistoryBatch> {
    let provider_symbol = request.symbol.to_uppercase();
    let include_prepost = if include_prepost {
        "&includePrePost=true"
    } else {
        ""
    };
    let events = if request.actions {
        "&events=div%2Csplits%2CcapitalGains"
    } else {
        ""
    };
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{provider_symbol}?range={range}&interval={interval}{include_prepost}{events}&includeAdjustedClose=true",
        range = request.range,
        interval = request.interval,
    );
    let response: YahooResponse = client
        .get(url)
        .send()
        .await
        .context("Yahoo history request failed")?
        .error_for_status()
        .context("Yahoo history returned HTTP error")?
        .json()
        .await
        .context("Yahoo history JSON parse failed")?;
    let result = yahoo_result(response)?;
    let timestamps = result
        .timestamp
        .ok_or_else(|| anyhow!("Yahoo history missing timestamps"))?;
    let events = result.events;
    let quote_block = result
        .indicators
        .as_ref()
        .and_then(|indicators| indicators.quote.as_ref())
        .and_then(|blocks| blocks.first())
        .ok_or_else(|| anyhow!("Yahoo history missing quote block"))?;
    let adjclose_block = result
        .indicators
        .as_ref()
        .and_then(|indicators| indicators.adjclose.as_ref())
        .and_then(|blocks| blocks.first());
    let action_index = request
        .actions
        .then(|| ActionIndex::from_events(events.as_ref()));

    let mut bars: Vec<OhlcBar> = timestamps
        .iter()
        .enumerate()
        .filter_map(|(index, timestamp)| {
            let close = option_at_f64(quote_block.close.as_ref(), index)?;
            let open_time = timestamp_sec_to_utc(*timestamp)?;
            let action = action_index
                .as_ref()
                .and_then(|index| index.values(*timestamp));
            Some(OhlcBar {
                symbol: request.symbol.to_uppercase(),
                provider: provider.to_string(),
                open_time,
                close_time: None,
                open: option_at_f64(quote_block.open.as_ref(), index),
                high: option_at_f64(quote_block.high.as_ref(), index),
                low: option_at_f64(quote_block.low.as_ref(), index),
                close,
                adj_close: adjclose_block
                    .and_then(|block| option_at_f64(block.adjclose.as_ref(), index)),
                volume: option_at_u64(quote_block.volume.as_ref(), index).map(|value| value as f64),
                quote_volume: None,
                trades: None,
                dividend: action.as_ref().and_then(|action| action.dividend),
                stock_split: action.as_ref().and_then(|action| action.stock_split),
                capital_gain: action.as_ref().and_then(|action| action.capital_gain),
                repaired: false,
            })
        })
        .rev()
        .take(request.limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let repair_applied =
        apply_history_adjustment_and_repair(&mut bars, request.adjustment, request.repair);

    Ok(HistoryBatch {
        symbol: request.symbol.to_uppercase(),
        provider: provider.to_string(),
        interval: request.interval.clone(),
        adjustment: request.adjustment.label().to_string(),
        actions_included: request.actions,
        repair_requested: request.repair,
        repair_applied,
        bars,
    })
}

fn yahoo_result(response: YahooResponse) -> Result<YahooResult> {
    if let Some(error) = response.chart.error {
        return Err(anyhow!("Yahoo error: {error}"));
    }
    response
        .chart
        .result
        .and_then(|mut results| {
            if results.is_empty() {
                None
            } else {
                Some(results.remove(0))
            }
        })
        .ok_or_else(|| anyhow!("Yahoo response missing result"))
}

#[derive(Debug, Default, Clone)]
struct ActionValues {
    dividend: Option<f64>,
    stock_split: Option<f64>,
    capital_gain: Option<f64>,
}

#[derive(Debug, Default)]
struct ActionIndex {
    by_timestamp: std::collections::BTreeMap<i64, ActionValues>,
    by_date: std::collections::BTreeMap<String, ActionValues>,
}

impl ActionIndex {
    fn from_events(events: Option<&YahooEvents>) -> Self {
        let mut index = Self::default();
        let Some(events) = events else {
            return index;
        };

        if let Some(dividends) = events.dividends.as_ref() {
            for (key, event) in dividends {
                let timestamp = event_timestamp(key, event.date);
                index.update(timestamp, |values| values.dividend = event.amount);
            }
        }
        if let Some(splits) = events.splits.as_ref() {
            for (key, event) in splits {
                let ratio = match (event.numerator, event.denominator) {
                    (Some(numerator), Some(denominator)) if denominator != 0.0 => {
                        Some(numerator / denominator)
                    }
                    _ => None,
                };
                let timestamp = event_timestamp(key, event.date);
                index.update(timestamp, |values| values.stock_split = ratio);
            }
        }
        if let Some(capital_gains) = events.capital_gains.as_ref() {
            for (key, event) in capital_gains {
                let timestamp = event_timestamp(key, event.date);
                index.update(timestamp, |values| values.capital_gain = event.amount);
            }
        }

        index
    }

    fn values(&self, timestamp: i64) -> Option<ActionValues> {
        self.by_timestamp
            .get(&timestamp)
            .cloned()
            .or_else(|| date_key(timestamp).and_then(|date| self.by_date.get(&date).cloned()))
            .filter(|values| {
                values.dividend.is_some()
                    || values.stock_split.is_some()
                    || values.capital_gain.is_some()
            })
    }

    fn update<F>(&mut self, timestamp: Option<i64>, mut update: F)
    where
        F: FnMut(&mut ActionValues),
    {
        let Some(timestamp) = timestamp else {
            return;
        };
        update(self.by_timestamp.entry(timestamp).or_default());
        if let Some(date) = date_key(timestamp) {
            update(self.by_date.entry(date).or_default());
        }
    }
}

fn event_timestamp(key: &str, date: Option<i64>) -> Option<i64> {
    date.or_else(|| key.parse::<i64>().ok())
}

fn date_key(timestamp: i64) -> Option<String> {
    timestamp_sec_to_utc(timestamp).and_then(|value| value.get(..10).map(str::to_string))
}

fn yahoo_v7_result(response: YahooV7Envelope) -> Result<YahooV7QuoteNode> {
    if let Some(error) = response.quote_response.error {
        return Err(anyhow!("Yahoo v7 error: {error}"));
    }
    response
        .quote_response
        .result
        .and_then(|mut results| {
            if results.is_empty() {
                None
            } else {
                Some(results.remove(0))
            }
        })
        .ok_or_else(|| anyhow!("Yahoo v7 response missing result"))
}

async fn fetch_yahoo_v7_quote(client: &Client, provider_symbol: &str) -> Result<YahooV7Envelope> {
    let mut url = Url::parse(YAHOO_QUOTE_V7_URL).context("invalid Yahoo v7 URL")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("symbols", provider_symbol);
        query.append_pair("formatted", "false");
        query.append_pair("lang", "en-US");
        query.append_pair("region", "US");
        query.append_pair("overnightPrice", "true");
    }
    fetch_json_with_crumb_retry(client, url.as_str(), "Yahoo v7").await
}

async fn fetch_yahoo_crumb(client: &Client) -> Result<String> {
    let cookie_response = client
        .get(YAHOO_COOKIE_URL)
        .send()
        .await
        .context("Yahoo cookie request failed")?;
    drop(cookie_response);

    let response = client
        .get(YAHOO_CRUMB_URL)
        .send()
        .await
        .context("Yahoo crumb request failed")?;
    let status = response.status();
    let crumb = response
        .text()
        .await
        .context("Yahoo crumb response text parse failed")?;
    if !status.is_success() {
        return Err(anyhow!("Yahoo crumb returned HTTP {status}: {crumb}"));
    }
    if crumb.is_empty() || crumb.contains('{') || crumb.contains('<') {
        return Err(anyhow!("Yahoo crumb response was invalid: {crumb}"));
    }
    Ok(crumb)
}

async fn fetch_json_with_crumb_retry<T>(client: &Client, url: &str, label: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let (status, body) = request_json_text(client, url, label).await?;
    if status.is_success() {
        return serde_json::from_str(&body).with_context(|| format!("{label} JSON parse failed"));
    }
    if !matches!(status.as_u16(), 401 | 403 | 429) {
        return Err(anyhow!("{label} returned HTTP {status}: {body}"));
    }

    let crumb = fetch_yahoo_crumb(client).await?;
    let mut url = Url::parse(url).with_context(|| format!("invalid {label} URL"))?;
    url.query_pairs_mut().append_pair("crumb", &crumb);
    let (status, body) = request_json_text(client, url.as_str(), label).await?;
    if !status.is_success() {
        return Err(anyhow!(
            "{label} returned HTTP {status} after crumb retry: {body}"
        ));
    }
    serde_json::from_str(&body)
        .with_context(|| format!("{label} JSON parse failed after crumb retry"))
}

async fn request_json_text(
    client: &Client,
    url: &str,
    label: &str,
) -> Result<(wreq::StatusCode, String)> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .with_context(|| format!("{label} request failed"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("{label} response text parse failed"))?;
    Ok((status, body))
}

fn option_at_f64(values: Option<&Vec<Option<f64>>>, index: usize) -> Option<f64> {
    values
        .and_then(|values| values.get(index))
        .and_then(|value| *value)
}

fn option_at_u64(values: Option<&Vec<Option<u64>>>, index: usize) -> Option<u64> {
    values
        .and_then(|values| values.get(index))
        .and_then(|value| *value)
}

fn last_close_index(block: &YahooQuoteBlock) -> Option<(usize, f64)> {
    block
        .close
        .as_ref()?
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, close)| close.map(|close| (index, close)))
}

fn classify_session(meta: &YahooMeta, timestamp: i64) -> String {
    let Some(periods) = meta.current_trading_period.as_ref() else {
        return "extended".to_string();
    };
    for (name, period) in [
        (SESSION_PRE, periods.pre.as_ref()),
        (SESSION_REGULAR, periods.regular.as_ref()),
        (SESSION_POST, periods.post.as_ref()),
    ] {
        if period_contains(period, timestamp) {
            return name.to_string();
        }
    }
    SESSION_EXTENDED.to_string()
}

fn period_contains(period: Option<&YahooTradingPeriod>, timestamp: i64) -> bool {
    let Some(period) = period else {
        return false;
    };
    match (period.start, period.end) {
        (Some(start), Some(end)) => timestamp >= start && timestamp < end,
        _ => false,
    }
}

struct SessionPointInput<'a> {
    label: &'static str,
    symbol: &'a str,
    price: Option<f64>,
    currency: Option<String>,
    session: &'static str,
    market_time: Option<i64>,
    change_pct_value: Option<f64>,
    previous_close: Option<f64>,
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    volume: Option<u64>,
    exchange: Option<String>,
    timezone: &'a str,
    note: &'static str,
}

fn push_session_point(points: &mut Vec<PricePoint>, input: SessionPointInput<'_>) {
    let Some(price) = input.price else {
        return;
    };
    let market_time_utc = input.market_time.and_then(timestamp_sec_to_utc);
    points.push(PricePoint {
        label: input.label.to_string(),
        symbol: input.symbol.to_string(),
        price: Some(price),
        currency: input.currency,
        provider: "yahoo-boats".to_string(),
        session: Some(input.session.to_string()),
        market_time_local: utc_to_local(market_time_utc.as_deref(), input.timezone),
        market_time_utc,
        change_pct: input
            .change_pct_value
            .or_else(|| change_pct(price, input.previous_close)),
        previous_close: input.previous_close,
        open: input.open,
        high: input.high,
        low: input.low,
        volume: input.volume,
        exchange: input.exchange,
        note: Some(input.note.to_string()),
    });
}
