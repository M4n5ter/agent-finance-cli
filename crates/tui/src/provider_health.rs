use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::state::AppState;
use crate::task_failure::TaskFailureSource;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ProviderHealthReport {
    pub providers: Vec<ProviderHealthProvider>,
    pub tasks: Vec<ProviderHealthTask>,
}

impl ProviderHealthReport {
    pub fn from_state(state: &AppState) -> Self {
        let mut builder = ProviderHealthBuilder::default();
        builder.add_market_snapshot(state);
        builder.add_history_snapshot(state);
        builder.add_crypto_evidence(state);
        builder.add_research_snapshot(state);
        builder.add_task_failures(state);
        builder.add_loading_tasks(state);
        builder.finish()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty() && self.tasks.is_empty()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ProviderHealthProvider {
    pub provider: String,
    pub severity: ProviderHealthSeverity,
    pub signals: Vec<ProviderHealthSignal>,
    pub freshness: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ProviderHealthSignal {
    pub source: ProviderHealthSource,
    pub status: ProviderHealthSeverity,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ProviderHealthTask {
    pub source: ProviderHealthSource,
    pub status: ProviderHealthSeverity,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderHealthSeverity {
    Ok,
    Loading,
    Warning,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderHealthSource {
    Quotes,
    History,
    CryptoEvidence,
    Account,
    News,
    PredictionMarkets,
    Scheduler,
}

impl ProviderHealthSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Quotes => "quotes",
            Self::History => "history",
            Self::CryptoEvidence => "crypto",
            Self::Account => "account",
            Self::News => "news",
            Self::PredictionMarkets => "predictions",
            Self::Scheduler => "scheduler",
        }
    }
}

#[derive(Debug, Default)]
struct ProviderHealthBuilder {
    providers: BTreeMap<String, ProviderHealthProvider>,
    tasks: Vec<ProviderHealthTask>,
}

impl ProviderHealthBuilder {
    fn add_market_snapshot(&mut self, state: &AppState) {
        let Some(snapshot) = state.market_snapshot.as_ref() else {
            return;
        };

        let known_providers = state
            .provider_profiles
            .iter()
            .map(|profile| profile.provider.as_str())
            .collect::<BTreeSet<_>>();
        let mut priced_by_provider = BTreeMap::<String, usize>::new();
        let mut unavailable = 0usize;
        for quote in &snapshot.quotes {
            if quote.price.is_some() {
                *priced_by_provider
                    .entry(quote.provider.clone())
                    .or_default() += 1;
            } else {
                unavailable += 1;
            }
        }
        for (provider, count) in priced_by_provider {
            self.provider_signal(
                provider,
                snapshot.fetched_at_local.clone(),
                ProviderHealthSignal {
                    source: ProviderHealthSource::Quotes,
                    status: ProviderHealthSeverity::Ok,
                    detail: format!("{count} priced quotes"),
                },
            );
        }
        if unavailable > 0 {
            self.task_warning(
                ProviderHealthSource::Quotes,
                format!("{unavailable} quotes returned without price"),
            );
        }
        for error in &snapshot.errors {
            if let Some(provider) = provider_from_quote_error(error, &known_providers) {
                self.provider_signal(
                    provider,
                    snapshot.fetched_at_local.clone(),
                    ProviderHealthSignal {
                        source: ProviderHealthSource::Quotes,
                        status: ProviderHealthSeverity::Warning,
                        detail: error.clone(),
                    },
                );
            } else {
                self.task_warning(ProviderHealthSource::Quotes, error.clone());
            }
        }
    }

    fn add_history_snapshot(&mut self, state: &AppState) {
        let Some(symbol) = state.selected_symbol() else {
            return;
        };
        let Some(snapshot) = state.history.selected_snapshot(symbol) else {
            return;
        };

        let status = if snapshot.errors.is_empty() {
            ProviderHealthSeverity::Ok
        } else {
            ProviderHealthSeverity::Warning
        };
        let detail = if snapshot.errors.is_empty() {
            format!(
                "{} {} bars interval={}",
                snapshot.symbol,
                snapshot.bars.len(),
                snapshot.interval
            )
        } else {
            format!(
                "{} {} bars warnings={}",
                snapshot.symbol,
                snapshot.bars.len(),
                snapshot.errors.len()
            )
        };
        self.provider_signal(
            snapshot.provider.clone(),
            snapshot.fetched_at_local.clone(),
            ProviderHealthSignal {
                source: ProviderHealthSource::History,
                status,
                detail,
            },
        );
    }

    fn add_crypto_evidence(&mut self, state: &AppState) {
        let Some(symbol) = state.selected_symbol() else {
            return;
        };
        let Some(snapshot) = state.evidence.selected_snapshot(symbol) else {
            return;
        };

        for provider in &snapshot.providers {
            let status = if provider.ok {
                ProviderHealthSeverity::Ok
            } else {
                ProviderHealthSeverity::Warning
            };
            let detail = provider.first_error.clone().unwrap_or_else(|| {
                format!(
                    "{}/{} endpoints, {} required failed",
                    provider.ok_endpoints, provider.total_endpoints, provider.required_failed
                )
            });
            self.provider_signal(
                provider.provider.clone(),
                snapshot.fetched_at_local.clone(),
                ProviderHealthSignal {
                    source: ProviderHealthSource::CryptoEvidence,
                    status,
                    detail,
                },
            );
        }
        for error in &snapshot.errors {
            self.task_warning(ProviderHealthSource::CryptoEvidence, error.clone());
        }
    }

    fn add_research_snapshot(&mut self, state: &AppState) {
        let Some(symbol) = state.selected_symbol() else {
            return;
        };
        let Some(snapshot) = state.research.selected_snapshot(symbol) else {
            return;
        };

        let mut news_by_provider = BTreeMap::<String, usize>::new();
        for item in &snapshot.news {
            *news_by_provider.entry(item.provider.clone()).or_default() += 1;
        }
        for (provider, count) in news_by_provider {
            self.provider_signal(
                provider,
                snapshot.fetched_at_local.clone(),
                ProviderHealthSignal {
                    source: ProviderHealthSource::News,
                    status: ProviderHealthSeverity::Ok,
                    detail: format!("{count} headlines"),
                },
            );
        }
        if !snapshot.prediction_markets.is_empty() {
            self.provider_signal(
                "polymarket",
                snapshot.fetched_at_local.clone(),
                ProviderHealthSignal {
                    source: ProviderHealthSource::PredictionMarkets,
                    status: ProviderHealthSeverity::Ok,
                    detail: format!("{} markets", snapshot.prediction_markets.len()),
                },
            );
        }
        for error in &snapshot.errors {
            self.task_warning(research_error_source(error), error.clone());
        }
    }

    fn add_task_failures(&mut self, state: &AppState) {
        let selected_symbol = state.selected_symbol();
        for failure in state.task_failures.iter() {
            if !failure.scope.selected_symbol_matches(selected_symbol) {
                continue;
            }
            self.task_warning(failure_source(failure.source), failure.error.clone());
        }
        if let Some(account) = state.account_snapshot.as_ref() {
            for error in &account.errors {
                self.task_warning(
                    ProviderHealthSource::Account,
                    format!("{}: {}", error.label, error.error),
                );
            }
        }
    }

    fn add_loading_tasks(&mut self, state: &AppState) {
        if state.refresh_loading() {
            self.task_loading(ProviderHealthSource::Quotes, "refresh in flight");
        }
        if state.history.loading() {
            self.task_loading(ProviderHealthSource::History, "load in flight");
        }
        if state.evidence.loading() {
            self.task_loading(ProviderHealthSource::CryptoEvidence, "load in flight");
        }
        if state.research.loading() {
            self.task_loading(ProviderHealthSource::News, "research load in flight");
            self.task_loading(
                ProviderHealthSource::PredictionMarkets,
                "research load in flight",
            );
        }
        if state.account_loading() {
            self.task_loading(ProviderHealthSource::Account, "account load in flight");
        }
    }

    fn provider_signal(
        &mut self,
        provider: impl Into<String>,
        freshness: Option<String>,
        signal: ProviderHealthSignal,
    ) {
        let provider = provider.into();
        let entry =
            self.providers
                .entry(provider.clone())
                .or_insert_with(|| ProviderHealthProvider {
                    provider,
                    severity: ProviderHealthSeverity::Ok,
                    signals: Vec::new(),
                    freshness: None,
                });
        entry.severity = entry.severity.max(signal.status);
        entry.freshness = freshness.or_else(|| entry.freshness.clone());
        entry.signals.push(signal);
    }

    fn task_warning(&mut self, source: ProviderHealthSource, detail: impl Into<String>) {
        self.tasks.push(ProviderHealthTask {
            source,
            status: ProviderHealthSeverity::Warning,
            detail: detail.into(),
        });
    }

    fn task_loading(&mut self, source: ProviderHealthSource, detail: impl Into<String>) {
        self.tasks.push(ProviderHealthTask {
            source,
            status: ProviderHealthSeverity::Loading,
            detail: detail.into(),
        });
    }

    fn finish(self) -> ProviderHealthReport {
        ProviderHealthReport {
            providers: self.providers.into_values().collect(),
            tasks: self.tasks,
        }
    }
}

fn provider_from_quote_error(error: &str, known_providers: &BTreeSet<&str>) -> Option<String> {
    let (prefix, _) = error.split_once(':')?;
    let exact = prefix.trim();
    if known_providers.contains(exact) {
        return Some(exact.to_string());
    }

    let last_token = exact.split_whitespace().last()?;
    known_providers
        .contains(last_token)
        .then(|| last_token.to_string())
}

fn research_error_source(error: &str) -> ProviderHealthSource {
    if error.trim_start().starts_with("polymarket:") {
        ProviderHealthSource::PredictionMarkets
    } else {
        ProviderHealthSource::News
    }
}

fn failure_source(source: TaskFailureSource) -> ProviderHealthSource {
    match source {
        TaskFailureSource::Quotes => ProviderHealthSource::Quotes,
        TaskFailureSource::History => ProviderHealthSource::History,
        TaskFailureSource::CryptoEvidence => ProviderHealthSource::CryptoEvidence,
        TaskFailureSource::Account => ProviderHealthSource::Account,
        TaskFailureSource::Scheduler => ProviderHealthSource::Scheduler,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TuiConfig;
    use crate::state::Action;
    use agent_finance_market::crypto_evidence_snapshot::{
        CryptoQuoteEvidenceSnapshot, ProviderQuoteEvidenceSnapshot,
    };
    use agent_finance_market::history_snapshot::HistorySnapshot;
    use agent_finance_market::research_snapshot::ResearchContextSnapshot;
    use agent_finance_market::snapshot::{MarketSnapshot, QuoteSnapshot, RegularBasisSnapshot};

    #[test]
    fn report_merges_provider_signals_without_mixing_task_identity() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["BTCUSDT".to_string()],
            ..TuiConfig::default()
        });

        load_quote_snapshot(
            &mut state,
            1,
            quote_snapshot("BTCUSDT", Some(250.0), "test"),
        );
        load_history_snapshot(&mut state, 2, history_snapshot("BTCUSDT", "test"));
        load_evidence_snapshot(&mut state, 3, evidence_snapshot("BTCUSDT", "binance", true));
        load_research_snapshot(&mut state, 4, research_snapshot("BTCUSDT", 2, 1));

        let report = ProviderHealthReport::from_state(&state);

        assert_eq!(report.tasks, []);
        assert_eq!(
            provider(&report, "test").signals,
            vec![
                ProviderHealthSignal {
                    source: ProviderHealthSource::Quotes,
                    status: ProviderHealthSeverity::Ok,
                    detail: "1 priced quotes".to_string(),
                },
                ProviderHealthSignal {
                    source: ProviderHealthSource::History,
                    status: ProviderHealthSeverity::Ok,
                    detail: "BTCUSDT 0 bars interval=1d".to_string(),
                },
                ProviderHealthSignal {
                    source: ProviderHealthSource::News,
                    status: ProviderHealthSeverity::Ok,
                    detail: "2 headlines".to_string(),
                },
            ]
        );
        assert_eq!(
            provider(&report, "binance").severity,
            ProviderHealthSeverity::Ok
        );
        assert_eq!(
            provider(&report, "polymarket").signals[0].source,
            ProviderHealthSource::PredictionMarkets
        );
    }

    #[test]
    fn report_keeps_last_provider_state_while_showing_new_failures_and_loading() {
        let mut state = AppState::from_config(TuiConfig::default());

        load_quote_snapshot(&mut state, 1, quote_snapshot("AAPL", Some(250.0), "yahoo"));
        state.reduce(Action::RefreshStarted(1));
        state.reduce(Action::RefreshFailed {
            generation: 1,
            error: "yahoo: timeout".to_string(),
        });
        state.reduce(Action::HistoryStarted {
            generation: 2,
            symbol: "AAPL".to_string(),
        });

        let report = ProviderHealthReport::from_state(&state);

        assert_eq!(
            provider(&report, "yahoo").severity,
            ProviderHealthSeverity::Ok
        );
        assert!(report.tasks.iter().any(|task| {
            task.source == ProviderHealthSource::Quotes
                && task.status == ProviderHealthSeverity::Warning
                && task.detail == "yahoo: timeout"
        }));
        assert!(report.tasks.iter().any(|task| {
            task.source == ProviderHealthSource::History
                && task.status == ProviderHealthSeverity::Loading
        }));
    }

    #[test]
    fn report_treats_missing_price_quotes_as_task_warnings_not_ok_providers() {
        let mut state = AppState::from_config(TuiConfig::default());

        load_quote_snapshot(&mut state, 1, quote_snapshot("AAPL", None, "unavailable"));

        let report = ProviderHealthReport::from_state(&state);

        assert!(report.providers.is_empty());
        assert_eq!(
            report.tasks,
            vec![ProviderHealthTask {
                source: ProviderHealthSource::Quotes,
                status: ProviderHealthSeverity::Warning,
                detail: "1 quotes returned without price".to_string(),
            }]
        );
    }

    #[test]
    fn report_clears_transient_failures_after_current_success() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::RefreshStarted(1));
        state.reduce(Action::RefreshFailed {
            generation: 1,
            error: "yahoo: timeout".to_string(),
        });
        state.reduce(Action::RefreshStarted(2));
        state.reduce(Action::SnapshotLoaded {
            generation: 2,
            snapshot: quote_snapshot("AAPL", Some(250.0), "yahoo"),
        });

