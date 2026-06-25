use anyhow::Result;
use futures_util::future;
use serde::Serialize;

use crate::model::{PredictionSearchReport, ResearchHighlight, SearchReport};
use crate::service::{self, MarketRuntime, NewsRequest, PolymarketSearchRequest};

#[derive(Debug, Clone)]
pub struct ResearchContextSnapshotRequest {
    pub symbol: String,
    pub news_count: usize,
    pub prediction_count: usize,
    pub refresh: bool,
    pub cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResearchContextSnapshot {
    pub requested_symbol: String,
    pub symbol: String,
    pub fetched_at_local: Option<String>,
    pub news: Vec<ResearchNewsSnapshot>,
    pub prediction_markets: Vec<PredictionMarketSnapshot>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResearchNewsSnapshot {
    pub title: String,
    pub provider: String,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PredictionMarketSnapshot {
    pub title: String,
    pub probability: Option<f64>,
    pub volume: Option<f64>,
    pub liquidity: Option<f64>,
    pub market_url: Option<String>,
}

pub async fn fetch_research_context_snapshot(
    runtime: &MarketRuntime,
    request: ResearchContextSnapshotRequest,
) -> ResearchContextSnapshot {
    let symbol = request.symbol;
    let news = service::news(
        runtime,
        NewsRequest {
            symbol: symbol.clone(),
            count: request.news_count,
            refresh: request.refresh,
            cache_ttl_seconds: request.cache_ttl_seconds,
        },
    );
    let prediction_markets = service::polymarket_search(
        runtime,
        PolymarketSearchRequest {
            query: symbol.clone(),
            limit: request.prediction_count,
            include_closed: false,
            min_volume: None,
            refresh: request.refresh,
            cache_ttl_seconds: request.cache_ttl_seconds,
        },
    );
    let (news, prediction_markets) = future::join(news, prediction_markets).await;

    snapshot_from_reports(symbol, news, prediction_markets)
}

fn snapshot_from_reports(
    symbol: String,
    news: Result<SearchReport>,
    prediction_markets: Result<PredictionSearchReport>,
) -> ResearchContextSnapshot {
    let mut fetched_at_local = None;
    let mut errors = Vec::new();
    let news = match news {
        Ok(report) => {
            fetched_at_local = Some(report.fetched_at_local.clone());
            news_from_report(report)
        }
        Err(error) => {
            errors.push(format!("news: {error}"));
            Vec::new()
        }
    };
    let prediction_markets = match prediction_markets {
        Ok(report) => {
            fetched_at_local.get_or_insert(report.fetched_at_local.clone());
            prediction_markets_from_report(report)
        }
        Err(error) => {
            errors.push(format!("polymarket: {error}"));
            Vec::new()
        }
    };

    ResearchContextSnapshot {
        requested_symbol: symbol.clone(),
        symbol,
        fetched_at_local,
        news,
        prediction_markets,
        errors,
    }
}

fn news_from_report(report: SearchReport) -> Vec<ResearchNewsSnapshot> {
    report
        .highlights
        .into_iter()
        .filter(|highlight| highlight.module == "news")
        .map(news_from_highlight)
        .collect()
}

fn news_from_highlight(highlight: ResearchHighlight) -> ResearchNewsSnapshot {
    ResearchNewsSnapshot {
        title: format!("{}: {}", highlight.label, highlight.value),
        provider: highlight.provider,
        module: highlight.module,
    }
}

fn prediction_markets_from_report(report: PredictionSearchReport) -> Vec<PredictionMarketSnapshot> {
    report
        .markets
        .into_iter()
        .map(|market| PredictionMarketSnapshot {
            title: market.question.unwrap_or(market.title),
            probability: preferred_outcome_probability(&market.outcomes),
            volume: market.volume.or(market.volume_24hr),
            liquidity: market.liquidity,
            market_url: market.market_url,
        })
        .collect()
}

fn preferred_outcome_probability(outcomes: &[crate::model::PredictionOutcome]) -> Option<f64> {
    outcomes
        .iter()
        .find(|outcome| outcome.label.eq_ignore_ascii_case("yes"))
        .and_then(|outcome| outcome.implied_probability)
        .or_else(|| {
            outcomes
                .iter()
                .find_map(|outcome| outcome.implied_probability)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PredictionMarketSummary, PredictionOutcome};
    use serde_json::json;

    #[test]
    fn snapshot_from_reports_keeps_news_and_prediction_signals() {
        let snapshot = snapshot_from_reports(
            "CRDO".to_string(),
            Ok(SearchReport {
                category: "news".to_string(),
                query: "CRDO".to_string(),
                provider: "yahoo".to_string(),
                fetched_at_utc: "2026-06-25T00:00:00Z".to_string(),
                fetched_at_local: "2026-06-25 08:00:00".to_string(),
                cache_status: "fresh".to_string(),
                highlights: vec![ResearchHighlight::new(
                    "News Reuters",
                    "headline",
                    "yahoo",
                    "news",
                )],
                payload: json!({}),
            }),
            Ok(PredictionSearchReport {
                provider: "polymarket".to_string(),
                query: "CRDO".to_string(),
                fetched_at_utc: "2026-06-25T00:00:00Z".to_string(),
                fetched_at_local: "2026-06-25 08:00:01".to_string(),
                cache_status: "fresh".to_string(),
                source_urls: Vec::new(),
                interpretation_note: "prediction market".to_string(),
                markets: vec![prediction_market()],
                payload: json!({}),
            }),
        );

        assert_eq!(snapshot.news[0].title, "News Reuters: headline");
        assert_eq!(snapshot.prediction_markets[0].title, "Will CRDO rally?");
        assert_eq!(snapshot.prediction_markets[0].probability, Some(0.64));
        assert!(snapshot.errors.is_empty());
    }

    #[test]
    fn snapshot_from_reports_keeps_partial_data_when_one_source_fails() {
        let snapshot = snapshot_from_reports(
            "CRDO".to_string(),
            Err(anyhow::anyhow!("news timeout")),
            Ok(PredictionSearchReport {
                provider: "polymarket".to_string(),
                query: "CRDO".to_string(),
                fetched_at_utc: "2026-06-25T00:00:00Z".to_string(),
                fetched_at_local: "2026-06-25 08:00:01".to_string(),
                cache_status: "fresh".to_string(),
                source_urls: Vec::new(),
                interpretation_note: "prediction market".to_string(),
                markets: vec![prediction_market()],
                payload: json!({}),
            }),
        );

        assert!(snapshot.news.is_empty());
        assert_eq!(snapshot.prediction_markets.len(), 1);
        assert_eq!(snapshot.prediction_markets[0].probability, Some(0.64));
        assert_eq!(snapshot.errors, vec!["news: news timeout"]);
    }

    fn prediction_market() -> PredictionMarketSummary {
        PredictionMarketSummary {
            id: Some("1".to_string()),
            condition_id: None,
            slug: None,
            event_id: None,
            event_slug: None,
            title: "CRDO market".to_string(),
            question: Some("Will CRDO rally?".to_string()),
            active: Some(true),
            closed: Some(false),
            accepting_orders: Some(true),
            end_time_utc: None,
            end_time_local: None,
            volume: Some(1000.0),
            volume_24hr: None,
            liquidity: Some(500.0),
            open_interest: None,
            best_bid: None,
            best_ask: None,
            spread: None,
            last_trade_price: None,
            one_hour_price_change: None,
            one_day_price_change: None,
            one_week_price_change: None,
            market_url: Some("https://polymarket.com/event/crdo".to_string()),
            outcomes: vec![PredictionOutcome {
                label: "Yes".to_string(),
                implied_probability: Some(0.64),
                clob_token_id: None,
                best_bid: None,
                best_ask: None,
                spread: None,
                last_trade_price: None,
                bid_count: 0,
                ask_count: 0,
            }],
        }
    }
}
