use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;
use wreq::Client;

use crate::http::{change_pct, parse_optional_f64, utc_now};
use crate::model::{
    HistoryBatch, OhlcBar, Quote, ResearchHighlight, SESSION_EXTENDED, SESSION_REGULAR,
    research_value_string,
};

#[derive(Debug, Deserialize)]
struct RobinhoodQuote {
    symbol: String,
    last_trade_price: Option<String>,
    last_extended_hours_trade_price: Option<String>,
    last_non_reg_trade_price: Option<String>,
    venue_last_trade_time: Option<String>,
    venue_last_non_reg_trade_time: Option<String>,
    previous_close: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RobinhoodInstrumentPage {
    results: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct RobinhoodHistorical {
    symbol: String,
    interval: String,
    historicals: Vec<RobinhoodHistoricalBar>,
}

#[derive(Debug, Deserialize)]
struct RobinhoodHistoricalBar {
    begins_at: Option<String>,
    open_price: Option<String>,
    close_price: Option<String>,
    high_price: Option<String>,
    low_price: Option<String>,
    volume: Option<u64>,
    interpolated: Option<bool>,
}

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let provider_symbol = symbol.to_uppercase();
    let url = format!("https://api.robinhood.com/quotes/{provider_symbol}/");
    let quote: RobinhoodQuote = client
        .get(url)
        .send()
        .await
        .context("Robinhood quote request failed")?
        .error_for_status()
        .context("Robinhood quote returned HTTP error")?
        .json()
        .await
        .context("Robinhood quote JSON parse failed")?;

    let extended_price = parse_optional_f64(quote.last_extended_hours_trade_price.as_deref())
        .or_else(|| parse_optional_f64(quote.last_non_reg_trade_price.as_deref()));
    let regular_price = parse_optional_f64(quote.last_trade_price.as_deref());
    let price = extended_price
        .or(regular_price)
        .ok_or_else(|| anyhow!("Robinhood quote missing usable price"))?;
    let previous_close = parse_optional_f64(quote.previous_close.as_deref());
    let session = if extended_price.is_some() {
        SESSION_EXTENDED
    } else {
        SESSION_REGULAR
    };
    let market_time = if extended_price.is_some() {
        quote
            .venue_last_non_reg_trade_time
            .or(quote.updated_at)
            .or(quote.venue_last_trade_time)
    } else {
        quote
            .venue_last_trade_time
            .or(quote.updated_at)
            .or(quote.venue_last_non_reg_trade_time)
    };

    Ok(Quote {
        symbol: quote.symbol,
        price,
        currency: Some("USD".to_string()),
        provider: "robinhood".to_string(),
        session: Some(session.to_string()),
        fetched_at_utc: utc_now(),
        market_time,
        previous_close,
        open: None,
        high: None,
        low: None,
        volume: None,
        exchange: None,
        provider_symbol: Some(provider_symbol),
        change_pct: change_pct(price, previous_close),
    })
}

pub async fn fetch_fundamentals_bundle(client: &Client, symbol: &str) -> Result<Value> {
    let instrument = fetch_instrument(client, symbol).await?;
    let fundamentals = fetch_json(
        client,
        &format!(
            "https://api.robinhood.com/fundamentals/{}/",
            symbol.trim().to_uppercase()
        ),
        "Robinhood fundamentals",
    )
    .await?;
    let market = instrument
        .get("market")
        .and_then(Value::as_str)
        .map(|url| fetch_json(client, url, "Robinhood market"));
    let market = match market {
        Some(future) => future.await.ok(),
        None => None,
    };

    Ok(json!({
        "symbol": symbol.trim().to_uppercase(),
        "instrument": instrument,
        "fundamentals": fundamentals,
        "market": market,
    }))
}

pub async fn fetch_events_bundle(client: &Client, symbol: &str) -> Result<Value> {
    let instrument = fetch_instrument(client, symbol).await?;
    let splits_url = instrument
        .get("splits")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Robinhood instrument missing splits URL"))?;
    let splits = fetch_json(client, splits_url, "Robinhood splits").await?;
    let market = instrument
        .get("market")
        .and_then(Value::as_str)
        .map(|url| fetch_json(client, url, "Robinhood market"));
    let market = match market {
        Some(future) => future.await.ok(),
        None => None,
    };
    let hours = market
        .as_ref()
        .and_then(|market| market.get("todays_hours"))
        .and_then(Value::as_str)
        .map(|url| fetch_json(client, url, "Robinhood market hours"));
    let hours = match hours {
        Some(future) => future.await.ok(),
        None => None,
    };

    Ok(json!({
        "symbol": symbol.trim().to_uppercase(),
        "instrument": instrument,
        "splits": splits,
        "market": market,
        "market_hours": hours,
    }))
}

pub async fn fetch_options_bundle(
    client: &Client,
    symbol: &str,
    expiration_date: Option<&str>,
    count: usize,
) -> Result<Value> {
    let instrument = fetch_instrument(client, symbol).await?;
    let chain_id = instrument
        .get("tradable_chain_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Robinhood instrument missing tradable_chain_id"))?;
    let chain = fetch_json(
        client,
        &format!("https://api.robinhood.com/options/chains/{chain_id}/"),
        "Robinhood options chain",
    )
    .await?;
    let selected_expiration = expiration_date.map(ToString::to_string).or_else(|| {
        chain
            .get("expiration_dates")
            .and_then(Value::as_array)
            .and_then(|dates| dates.first())
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    let instruments = match selected_expiration.as_deref() {
        Some(expiration) => fetch_option_instruments(client, chain_id, expiration, count).await?,
        None => Vec::new(),
    };
    let coverage_gaps = vec![
        "Robinhood option quote marketdata endpoints are not reliably anonymous; this payload includes chain and contract metadata, not bid/ask/IV quotes.",
    ];

    Ok(json!({
        "symbol": symbol.trim().to_uppercase(),
        "instrument": instrument,
        "chain": chain,
        "selected_expiration": selected_expiration,
        "option_instruments": instruments,
        "coverage_gaps": coverage_gaps,
    }))
}

pub async fn fetch_history(
    client: &Client,
    symbol: &str,
    interval: &str,
    range: &str,
    extended: bool,
    limit: usize,
) -> Result<HistoryBatch> {
    let provider_symbol = symbol.trim().to_uppercase();
    let interval = robinhood_interval(interval)?;
    let span = robinhood_span(range)?;
    let bounds = if extended { "trading" } else { "regular" };
    let mut url = Url::parse(&format!(
        "https://api.robinhood.com/quotes/historicals/{provider_symbol}/"
    ))
    .context("invalid Robinhood historicals URL")?;
    url.query_pairs_mut()
        .append_pair("interval", interval)
        .append_pair("span", span)
        .append_pair("bounds", bounds);
    let history: RobinhoodHistorical = client
        .get(url.as_str())
        .send()
        .await
        .context("Robinhood historicals request failed")?
        .error_for_status()
        .context("Robinhood historicals returned HTTP error")?
        .json()
        .await
        .context("Robinhood historicals JSON parse failed")?;
    let mut bars = history
        .historicals
        .into_iter()
        .filter_map(|row| historical_bar_to_ohlc(&history.symbol, row))
        .collect::<Vec<_>>();
    if bars.len() > limit {
        let start = bars.len().saturating_sub(limit);
        bars = bars.split_off(start);
    }

    Ok(HistoryBatch {
        symbol: history.symbol,
        provider: "robinhood".to_string(),
        interval: history.interval,
        adjustment: "raw".to_string(),
        actions_included: false,
        repair_requested: false,
        repair_applied: false,
        bars,
    })
}

pub fn fundamentals_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    for (label, path, module) in [
        ("Company", "/instrument/name", "instrument"),
        ("List date", "/instrument/list_date", "instrument"),
        ("Tradability", "/instrument/tradability", "instrument"),
        (
            "Fractional trading",
            "/instrument/fractional_tradability",
            "instrument",
        ),
        (
            "Shorting status",
            "/instrument/short_selling_tradability",
            "instrument",
        ),
        (
            "High-risk flag",
            "/instrument/is_high_investment_risk",
            "instrument",
        ),
        ("SPAC flag", "/instrument/is_spac", "instrument"),
        ("Market cap", "/fundamentals/market_cap", "fundamentals"),
        ("PE", "/fundamentals/pe_ratio", "fundamentals"),
        ("PB", "/fundamentals/pb_ratio", "fundamentals"),
        (
            "Dividend yield",
            "/fundamentals/dividend_yield",
            "fundamentals",
        ),
        ("Float", "/fundamentals/float", "fundamentals"),
        (
            "Shares outstanding",
            "/fundamentals/shares_outstanding",
            "fundamentals",
        ),
        (
            "52-week high",
            "/fundamentals/high_52_weeks",
            "fundamentals",
        ),
        ("52-week low", "/fundamentals/low_52_weeks", "fundamentals"),
        (
            "30-day avg volume",
            "/fundamentals/average_volume_30_days",
            "fundamentals",
        ),
        ("Market", "/market/name", "market"),
        ("Timezone", "/market/timezone", "market"),
    ] {
        push_path(&mut rows, payload, label, path, module);
    }
    if let Some(description) = payload
        .pointer("/fundamentals/description")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        rows.push(ResearchHighlight::new(
            "Company description",
            truncate(description.trim(), 220),
            "robinhood",
            "fundamentals",
        ));
    }
    rows
}

