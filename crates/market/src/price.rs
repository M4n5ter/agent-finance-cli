use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use wreq::Client;

use crate::args::SessionMode;
use crate::http::{change_pct, utc_now};
use crate::model::{
    PricePoint, PriceSummary, Quote, RegularBasis, SESSION_EXTENDED, SESSION_OVERNIGHT,
    SESSION_POST, SESSION_PRE, SESSION_REGULAR,
};
use crate::providers::{self, binance, cnbc, robinhood, yahoo};
use crate::time::{now_local, utc_to_local};

pub async fn fetch_price_summary(
    client: &Client,
    symbol: &str,
    timezone: &str,
    mode: SessionMode,
    binance_config: Option<&binance::BinanceConfig>,
    proxy_symbol: Option<&str>,
) -> PriceSummary {
    let normalized = symbol.trim().to_uppercase();
    let fetched_at_utc = utc_now();
    let fetched_at_local = now_local(timezone);
    let mut errors = BTreeMap::new();
    let mut sessions = Vec::new();

    match yahoo::fetch_session_points(client, &normalized, timezone).await {
        Ok(points) => sessions.extend(points),
        Err(error) => {
            errors.insert("yahoo-boats".to_string(), format!("{error:#}"));
        }
    }

    if sessions.is_empty() {
        match providers::fetch_quote_without_boats(client, &normalized, "fallback").await {
            Ok(quote) => sessions.push(quote_to_point(
                quote,
                "Current price",
                timezone,
                Some("Yahoo/Stooq fallback".to_string()),
            )),
            Err(error) => {
                errors.insert("auto".to_string(), format!("{error:#}"));
            }
        }
    }

    if matches!(mode, SessionMode::All) {
        match cnbc::fetch_quote(client, &normalized).await {
            Ok(quote) => sessions.push(quote_to_point(
                quote,
                "CNBC extended cross-check",
                timezone,
                Some("CNBC ExtendedMktQuote cross-check".to_string()),
            )),
            Err(error) => {
                errors.insert("cnbc-extended".to_string(), format!("{error:#}"));
            }
        }
        match robinhood::fetch_quote(client, &normalized).await {
            Ok(quote) => sessions.push(quote_to_point(
                quote,
                "Robinhood extended cross-check",
                timezone,
                Some("Robinhood public quote cross-check".to_string()),
            )),
            Err(error) => {
                errors.insert("robinhood".to_string(), format!("{error:#}"));
            }
        }
    }

    let proxy = if let Some(proxy_symbol) = proxy_symbol {
        let result = match binance_config {
            Some(config) => binance::futures_quote(config, proxy_symbol).await,
            None => Err(anyhow::anyhow!(
                "Binance config is unavailable for proxy symbol"
            )),
        };
        match result {
            Ok(quote) => Some(quote_to_point(
                quote,
                "Binance USD-M proxy context",
                timezone,
                Some("Proxy price is for price discovery and sentiment monitoring; it is not the stock or legal-equity price".to_string()),
            )),
            Err(error) => {
                errors.insert(
                    format!("binance-usds-futures:{proxy_symbol}"),
                    format!("{error:#}"),
                );
                None
            }
        }
    } else {
        None
    };

    let regular_basis = regular_basis(&sessions);
    let current = choose_current(&sessions, mode).cloned();

    PriceSummary {
        symbol: normalized,
        timezone: timezone.to_string(),
        fetched_at_utc,
        fetched_at_local,
        current,
        regular_basis,
        sessions,
        proxy,
        errors,
    }
}

pub fn quote_to_point(
    quote: Quote,
    label: &str,
    timezone: &str,
    note: Option<String>,
) -> PricePoint {
    PricePoint {
        label: label.to_string(),
        symbol: quote.symbol,
        price: Some(quote.price),
        currency: quote.currency,
        provider: quote.provider,
        session: quote.session,
        market_time_local: utc_to_local(quote.market_time.as_deref(), timezone),
        market_time_utc: quote.market_time,
        change_pct: quote
            .change_pct
            .or_else(|| change_pct(quote.price, quote.previous_close)),
        previous_close: quote.previous_close,
        open: quote.open,
        high: quote.high,
        low: quote.low,
        volume: quote.volume,
        exchange: quote.exchange,
        note,
    }
}