        let report = ProviderHealthReport::from_state(&state);

        assert_eq!(
            provider(&report, "yahoo").severity,
            ProviderHealthSeverity::Ok
        );
        assert!(
            report
                .tasks
                .iter()
                .all(|task| task.source != ProviderHealthSource::Quotes)
        );
    }

    #[test]
    fn report_keeps_quote_category_errors_out_of_provider_identity() {
        let mut state = AppState::from_config(TuiConfig::default());
        let mut snapshot = quote_snapshot("AAPL", Some(250.0), "yahoo");
        snapshot.errors = vec![
            "equity price refresh failed: timeout".to_string(),
            "AAPL yahoo: unavailable".to_string(),
        ];

        load_quote_snapshot(&mut state, 1, snapshot);

        let report = ProviderHealthReport::from_state(&state);

        assert!(report.providers.iter().all(|row| {
            row.provider != "equity price refresh failed" && row.provider != "AAPL yahoo"
        }));
        assert_eq!(
            provider(&report, "yahoo").severity,
            ProviderHealthSeverity::Warning
        );
        assert!(report.tasks.iter().any(|task| {
            task.source == ProviderHealthSource::Quotes
                && task.detail == "equity price refresh failed: timeout"
        }));
    }

    #[test]
    fn report_maps_polymarket_errors_to_prediction_market_tasks() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            ..TuiConfig::default()
        });
        let mut snapshot = research_snapshot("CRDO", 0, 0);
        snapshot.errors = vec![
            "news: rate limited".to_string(),
            "polymarket: unavailable".to_string(),
        ];

        load_research_snapshot(&mut state, 1, snapshot);

        let report = ProviderHealthReport::from_state(&state);

        assert!(report.tasks.iter().any(|task| {
            task.source == ProviderHealthSource::News && task.detail == "news: rate limited"
        }));
        assert!(report.tasks.iter().any(|task| {
            task.source == ProviderHealthSource::PredictionMarkets
                && task.detail == "polymarket: unavailable"
        }));
    }

    #[test]
    fn report_clears_symbol_failures_after_alias_normalized_success() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["BTC/USDT".to_string()],
            ..TuiConfig::default()
        });
        let mut history = history_snapshot("BTCUSDT", "binance");
        history.requested_symbol = "BTC/USDT".to_string();
        let mut evidence = evidence_snapshot("BTCUSDT", "binance", true);
        evidence.requested_symbol = "BTC/USDT".to_string();

        state.reduce(Action::HistoryStarted {
            generation: 1,
            symbol: "BTC/USDT".to_string(),
        });
        state.reduce(Action::HistoryFailed {
            generation: 1,
            symbol: "BTC/USDT".to_string(),
            error: "timeout".to_string(),
        });
        state.reduce(Action::HistoryStarted {
            generation: 2,
            symbol: "BTC/USDT".to_string(),
        });
        state.reduce(Action::HistoryLoaded {
            generation: 2,
            snapshot: history,
        });
        state.reduce(Action::EvidenceStarted {
            generation: 3,
            symbol: "BTC/USDT".to_string(),
        });
        state.reduce(Action::EvidenceFailed {
            generation: 3,
            symbol: "BTC/USDT".to_string(),
            error: "timeout".to_string(),
        });
        state.reduce(Action::EvidenceStarted {
            generation: 4,
            symbol: "BTC/USDT".to_string(),
        });
        state.reduce(Action::EvidenceLoaded {
            generation: 4,
            snapshot: evidence,
        });

        let report = ProviderHealthReport::from_state(&state);

        assert!(report.tasks.iter().all(|task| {
            task.source != ProviderHealthSource::History
                && task.source != ProviderHealthSource::CryptoEvidence
        }));
    }

    fn provider<'a>(
        report: &'a ProviderHealthReport,
        provider: &str,
    ) -> &'a ProviderHealthProvider {
        report
            .providers
            .iter()
            .find(|row| row.provider == provider)
            .expect("provider should exist")
    }

    fn load_quote_snapshot(state: &mut AppState, generation: u64, snapshot: MarketSnapshot) {
        state.reduce(Action::RefreshStarted(generation));
        state.reduce(Action::SnapshotLoaded {
            generation,
            snapshot,
        });
    }

    fn load_history_snapshot(state: &mut AppState, generation: u64, snapshot: HistorySnapshot) {
        let symbol = snapshot.requested_symbol.clone();
        state.reduce(Action::HistoryStarted { generation, symbol });
        state.reduce(Action::HistoryLoaded {
            generation,
            snapshot,
        });
    }

    fn load_evidence_snapshot(
        state: &mut AppState,
        generation: u64,
        snapshot: CryptoQuoteEvidenceSnapshot,
    ) {
        let symbol = snapshot.requested_symbol.clone();
        state.reduce(Action::EvidenceStarted { generation, symbol });
        state.reduce(Action::EvidenceLoaded {
            generation,
            snapshot,
        });
    }

    fn load_research_snapshot(
        state: &mut AppState,
        generation: u64,
        snapshot: ResearchContextSnapshot,
    ) {
        let symbol = snapshot.requested_symbol.clone();
        state.reduce(Action::ResearchStarted { generation, symbol });
        state.reduce(Action::ResearchLoaded {
            generation,
            snapshot,
        });
    }

    fn quote_snapshot(symbol: &str, price: Option<f64>, provider: &str) -> MarketSnapshot {
        MarketSnapshot {
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            quotes: vec![QuoteSnapshot {
                symbol: symbol.to_string(),
                price,
                currency: Some("USD".to_string()),
                provider: provider.to_string(),
                session: Some("regular".to_string()),
                market_time_local: None,
                change_pct: Some(1.0),
                aliases: Vec::new(),
                regular_basis: RegularBasisSnapshot {
                    previous_close: Some(247.0),
                    open: None,
                    high: None,
                    low: None,
                    volume: None,
                },
            }],
            errors: Vec::new(),
        }
    }

    fn history_snapshot(symbol: &str, provider: &str) -> HistorySnapshot {
        HistorySnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            provider: provider.to_string(),
            interval: "1d".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            latest_close: Some(250.0),
            latest_time: Some("2026-06-25".to_string()),
            return_pct: Some(1.0),
            volume: Some(10_000.0),
            bars: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn evidence_snapshot(symbol: &str, provider: &str, ok: bool) -> CryptoQuoteEvidenceSnapshot {
        CryptoQuoteEvidenceSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            instrument: "spot".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            ok_providers: usize::from(ok),
            total_providers: 1,
            providers: vec![ProviderQuoteEvidenceSnapshot {
                provider: provider.to_string(),
                ok,
                ok_endpoints: usize::from(ok),
                total_endpoints: 1,
                required_failed: usize::from(!ok),
                first_error: (!ok).then(|| "timeout".to_string()),
                endpoints: Vec::new(),
            }],
            errors: Vec::new(),
        }
    }

    fn research_snapshot(
        symbol: &str,
        news_count: usize,
        prediction_count: usize,
    ) -> ResearchContextSnapshot {
        ResearchContextSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            news: (0..news_count)
                .map(
                    |index| agent_finance_market::research_snapshot::ResearchNewsSnapshot {
                        title: format!("headline {index}"),
                        provider: "test".to_string(),
                        module: "news".to_string(),
                    },
                )
                .collect(),
            prediction_markets: (0..prediction_count)
                .map(
                    |index| agent_finance_market::research_snapshot::PredictionMarketSnapshot {
                        title: format!("market {index}"),
                        probability: Some(0.5),
                        volume: Some(1000.0),
                        liquidity: None,
                        market_url: None,
                    },
                )
                .collect(),
            errors: Vec::new(),
        }
    }
}