pub fn events_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(
        &mut rows,
        payload,
        "Company",
        "/instrument/name",
        "instrument",
    );
    push_path(
        &mut rows,
        payload,
        "Status",
        "/instrument/state",
        "instrument",
    );
    push_path(
        &mut rows,
        payload,
        "Market open today",
        "/market_hours/is_open",
        "market_hours",
    );
    push_path(
        &mut rows,
        payload,
        "Regular open",
        "/market_hours/opens_at",
        "market_hours",
    );
    push_path(
        &mut rows,
        payload,
        "Regular close",
        "/market_hours/closes_at",
        "market_hours",
    );
    push_path(
        &mut rows,
        payload,
        "Extended open",
        "/market_hours/extended_opens_at",
        "market_hours",
    );
    push_path(
        &mut rows,
        payload,
        "Extended close",
        "/market_hours/extended_closes_at",
        "market_hours",
    );
    if let Some(splits) = payload.pointer("/splits/results").and_then(Value::as_array) {
        for (index, split) in splits.iter().take(5).enumerate() {
            let date = string_at(split, "/execution_date").unwrap_or_else(|| "-".to_string());
            let multiplier = string_at(split, "/multiplier").unwrap_or_else(|| "-".to_string());
            let divisor = string_at(split, "/divisor").unwrap_or_else(|| "-".to_string());
            rows.push(ResearchHighlight::new(
                &format!("Split {}", index + 1),
                format!("{date} multiplier={multiplier} divisor={divisor}"),
                "robinhood",
                "splits",
            ));
        }
    }
    rows
}