fn choose_current(sessions: &[PricePoint], mode: SessionMode) -> Option<&PricePoint> {
    match mode {
        SessionMode::Regular => sessions
            .iter()
            .find(|point| has_session(point, SESSION_REGULAR)),
        SessionMode::Extended => sessions
            .iter()
            .filter(|point| {
                has_session(point, SESSION_PRE)
                    || has_session(point, SESSION_POST)
                    || has_session(point, SESSION_EXTENDED)
            })
            .max_by_key(|point| point_time(point))
            .or_else(|| {
                sessions
                    .iter()
                    .find(|point| has_session(point, SESSION_REGULAR))
            }),
        SessionMode::Overnight => sessions
            .iter()
            .find(|point| has_session(point, SESSION_OVERNIGHT))
            .or_else(|| choose_current(sessions, SessionMode::Extended)),
        SessionMode::Smart | SessionMode::All => sessions
            .iter()
            .max_by_key(|point| point_time(point))
            .or_else(|| {
                [
                    SESSION_OVERNIGHT,
                    SESSION_POST,
                    SESSION_PRE,
                    SESSION_EXTENDED,
                    SESSION_REGULAR,
                ]
                .iter()
                .find_map(|session| sessions.iter().find(|point| has_session(point, session)))
            }),
    }
}

fn regular_basis(sessions: &[PricePoint]) -> RegularBasis {
    let regular = sessions
        .iter()
        .find(|point| has_session(point, SESSION_REGULAR));
    let fallback = sessions.first();
    let source = regular.or(fallback);
    RegularBasis {
        previous_close: source.and_then(|point| point.previous_close),
        open: source.and_then(|point| point.open),
        high: source.and_then(|point| point.high),
        low: source.and_then(|point| point.low),
        volume: source.and_then(|point| point.volume),
    }
}

fn has_session(point: &PricePoint, expected: &str) -> bool {
    point
        .session
        .as_deref()
        .map(|session| session.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn point_time(point: &PricePoint) -> i64 {
    point
        .market_time_utc
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc).timestamp())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(label: &str, session: &str, utc: &str, price: f64) -> PricePoint {
        PricePoint {
            label: label.to_string(),
            symbol: "CRDO".to_string(),
            price: Some(price),
            currency: Some("USD".to_string()),
            provider: "fixture".to_string(),
            session: Some(session.to_string()),
            market_time_utc: Some(utc.to_string()),
            market_time_local: None,
            change_pct: None,
            previous_close: Some(200.0),
            open: None,
            high: None,
            low: None,
            volume: None,
            exchange: None,
            note: None,
        }
    }

    #[test]
    fn smart_mode_uses_latest_observable_session_not_fixed_priority() {
        let sessions = vec![
            point("Regular", "regular", "2026-06-01T20:00:00Z", 226.1),
            point("Overnight", "overnight", "2026-06-02T07:00:00Z", 206.5),
        ];
        let current = choose_current(&sessions, SessionMode::Smart).unwrap();
        assert_eq!(current.session.as_deref(), Some("overnight"));
        assert_eq!(current.price, Some(206.5));
    }

    #[test]
    fn regular_mode_ignores_later_overnight_quote() {
        let sessions = vec![
            point("Regular", "regular", "2026-06-01T20:00:00Z", 226.1),
            point("Overnight", "overnight", "2026-06-02T07:00:00Z", 206.5),
        ];
        let current = choose_current(&sessions, SessionMode::Regular).unwrap();
        assert_eq!(current.session.as_deref(), Some("regular"));
        assert_eq!(current.price, Some(226.1));
    }
}
