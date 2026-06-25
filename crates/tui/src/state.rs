use std::collections::VecDeque;

use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::model::ProviderProfile;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::service;
use agent_finance_market::snapshot::MarketSnapshot;

use crate::command::{CommandEffect, CommandPaletteState};
use crate::config::{LayoutConfig, TuiConfig};
use crate::model::{DockedPanels, FloatingKind, FloatingPane, Panel, TaskLogEntry};

#[derive(Debug, Clone)]
pub struct AppState {
    pub watchlist: Vec<String>,
    pub selected_symbol: usize,
    pub layout: LayoutConfig,
    pub panels: DockedPanels,
    pub floating: Vec<FloatingPane>,
    pub command_palette: CommandPaletteState,
    pub task_log: VecDeque<TaskLogEntry>,
    pub provider_profiles: Vec<ProviderProfile>,
    pub market_snapshot: Option<MarketSnapshot>,
    pub refresh: LoadSlot<()>,
    pub history: SelectedSymbolLoad<HistorySnapshot>,
    pub evidence: SelectedSymbolLoad<CryptoQuoteEvidenceSnapshot>,
    pub research: SelectedSymbolLoad<ResearchContextSnapshot>,
    pub scheduler_error: Option<String>,
}

impl AppState {
    pub fn from_config(config: TuiConfig) -> Self {
        Self {
            watchlist: config.watchlist,
            selected_symbol: 0,
            layout: config.layout,
            panels: DockedPanels::default(),
            floating: Vec::new(),
            command_palette: CommandPaletteState::default(),
            task_log: VecDeque::new(),
            provider_profiles: service::provider_profiles(),
            market_snapshot: None,
            refresh: LoadSlot::new(),
            history: SelectedSymbolLoad::new(),
            evidence: SelectedSymbolLoad::new(),
            research: SelectedSymbolLoad::new(),
            scheduler_error: None,
        }
    }

    pub fn selected_symbol(&self) -> Option<&str> {
        self.watchlist.get(self.selected_symbol).map(String::as_str)
    }