pub fn options_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    push_path(&mut rows, payload, "Chain", "/chain/id", "options");
    push_path(
        &mut rows,
        payload,
        "Selected Expiration",
        "/selected_expiration",
        "options",
    );
    if let Some(expirations) = payload
        .pointer("/chain/expiration_dates")
        .and_then(Value::as_array)
    {
        rows.push(ResearchHighlight::new(
            "Expiry count",
            expirations.len().to_string(),
            "robinhood",
            "options",
        ));
        rows.push(ResearchHighlight::new(
            "Nearest expiry",
            expirations
                .iter()
                .take(8)
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            "robinhood",
            "options",
        ));
    }
    if let Some(instruments) = payload
        .pointer("/option_instruments")
        .and_then(Value::as_array)
    {
        rows.push(ResearchHighlight::new(
            "Contract count",
            instruments.len().to_string(),
            "robinhood",
            "option_instruments",
        ));
        for contract in instruments.iter().take(6) {
            let strike = string_at(contract, "/strike_price").unwrap_or_else(|| "-".to_string());
            let contract_type = string_at(contract, "/type").unwrap_or_else(|| "-".to_string());
            let tradability =
                string_at(contract, "/tradability").unwrap_or_else(|| "-".to_string());
            rows.push(ResearchHighlight::new(
                "Contract sample",
                format!("{contract_type} strike={strike} tradability={tradability}"),
                "robinhood",
                "option_instruments",
            ));
        }
    }
    rows
}

