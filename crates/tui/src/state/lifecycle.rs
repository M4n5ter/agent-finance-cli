use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::snapshot::MarketSnapshot;

use crate::account::AccountSnapshot;
use crate::task_failure::{TaskFailure, TaskFailureSource};
use crate::task_log::TaskKey;

use super::AppState;

impl AppState {
    pub(super) fn refresh_started(&mut self, generation: u64) {
        self.refresh.start(generation, ());
        self.task_log.running(
            TaskKey::Refresh { generation },
            "market snapshot refreshing",
        );
    }

    pub(super) fn snapshot_loaded(&mut self, generation: u64, snapshot: MarketSnapshot) {
        if let Some(active) = self.refresh.finish(generation) {
            self.task_failures.clear_global(TaskFailureSource::Quotes);
            if !snapshot.errors.is_empty() {
                self.task_log.warning(
                    TaskKey::Refresh {
                        generation: active.generation,
                    },
                    format!(
                        "refresh completed with {} provider errors",
                        snapshot.errors.len()
                    ),
                );
            } else {
                self.task_log.succeeded(
                    TaskKey::Refresh {
                        generation: active.generation,
                    },
                    "market snapshot refreshed",
                );
            }
            self.market_snapshot = Some(snapshot);
        } else {
            self.task_log.warning_event(format!(
                "ignored stale market snapshot generation {generation}",
            ));
        }
    }

    pub(super) fn refresh_failed(&mut self, generation: u64, error: String) {
        if let Some(active) = self.refresh.finish(generation) {
            self.task_failures.set(TaskFailure::market(error.clone()));
            self.task_log.failed(
                TaskKey::Refresh {
                    generation: active.generation,
                },
                format!("market refresh failed: {error}"),
            );
        }
    }

    pub(super) fn history_started(&mut self, generation: u64, symbol: String) {
        self.task_log.running(
            TaskKey::History {
                generation,
                symbol: symbol.clone(),
            },
            format!("{symbol} history loading"),
        );
        self.history.start(generation, symbol);
    }