    pub fn reduce(&mut self, action: Action) {
        match action {
            Action::Focus(panel) => {
                self.panels.focus(panel);
            }
            Action::SelectNextSymbol => self.shift_symbol(1),
            Action::SelectPreviousSymbol => self.shift_symbol(-1),
            Action::ToggleFloating(kind) => self.toggle_floating(kind),
            Action::MoveCommandSelection(direction) => {
                self.command_palette.shift(direction);
            }
            Action::ApplyCommand(effect) => self.apply_command(effect),
            Action::CloseFocusedPanel => self.panels.close_focused(),
            Action::RestorePanels => self.panels.restore(),
            Action::CloseFocusedFloating => {
                self.floating.pop();
            }
            Action::ResetLayout => {
                self.floating.clear();
                self.layout = LayoutConfig::default();
                self.panels = DockedPanels::default();
            }
            Action::ResizeDockedColumns {
                left_ratio,
                main_ratio,
            } => {
                self.layout.left_ratio = left_ratio;
                self.layout.main_ratio = main_ratio;
                self.layout.normalize();
            }
            Action::RefreshStarted(generation) => {
                self.refresh.start(generation, ());
            }
            Action::SnapshotLoaded {
                generation,
                snapshot,
            } => {
                if self.refresh.finish(generation) {
                    if !snapshot.errors.is_empty() {
                        self.push_log(TaskLogEntry::warning(format!(
                            "refresh completed with {} provider errors",
                            snapshot.errors.len()
                        )));
                    } else {
                        self.push_log(TaskLogEntry::info("market snapshot refreshed".to_string()));
                    }
                    self.market_snapshot = Some(snapshot);
                } else {
                    self.push_log(TaskLogEntry::warning(format!(
                        "ignored stale market snapshot generation {generation}",
                    )));
                }
            }
            Action::RefreshFailed { generation, error } => {
                if self.refresh.finish(generation) {
                    self.push_log(TaskLogEntry::warning(format!(
                        "market refresh failed: {error}"
                    )));
                }
            }
            Action::HistoryStarted { generation, symbol } => {
                self.history.start(generation, symbol);
            }
            Action::HistoryLoaded {
                generation,
                snapshot,
            } => {
                if self.history.finish(generation) {
                    if !snapshot.errors.is_empty() {
                        self.push_log(TaskLogEntry::warning(format!(
                            "history loaded with {} warnings",
                            snapshot.errors.len()
                        )));
                    } else {
                        self.push_log(TaskLogEntry::info(format!(
                            "{} history loaded",
                            snapshot.symbol
                        )));
                    }
                    self.history.set_snapshot(snapshot);
                } else {
                    self.push_log(TaskLogEntry::warning(format!(
                        "ignored stale history generation {generation}",
                    )));
                }
            }
            Action::HistoryFailed {
                generation,
                symbol,
                error,
            } => {
                if self.history.finish(generation) {
                    self.push_log(TaskLogEntry::warning(format!(
                        "{symbol} history failed: {error}"
                    )));
                }
            }
            Action::EvidenceStarted { generation, symbol } => {
                self.evidence.start(generation, symbol);
            }
            Action::EvidenceLoaded {
                generation,
                snapshot,
            } => {
                if self.evidence.finish(generation) {
                    if !snapshot.errors.is_empty() {
                        self.push_log(TaskLogEntry::warning(format!(
                            "crypto evidence loaded with {} warnings",
                            snapshot.errors.len()
                        )));
                    } else {
                        self.push_log(TaskLogEntry::info(format!(
                            "{} crypto evidence loaded",
                            snapshot.symbol
                        )));
                    }
                    self.evidence.set_snapshot(snapshot);
                } else {
                    self.push_log(TaskLogEntry::warning(format!(
                        "ignored stale crypto evidence generation {generation}",
                    )));
                }
            }
            Action::EvidenceFailed {
                generation,
                symbol,
                error,
            } => {
                if self.evidence.finish(generation) {
                    self.push_log(TaskLogEntry::warning(format!(
                        "{symbol} crypto evidence failed: {error}"
                    )));
                }
            }
            Action::ResearchStarted { generation, symbol } => {
                self.research.start(generation, symbol);
            }
            Action::ResearchLoaded {
                generation,
                snapshot,
            } => {
                if self.research.finish(generation) {
                    if !snapshot.errors.is_empty() {
                        self.push_log(TaskLogEntry::warning(format!(
                            "research loaded with {} warnings",
                            snapshot.errors.len()
                        )));
                    } else {
                        self.push_log(TaskLogEntry::info(format!(
                            "{} research context loaded",
                            snapshot.symbol
                        )));
                    }
                    self.research.set_snapshot(snapshot);
                } else {
                    self.push_log(TaskLogEntry::warning(format!(
                        "ignored stale research generation {generation}",
                    )));
                }
            }
            Action::SchedulerFailed(error) => {
                self.refresh.stop();
                self.history.stop();
                self.evidence.stop();
                self.research.stop();
                self.scheduler_error = Some(error.clone());
                self.push_log(TaskLogEntry::warning(format!("scheduler failed: {error}")));
            }
            Action::Log(message) => self.push_log(TaskLogEntry::info(message)),
        }
    }

    fn shift_symbol(&mut self, direction: isize) {
        if self.watchlist.is_empty() {
            self.selected_symbol = 0;
            return;
        }

        let len = self.watchlist.len() as isize;
        let selected = self.selected_symbol as isize;
        self.selected_symbol = (selected + direction).rem_euclid(len) as usize;
    }

    fn toggle_floating(&mut self, kind: FloatingKind) {
        if let Some(index) = self.floating.iter().position(|pane| pane.kind == kind) {
            self.floating.remove(index);
            return;
        }

        self.open_floating(kind);
    }

    fn close_floating(&mut self, kind: FloatingKind) {
        self.floating.retain(|pane| pane.kind != kind);
    }

    fn open_floating(&mut self, kind: FloatingKind) {
        self.close_floating(kind);
        let next_z = self.next_floating_z_index();
        self.floating.push(FloatingPane {
            kind,
            z_index: next_z,
        });
    }

    fn next_floating_z_index(&self) -> u16 {
        self.floating
            .iter()
            .map(|pane| pane.z_index)
            .max()
            .unwrap_or(0)
            + 1
    }

    fn apply_command(&mut self, effect: CommandEffect) {
        match effect {
            CommandEffect::OpenFloating(kind) => {
                self.close_floating(FloatingKind::CommandPalette);
                self.open_floating(kind);
            }
            CommandEffect::ResetLayout => {
                self.reduce(Action::ResetLayout);
            }
            CommandEffect::FocusPanel(panel) => {
                self.close_floating(FloatingKind::CommandPalette);
                self.reduce(Action::Focus(panel));
            }
            CommandEffect::TogglePanel(panel) => {
                self.close_floating(FloatingKind::CommandPalette);
                self.panels.toggle(panel);
            }
            CommandEffect::CloseFocusedPanel => {
                self.close_floating(FloatingKind::CommandPalette);
                self.reduce(Action::CloseFocusedPanel);
            }
            CommandEffect::RestorePanels => {
                self.close_floating(FloatingKind::CommandPalette);
                self.reduce(Action::RestorePanels);
            }
            CommandEffect::CloseCommandPalette => {
                self.close_floating(FloatingKind::CommandPalette);
            }
        }
    }