async fn fetch_instrument(client: &Client, symbol: &str) -> Result<Value> {
    let provider_symbol = symbol.trim().to_uppercase();
    let url = format!("https://api.robinhood.com/instruments/?symbol={provider_symbol}");
    let page: RobinhoodInstrumentPage = client
        .get(url)
        .send()
        .await
        .context("Robinhood instruments request failed")?
        .error_for_status()
        .context("Robinhood instruments returned HTTP error")?
        .json()
        .await
        .context("Robinhood instruments JSON parse failed")?;
    page.results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("Robinhood instruments did not contain {provider_symbol}"))
}

async fn fetch_json(client: &Client, url: &str, label: &str) -> Result<Value> {
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("{label} request failed"))?
        .error_for_status()
        .with_context(|| format!("{label} returned HTTP error"))?
        .json()
        .await
        .with_context(|| format!("{label} JSON parse failed"))
}

async fn fetch_option_instruments(
    client: &Client,
    chain_id: &str,
    expiration: &str,
    count: usize,
) -> Result<Vec<Value>> {
    let mut url = Url::parse("https://api.robinhood.com/options/instruments/")
        .context("invalid Robinhood option instruments URL")?;
    url.query_pairs_mut()
        .append_pair("chain_id", chain_id)
        .append_pair("expiration_dates", expiration)
        .append_pair("state", "active");
    let page = fetch_json(client, url.as_str(), "Robinhood option instruments").await?;
    Ok(page
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(count.max(1))
        .collect())
}

fn robinhood_interval(interval: &str) -> Result<&'static str> {
    match interval {
        "5m" | "5minute" => Ok("5minute"),
        "10m" | "10minute" => Ok("10minute"),
        "1h" | "hour" => Ok("hour"),
        "1d" | "day" => Ok("day"),
        "1w" | "week" => Ok("week"),
        _ => Err(anyhow!(
            "Robinhood history supports 5m, 10m, 1h, 1d, and 1w intervals"
        )),
    }
}

fn robinhood_span(range: &str) -> Result<&'static str> {
    match range {
        "1d" | "day" => Ok("day"),
        "1w" | "5d" | "week" => Ok("week"),
        "1mo" | "month" => Ok("month"),
        "3mo" | "3month" => Ok("3month"),
        "1y" | "year" => Ok("year"),
        _ => Err(anyhow!(
            "Robinhood history supports 1d, 5d/1w, 1mo, 3mo, and 1y ranges"
        )),
    }
}

fn historical_bar_to_ohlc(symbol: &str, row: RobinhoodHistoricalBar) -> Option<OhlcBar> {
    let close = parse_optional_f64(row.close_price.as_deref())?;
    Some(OhlcBar {
        symbol: symbol.to_uppercase(),
        provider: "robinhood".to_string(),
        open_time: row.begins_at?,
        close_time: None,
        open: parse_optional_f64(row.open_price.as_deref()),
        high: parse_optional_f64(row.high_price.as_deref()),
        low: parse_optional_f64(row.low_price.as_deref()),
        close,
        adj_close: None,
        volume: row.volume.map(|value| value as f64),
        quote_volume: None,
        trades: None,
        dividend: None,
        stock_split: None,
        capital_gain: None,
        repaired: row.interpolated.unwrap_or(false),
    })
}

fn push_path(
    rows: &mut Vec<ResearchHighlight>,
    payload: &Value,
    label: &str,
    path: &str,
    module: &str,
) {
    if let Some(row) = ResearchHighlight::from_path(Some(payload), label, path, "robinhood", module)
        .filter(|row| !row.value.trim().is_empty())
    {
        rows.push(row);
    }
}

fn string_at(payload: &Value, path: &str) -> Option<String> {
    research_value_string(payload.pointer(path)).filter(|value| !value.trim().is_empty())
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>() + "..."
}
