use anyhow::Result;
use serde_json::Value;
use wreq::Client;

use super::QuoteSummaryKind;
use crate::cache;
use crate::http::utc_now;
use crate::providers::{cnbc, robinhood, sec_edgar, yahoo};

pub(super) async fn fetch_quote_summary_live(
    client: &Client,
    symbol: &str,
    modules: &[&str],
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = yahoo::fetch_quote_summary(client, symbol, modules).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("yahoo-quote-summary", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_options_live(
    client: &Client,
    symbol: &str,
    expiry: Option<i64>,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = yahoo::fetch_options(client, symbol, expiry).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("yahoo-options", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_search_live(
    client: &Client,
    query: &str,
    quotes_count: usize,
    news_count: usize,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = yahoo::fetch_search(client, query, quotes_count, news_count).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("yahoo-search", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_screen_live(
    client: &Client,
    screener: &str,
    count: usize,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = yahoo::fetch_screen(client, screener, count).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("yahoo-screen", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_sec_company_live(
    client: &Client,
    symbol: &str,
    include_companyfacts: bool,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = sec_edgar::fetch_company_bundle(client, symbol, include_companyfacts).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("sec-edgar-company", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_robinhood_live(
    client: &Client,
    symbol: &str,
    kind: QuoteSummaryKind,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = match kind {
        QuoteSummaryKind::Fundamentals => {
            robinhood::fetch_fundamentals_bundle(client, symbol).await?
        }
        QuoteSummaryKind::Events => robinhood::fetch_events_bundle(client, symbol).await?,
        QuoteSummaryKind::Analysis | QuoteSummaryKind::Ownership => unreachable!(),
    };
    let fetched_at_utc = utc_now();
    cache::write_json("robinhood-research", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_cnbc_live(
    client: &Client,
    symbol: &str,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = cnbc::fetch_quote_payload(client, symbol).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("cnbc-fundamentals-lite", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}

pub(super) async fn fetch_robinhood_options_live(
    client: &Client,
    symbol: &str,
    expiration_date: Option<&str>,
    count: usize,
    key: &str,
) -> Result<(String, String, Value)> {
    let payload = robinhood::fetch_options_bundle(client, symbol, expiration_date, count).await?;
    let fetched_at_utc = utc_now();
    cache::write_json("robinhood-options", key, &fetched_at_utc, &payload)?;
    Ok((fetched_at_utc, "live".to_string(), payload))
}