    fn push_log(&mut self, entry: TaskLogEntry) {
        self.task_log.push_back(entry);
        while self.task_log.len() > 200 {
            self.task_log.pop_front();
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LoadSlot<K> {
    pub generation: u64,
    pub loading: bool,
    pub key: Option<K>,
}

impl<K> LoadSlot<K> {
    fn new() -> Self {
        Self {
            generation: 0,
            loading: false,
            key: None,
        }
    }

    fn start(&mut self, generation: u64, key: K) {
        self.generation = generation;
        self.loading = true;
        self.key = Some(key);
    }

    fn finish(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading = false;
        true
    }

    fn stop(&mut self) {
        self.loading = false;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedSymbolLoad<T> {
    snapshot: Option<T>,
    request: LoadSlot<String>,
}

impl<T> SelectedSymbolLoad<T> {
    fn new() -> Self {
        Self {
            snapshot: None,
            request: LoadSlot::new(),
        }
    }

    pub fn loading(&self) -> bool {
        self.request.loading
    }

    fn start(&mut self, generation: u64, symbol: String) {
        self.request.start(generation, symbol);
    }

    fn finish(&mut self, generation: u64) -> bool {
        self.request.finish(generation)
    }

    fn set_snapshot(&mut self, snapshot: T) {
        self.snapshot = Some(snapshot);
    }

    fn stop(&mut self) {
        self.request.stop();
    }
}

pub trait SymbolSnapshot {
    fn requested_symbol(&self) -> &str;
    fn symbol(&self) -> &str;
}

impl SymbolSnapshot for HistorySnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl SymbolSnapshot for CryptoQuoteEvidenceSnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl SymbolSnapshot for ResearchContextSnapshot {
    fn requested_symbol(&self) -> &str {
        &self.requested_symbol
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl<T: SymbolSnapshot> SelectedSymbolLoad<T> {
    pub fn has_selected_snapshot(&self, selected: &str) -> bool {
        self.selected_snapshot(selected).is_some()
    }

    pub fn selected_snapshot(&self, selected: &str) -> Option<&T> {
        self.snapshot.as_ref().filter(|snapshot| {
            snapshot.requested_symbol() == selected || snapshot.symbol() == selected
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Focus(Panel),
    SelectNextSymbol,
    SelectPreviousSymbol,
    ToggleFloating(FloatingKind),
    MoveCommandSelection(isize),
    ApplyCommand(CommandEffect),
    CloseFocusedPanel,
    RestorePanels,
    CloseFocusedFloating,
    ResetLayout,
    ResizeDockedColumns {
        left_ratio: u16,
        main_ratio: u16,
    },
    RefreshStarted(u64),
    SnapshotLoaded {
        generation: u64,
        snapshot: MarketSnapshot,
    },
    RefreshFailed {
        generation: u64,
        error: String,
    },
    HistoryStarted {
        generation: u64,
        symbol: String,
    },
    HistoryLoaded {
        generation: u64,
        snapshot: HistorySnapshot,
    },
    HistoryFailed {
        generation: u64,
        symbol: String,
        error: String,
    },
    EvidenceStarted {
        generation: u64,
        symbol: String,
    },
    EvidenceLoaded {
        generation: u64,
        snapshot: CryptoQuoteEvidenceSnapshot,
    },
    EvidenceFailed {
        generation: u64,
        symbol: String,
        error: String,
    },
    ResearchStarted {
        generation: u64,
        symbol: String,
    },
    ResearchLoaded {
        generation: u64,
        snapshot: ResearchContextSnapshot,
    },
    SchedulerFailed(String),
    Log(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandEffect;
    use crate::config::MAX_LEFT_MAIN_RATIO;
    use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
    use agent_finance_market::history_snapshot::HistorySnapshot;
    use agent_finance_market::research_snapshot::ResearchContextSnapshot;
    use agent_finance_market::snapshot::{QuoteSnapshot, RegularBasisSnapshot};

    #[test]
    fn reducer_wraps_symbol_focus_across_watchlist_boundaries() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
            ..TuiConfig::default()
        });

        state.reduce(Action::SelectPreviousSymbol);

        assert_eq!(state.selected_symbol(), Some("CRDO"));

        state.reduce(Action::SelectNextSymbol);

        assert_eq!(state.selected_symbol(), Some("AAPL"));
    }

    #[test]
    fn floating_panes_keep_newest_overlay_on_top() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ToggleFloating(FloatingKind::Help));
        state.reduce(Action::ToggleFloating(FloatingKind::CommandPalette));

        assert_eq!(state.floating[0].z_index, 1);
        assert_eq!(state.floating[1].z_index, 2);

        state.reduce(Action::CloseFocusedFloating);

        assert_eq!(state.floating.len(), 1);
        assert_eq!(state.floating[0].kind, FloatingKind::Help);
    }

    #[test]
    fn command_palette_wraps_selection_and_executes_overlay_commands() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ToggleFloating(FloatingKind::CommandPalette));
        state.reduce(Action::MoveCommandSelection(-1));
        assert_eq!(
            state.command_palette.selected_effect(),
            CommandEffect::CloseCommandPalette
        );

        state.reduce(Action::MoveCommandSelection(1));
        assert_eq!(
            state.command_palette.selected_effect(),
            CommandEffect::OpenFloating(FloatingKind::Help)
        );

        state.reduce(Action::ApplyCommand(CommandEffect::OpenFloating(
            FloatingKind::Help,
        )));

        assert_eq!(state.floating.len(), 1);
        assert_eq!(state.floating[0].kind, FloatingKind::Help);
    }

    #[test]
    fn command_palette_executes_panel_focus_commands() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ToggleFloating(FloatingKind::CommandPalette));
        state.reduce(Action::ApplyCommand(CommandEffect::FocusPanel(
            Panel::Research,
        )));

        assert_eq!(state.panels.focused(), Panel::Research);
        assert!(state.floating.is_empty());
    }

    #[test]
    fn command_palette_close_preserves_underlying_overlay() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ToggleFloating(FloatingKind::Help));
        state.reduce(Action::ToggleFloating(FloatingKind::CommandPalette));
        state.reduce(Action::ApplyCommand(CommandEffect::CloseCommandPalette));

        assert_eq!(state.floating.len(), 1);
        assert_eq!(state.floating[0].kind, FloatingKind::Help);
    }

