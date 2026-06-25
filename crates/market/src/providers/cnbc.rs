use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use url::Url;
use wreq::Client;

use crate::http::{change_pct, utc_now};
use crate::model::{Quote, ResearchHighlight, research_value_string};

const BASE_URL: &str = "https://quote.cnbc.com/quote-html-webservice/quote.htm";

#[derive(Debug, Deserialize)]
struct CnbcResponse {
    #[serde(rename = "ITVQuoteResult")]
    result: CnbcQuoteResult,
}

#[derive(Debug, Deserialize)]
struct CnbcQuoteResult {
    #[serde(rename = "ITVQuote")]
    quote: Option<OneOrMany<CnbcQuote>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    fn into_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(value) => vec![value],
            OneOrMany::Many(values) => values,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CnbcQuote {
    symbol: Option<String>,
    last: Option<String>,
    last_timedate: Option<String>,
    exchange: Option<String>,
    open: Option<String>,
    high: Option<String>,
    low: Option<String>,
    volume: Option<String>,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    previous_day_closing: Option<String>,
    change_pct: Option<String>,
    #[serde(rename = "feedSymbol")]
    feed_symbol: Option<String>,
    alt_symbol: Option<String>,
    curmktstatus: Option<String>,
    #[serde(rename = "ExtendedMktQuote")]
    extended_market_quote: Option<CnbcExtendedQuote>,
}

#[derive(Debug, Deserialize)]
struct CnbcExtendedQuote {
    #[serde(rename = "type")]
    quote_type: Option<String>,
    last: Option<String>,
    last_timedate: Option<String>,
    last_time: Option<String>,
    change_pct: Option<String>,
    volume: Option<String>,
}

pub async fn fetch_quote(client: &Client, symbol: &str) -> Result<Quote> {
    let provider_symbol = symbol.to_uppercase();
    let mut url = Url::parse(BASE_URL).context("invalid CNBC base URL")?;
    url.query_pairs_mut()
        .append_pair("symbols", &provider_symbol)
        .append_pair("requestMethod", "itv")
        .append_pair("noform", "1")
        .append_pair("partnerId", "2")
        .append_pair("fund", "1")
        .append_pair("exthrs", "1")
        .append_pair("output", "json");
    let response: CnbcResponse = client
        .get(url.as_str())
        .send()
        .await
        .context("CNBC quote request failed")?
        .error_for_status()
        .context("CNBC quote returned HTTP error")?
        .json()
        .await
        .context("CNBC quote JSON parse failed")?;
    let quote = response
        .result
        .quote
        .map(OneOrMany::into_vec)
        .and_then(|mut quotes| quotes.pop())
        .ok_or_else(|| anyhow!("CNBC response missing quote"))?;

    let extended = quote.extended_market_quote.as_ref();
    let extended_price = extended.and_then(|quote| parse_market_f64(quote.last.as_deref()));
    let regular_price = parse_market_f64(quote.last.as_deref());
    let price = extended_price
        .or(regular_price)
        .ok_or_else(|| anyhow!("CNBC quote missing usable price"))?;
    let previous_close = parse_market_f64(quote.previous_day_closing.as_deref());
    let using_extended = extended_price.is_some();
    let session = if using_extended {
        session_label(extended.and_then(|quote| quote.quote_type.as_deref()))
    } else {
        session_label(quote.curmktstatus.as_deref())
    };
    let market_time = if using_extended {
        extended
            .and_then(|quote| {
                quote
                    .last_timedate
                    .clone()
                    .or_else(|| quote.last_time.clone())
            })
            .or_else(|| quote.last_timedate.clone())
    } else {
        quote.last_timedate.clone()
    };
    let provider_change_pct = if using_extended {
        extended.and_then(|quote| parse_market_f64(quote.change_pct.as_deref()))
    } else {
        parse_market_f64(quote.change_pct.as_deref())
    };
    let change_pct_value = change_pct(price, previous_close).or(provider_change_pct);
    let volume = if using_extended {
        extended.and_then(|quote| parse_market_u64(quote.volume.as_deref()))
    } else {
        parse_market_u64(quote.volume.as_deref())
    };
    let open = non_zero(parse_market_f64(quote.open.as_deref()));
    let high = non_zero(parse_market_f64(quote.high.as_deref()));
    let low = non_zero(parse_market_f64(quote.low.as_deref()));
    let symbol = quote.symbol.unwrap_or_else(|| provider_symbol.clone());
    let currency = quote.currency_code;
    let exchange = quote.exchange;
    let provider_symbol = quote
        .feed_symbol
        .or(quote.alt_symbol)
        .or(Some(provider_symbol));

    Ok(Quote {
        symbol,
        price,
        currency,
        provider: "cnbc-extended".to_string(),
        session: Some(session),
        fetched_at_utc: utc_now(),
        market_time,
        previous_close,
        open,
        high,
        low,
        volume,
        exchange,
        provider_symbol,
        change_pct: change_pct_value,
    })
}

pub async fn fetch_quote_payload(client: &Client, symbol: &str) -> Result<Value> {
    let provider_symbol = symbol.to_uppercase();
    let mut url = Url::parse(BASE_URL).context("invalid CNBC base URL")?;
    url.query_pairs_mut()
        .append_pair("symbols", &provider_symbol)
        .append_pair("requestMethod", "itv")
        .append_pair("noform", "1")
        .append_pair("partnerId", "2")
        .append_pair("fund", "1")
        .append_pair("exthrs", "1")
        .append_pair("output", "json");
    client
        .get(url.as_str())
        .send()
        .await
        .context("CNBC quote payload request failed")?
        .error_for_status()
        .context("CNBC quote payload returned HTTP error")?
        .json()
        .await
        .context("CNBC quote payload JSON parse failed")
}

pub fn fundamentals_highlights(payload: &Value) -> Vec<ResearchHighlight> {
    let mut rows = Vec::new();
    let Some(quote) = first_quote(payload) else {
        return rows;
    };
    for (label, path) in [
        ("Company", "/name"),
        ("Exchange", "/exchange"),
        ("Quote source", "/provider"),
        ("Current status", "/curmktstatus"),
        ("Market cap", "/mktcapView"),
        ("PE", "/pe"),
        ("Forward PE", "/fpe"),
        ("EPS", "/eps"),
        ("Forward EPS", "/feps"),
        ("Dividend", "/dividend"),
        ("Dividend yield", "/dividendyield"),
        ("Beta", "/beta"),
        ("10-day avg volume", "/tendayavgvol"),
        ("Revenue TTM", "/revenuettm"),
        ("Price/Sales", "/psales"),
        ("Forward Sales", "/fsales"),
        ("Shares Out", "/sharesout"),
        ("ROE TTM", "/ROETTM"),
        ("Net margin TTM", "/NETPROFTTM"),
        ("Gross margin TTM", "/GROSMGNTTM"),
        ("EBITDA TTM", "/TTMEBITD"),
        ("Debt/Equity", "/DEBTEQTYQ"),
        ("52-week high", "/yrhiprice"),
        ("52-week low", "/yrloprice"),
    ] {
        push_path(&mut rows, quote, label, path);
    }
    if let Some(extended) = quote.get("ExtendedMktQuote") {
        for (label, path) in [
            ("Extended type", "/type"),
            ("Extended price", "/last"),
            ("Extended change", "/change"),
            ("Extended change pct", "/change_pct"),
            ("Extended volume", "/volume"),
            ("Extended time", "/last_time"),
        ] {
            if let Some(value) = string_at(extended, path) {
                rows.push(ResearchHighlight::new(
                    label,
                    value,
                    "cnbc",
                    "fundamentals-lite",
                ));
            }
        }
    }
    rows
}

fn first_quote(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/ITVQuoteResult/ITVQuote")
        .and_then(|quote| match quote {
            Value::Array(quotes) => quotes.first(),
            Value::Object(_) => Some(quote),
            _ => None,
        })
        .or_else(|| {
            payload
                .pointer("/FormattedQuoteResult/FormattedQuote")
                .and_then(|quote| match quote {
                    Value::Array(quotes) => quotes.first(),
                    Value::Object(_) => Some(quote),
                    _ => None,
                })
        })
}

fn push_path(rows: &mut Vec<ResearchHighlight>, payload: &Value, label: &str, path: &str) {
    if let Some(row) =
        ResearchHighlight::from_path(Some(payload), label, path, "cnbc", "fundamentals-lite")
            .filter(|row| !row.value.trim().is_empty())
    {
        rows.push(row);
    }
}

fn string_at(payload: &Value, path: &str) -> Option<String> {
    research_value_string(payload.pointer(path)).filter(|value| !value.trim().is_empty())
}

fn parse_market_f64(value: Option<&str>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() || value == "N/D" || value == "--" {
        return None;
    }
    let cleaned = value
        .trim_start_matches('$')
        .trim_end_matches('%')
        .replace(',', "");
    cleaned.parse::<f64>().ok()
}

fn parse_market_u64(value: Option<&str>) -> Option<u64> {
    parse_market_f64(value).map(|value| value as u64)
}

fn non_zero(value: Option<f64>) -> Option<f64> {
    value.filter(|value| *value != 0.0)
}

fn session_label(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "regular".to_string();
    };
    value.to_ascii_lowercase()
}