    pub(super) fn history_loaded(&mut self, generation: u64, snapshot: HistorySnapshot) {
        if let Some(active) = self.history.finish(generation) {
            self.task_failures.clear_symbol(
                TaskFailureSource::History,
                snapshot.requested_symbol.as_str(),
                snapshot.symbol.as_str(),
            );
            if !snapshot.errors.is_empty() {
                self.task_log.warning(
                    TaskKey::History {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!(
                        "{} history loaded with {} warnings",
                        snapshot.symbol,
                        snapshot.errors.len()
                    ),
                );
            } else {
                self.task_log.succeeded(
                    TaskKey::History {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!("{} history loaded", snapshot.symbol),
                );
            }
            self.history.set_snapshot(snapshot);
        } else {
            self.task_log
                .warning_event(format!("ignored stale history generation {generation}",));
        }
    }

    pub(super) fn history_failed(&mut self, generation: u64, symbol: String, error: String) {
        if let Some(active) = self.history.finish(generation) {
            self.task_failures
                .set(TaskFailure::history(symbol.clone(), error.clone()));
            self.task_log.failed(
                TaskKey::History {
                    generation: active.generation,
                    symbol: active.key,
                },
                format!("{symbol} history failed: {error}"),
            );
        }
    }

    pub(super) fn evidence_started(&mut self, generation: u64, symbol: String) {
        self.task_log.running(
            TaskKey::Evidence {
                generation,
                symbol: symbol.clone(),
            },
            format!("{symbol} crypto evidence loading"),
        );
        self.evidence.start(generation, symbol);
    }

    pub(super) fn evidence_loaded(
        &mut self,
        generation: u64,
        snapshot: CryptoQuoteEvidenceSnapshot,
    ) {
        if let Some(active) = self.evidence.finish(generation) {
            self.task_failures.clear_symbol(
                TaskFailureSource::CryptoEvidence,
                snapshot.requested_symbol.as_str(),
                snapshot.symbol.as_str(),
            );
            if !snapshot.errors.is_empty() {
                self.task_log.warning(
                    TaskKey::Evidence {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!(
                        "{} crypto evidence loaded with {} warnings",
                        snapshot.symbol,
                        snapshot.errors.len()
                    ),
                );
            } else {
                self.task_log.succeeded(
                    TaskKey::Evidence {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!("{} crypto evidence loaded", snapshot.symbol),
                );
            }
            self.evidence.set_snapshot(snapshot);
        } else {
            self.task_log.warning_event(format!(
                "ignored stale crypto evidence generation {generation}",
            ));
        }
    }

    pub(super) fn evidence_failed(&mut self, generation: u64, symbol: String, error: String) {
        if let Some(active) = self.evidence.finish(generation) {
            self.task_failures
                .set(TaskFailure::evidence(symbol.clone(), error.clone()));
            self.task_log.failed(
                TaskKey::Evidence {
                    generation: active.generation,
                    symbol: active.key,
                },
                format!("{symbol} crypto evidence failed: {error}"),
            );
        }
    }

    pub(super) fn research_started(&mut self, generation: u64, symbol: String) {
        self.task_log.running(
            TaskKey::Research {
                generation,
                symbol: symbol.clone(),
            },
            format!("{symbol} research loading"),
        );
        self.research.start(generation, symbol);
    }

    pub(super) fn research_loaded(&mut self, generation: u64, snapshot: ResearchContextSnapshot) {
        if let Some(active) = self.research.finish(generation) {
            if !snapshot.errors.is_empty() {
                self.task_log.warning(
                    TaskKey::Research {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!(
                        "{} research loaded with {} warnings",
                        snapshot.symbol,
                        snapshot.errors.len()
                    ),
                );
            } else {
                self.task_log.succeeded(
                    TaskKey::Research {
                        generation: active.generation,
                        symbol: active.key.clone(),
                    },
                    format!("{} research context loaded", snapshot.symbol),
                );
            }
            self.research.set_snapshot(snapshot);
        } else {
            self.task_log
                .warning_event(format!("ignored stale research generation {generation}",));
        }
    }

    pub(super) fn account_started(&mut self, generation: u64, profile: String) {
        self.task_log.running(
            TaskKey::Account {
                generation,
                profile: profile.clone(),
            },
            format!("{profile} account snapshot loading"),
        );
        self.account.start(generation, profile);
    }

    pub(super) fn account_loaded(&mut self, generation: u64, snapshot: AccountSnapshot) {
        if let Some(active) = self.account.finish(generation) {
            self.task_failures
                .clear_profile(TaskFailureSource::Account, &active.key);
            if !snapshot.errors.is_empty() {
                self.task_log.warning(
                    TaskKey::Account {
                        generation: active.generation,
                        profile: active.key.clone(),
                    },
                    format!(
                        "{} account loaded with {} warnings",
                        snapshot.profile,
                        snapshot.errors.len()
                    ),
                );
            } else {
                self.task_log.succeeded(
                    TaskKey::Account {
                        generation: active.generation,
                        profile: active.key.clone(),
                    },
                    format!("{} account snapshot loaded", snapshot.profile),
                );
            }
            self.account_snapshot = Some(snapshot);
        } else {
            self.task_log
                .warning_event(format!("ignored stale account generation {generation}",));
        }
    }

    pub(super) fn account_failed(&mut self, generation: u64, profile: String, error: String) {
        if let Some(active) = self.account.finish(generation) {
            self.task_failures
                .set(TaskFailure::account(profile.clone(), error.clone()));
            self.task_log.failed(
                TaskKey::Account {
                    generation: active.generation,
                    profile: active.key,
                },
                format!("{profile} account snapshot failed: {error}"),
            );
        }
    }

    pub(super) fn scheduler_failed(&mut self, error: String) {
        if let Some(active) = self.refresh.cancel() {
            self.task_log.failed(
                TaskKey::Refresh {
                    generation: active.generation,
                },
                format!("market snapshot refresh cancelled: {error}"),
            );
        }
        if let Some(active) = self.history.cancel() {
            self.task_log.failed(
                TaskKey::History {
                    generation: active.generation,
                    symbol: active.key.clone(),
                },
                format!("{} history loading cancelled: {error}", active.key),
            );
        }
        if let Some(active) = self.evidence.cancel() {
            self.task_log.failed(
                TaskKey::Evidence {
                    generation: active.generation,
                    symbol: active.key.clone(),
                },
                format!("{} crypto evidence loading cancelled: {error}", active.key),
            );
        }
        if let Some(active) = self.research.cancel() {
            self.task_log.failed(
                TaskKey::Research {
                    generation: active.generation,
                    symbol: active.key.clone(),
                },
                format!("{} research loading cancelled: {error}", active.key),
            );
        }
        if let Some(active) = self.account.cancel() {
            self.task_log.failed(
                TaskKey::Account {
                    generation: active.generation,
                    profile: active.key.clone(),
                },
                format!("{} account loading cancelled: {error}", active.key),
            );
        }
        self.scheduler_error = Some(error.clone());
        self.task_failures
            .set(TaskFailure::scheduler(error.clone()));
        self.task_log
            .failed(TaskKey::Scheduler, format!("scheduler failed: {error}"));
    }
}