    #[test]
    fn panel_lifecycle_closes_focused_panel_and_restores_all_panels() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Focus(Panel::Research));

        state.reduce(Action::CloseFocusedPanel);

        assert!(!state.panels.contains(Panel::Research));
        assert_ne!(state.panels.focused(), Panel::Research);
        assert!(state.panels.contains(state.panels.focused()));

        state.reduce(Action::RestorePanels);

        assert!(
            Panel::ALL
                .into_iter()
                .all(|panel| state.panels.contains(panel))
        );
        assert_eq!(state.panels.open_count(), Panel::ALL.len());
    }

    #[test]
    fn panel_lifecycle_toggles_panels_without_closing_the_last_one() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ApplyCommand(CommandEffect::TogglePanel(
            Panel::History,
        )));
        assert!(!state.panels.contains(Panel::History));

        state.reduce(Action::ApplyCommand(CommandEffect::TogglePanel(
            Panel::History,
        )));
        assert!(state.panels.contains(Panel::History));
        assert_eq!(state.panels.focused(), Panel::History);

        for panel in [
            Panel::Watchlist,
            Panel::Quote,
            Panel::Evidence,
            Panel::Research,
            Panel::ProviderHealth,
            Panel::TaskLog,
        ] {
            state.reduce(Action::ApplyCommand(CommandEffect::TogglePanel(panel)));
        }
        assert_eq!(state.panels.open_count(), 1);
        assert!(state.panels.contains(Panel::History));

        state.reduce(Action::ApplyCommand(CommandEffect::TogglePanel(
            Panel::History,
        )));
        assert_eq!(state.panels.open_count(), 1);
        assert!(state.panels.contains(Panel::History));
    }

    #[test]
    fn reducer_resizes_and_resets_docked_layout() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ResizeDockedColumns {
            left_ratio: 8,
            main_ratio: 80,
        });
        assert_eq!(state.layout.left_ratio, 15);
        assert_eq!(state.layout.main_ratio, 60);
        assert!(state.layout.left_ratio + state.layout.main_ratio <= MAX_LEFT_MAIN_RATIO);

        state.reduce(Action::ResetLayout);
        assert_eq!(state.layout, LayoutConfig::default());
    }

    #[test]
    fn reducer_accepts_current_snapshot_and_ignores_stale_snapshot() {
        let mut state = AppState::from_config(TuiConfig::default());
        let current = snapshot(2, "CRDO");
        let stale = snapshot(1, "AAPL");

        state.reduce(Action::RefreshStarted(2));
        state.reduce(Action::SnapshotLoaded {
            generation: 1,
            snapshot: stale,
        });
        assert!(state.market_snapshot.is_none());
        assert!(state.refresh.loading);

        state.reduce(Action::SnapshotLoaded {
            generation: 2,
            snapshot: current,
        });
        assert_eq!(
            state
                .market_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.quote_for("CRDO"))
                .and_then(|quote| quote.price),
            Some(250.0)
        );
        assert!(!state.refresh.loading);
    }

    #[test]
    fn reducer_clears_in_flight_refresh_on_scheduler_fatal_failure() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::RefreshStarted(1));
        state.reduce(Action::HistoryStarted {
            generation: 1,
            symbol: "CRDO".to_string(),
        });
        state.reduce(Action::EvidenceStarted {
            generation: 1,
            symbol: "BTCUSDT".to_string(),
        });
        state.reduce(Action::ResearchStarted {
            generation: 1,
            symbol: "CRDO".to_string(),
        });
        state.reduce(Action::SchedulerFailed(
            "scheduler runtime failed".to_string(),
        ));

        assert!(!state.refresh.loading);
        assert!(!state.history.loading());
        assert!(!state.evidence.loading());
        assert!(!state.research.loading());
        assert_eq!(
            state.scheduler_error.as_deref(),
            Some("scheduler runtime failed")
        );
    }

    #[test]
    fn reducer_accepts_current_history_and_ignores_stale_history() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::HistoryStarted {
            generation: 2,
            symbol: "CRDO".to_string(),
        });
        state.reduce(Action::HistoryLoaded {
            generation: 1,
            snapshot: history_snapshot("AAPL", 100.0),
        });
        assert!(state.history.selected_snapshot("AAPL").is_none());
        assert!(state.history.loading());

        state.reduce(Action::HistoryLoaded {
            generation: 2,
            snapshot: history_snapshot("CRDO", 250.0),
        });
        assert_eq!(
            state
                .history
                .selected_snapshot("CRDO")
                .and_then(|snapshot| snapshot.latest_close),
            Some(250.0)
        );
        assert!(!state.history.loading());
    }

    #[test]
    fn reducer_accepts_current_evidence_and_ignores_stale_evidence() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::EvidenceStarted {
            generation: 2,
            symbol: "BTCUSDT".to_string(),
        });
        state.reduce(Action::EvidenceLoaded {
            generation: 1,
            snapshot: evidence_snapshot("ETHUSDT", 1, 2),
        });
        assert!(state.evidence.selected_snapshot("ETHUSDT").is_none());
        assert!(state.evidence.loading());

        state.reduce(Action::EvidenceLoaded {
            generation: 2,
            snapshot: evidence_snapshot("BTCUSDT", 2, 3),
        });
        assert_eq!(
            state
                .evidence
                .selected_snapshot("BTCUSDT")
                .map(|snapshot| (snapshot.ok_providers, snapshot.total_providers)),
            Some((2, 3))
        );
        assert!(!state.evidence.loading());
    }

    #[test]
    fn reducer_accepts_current_research_and_ignores_stale_research() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::ResearchStarted {
            generation: 2,
            symbol: "CRDO".to_string(),
        });
        state.reduce(Action::ResearchLoaded {
            generation: 1,
            snapshot: research_snapshot("AAPL", 1, 1),
        });
        assert!(state.research.selected_snapshot("AAPL").is_none());
        assert!(state.research.loading());

        state.reduce(Action::ResearchLoaded {
            generation: 2,
            snapshot: research_snapshot("CRDO", 2, 3),
        });
        assert_eq!(
            state
                .research
                .selected_snapshot("CRDO")
                .map(|snapshot| (snapshot.news.len(), snapshot.prediction_markets.len())),
            Some((2, 3))
        );
        assert!(!state.research.loading());
    }

    fn snapshot(_generation: u64, symbol: &str) -> MarketSnapshot {
        MarketSnapshot {
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            quotes: vec![QuoteSnapshot {
                symbol: symbol.to_string(),
                price: Some(250.0),
                currency: Some("USD".to_string()),
                provider: "test".to_string(),
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

    fn history_snapshot(symbol: &str, latest_close: f64) -> HistorySnapshot {
        HistorySnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            provider: "test".to_string(),
            interval: "1d".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            latest_close: Some(latest_close),
            latest_time: Some("2026-06-25".to_string()),
            return_pct: Some(1.0),
            volume: Some(10_000.0),
            bars: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn evidence_snapshot(
        symbol: &str,
        ok_providers: usize,
        total_providers: usize,
    ) -> CryptoQuoteEvidenceSnapshot {
        CryptoQuoteEvidenceSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            instrument: "spot".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            ok_providers,
            total_providers,
            providers: Vec::new(),
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
