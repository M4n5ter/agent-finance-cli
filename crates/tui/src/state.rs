use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::model::ProviderProfile;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::service;
use agent_finance_market::snapshot::MarketSnapshot;

use crate::command::{ActionId, CommandPaletteState};
use crate::config::{FloatingConfig, LayoutConfig, PanelConfig, TuiConfig, WorkspaceConfig};
use crate::keymap::KeymapConfig;
use crate::model::{DockedPanels, FloatingKind, FloatingPane, FloatingSize, Panel, WorkspaceKind};
use crate::search::SymbolSearchState;
use crate::task_failure::TaskFailures;
use crate::task_log::TaskLog;

mod interaction;
mod lifecycle;
mod load;
mod workspace;

use load::LoadSlot;
pub use load::{SelectedDataState, SelectedSymbolLoad, SymbolSnapshot};

#[derive(Debug, Clone)]
pub struct AppState {
    pub watchlist: Vec<String>,
    pub selected_symbol: usize,
    pub workspace: WorkspaceKind,
    pub zoomed: bool,
    pub layout: LayoutConfig,
    pub panels: DockedPanels,
    pub floating: Vec<FloatingPane>,
    pub command_palette: CommandPaletteState,
    pub symbol_search: SymbolSearchState,
    pub keymap: KeymapConfig,
    pub task_log: TaskLog,
    pub provider_profiles: Vec<ProviderProfile>,
    pub market_snapshot: Option<MarketSnapshot>,
    refresh: LoadSlot<()>,
    pub history: SelectedSymbolLoad<HistorySnapshot>,
    pub evidence: SelectedSymbolLoad<CryptoQuoteEvidenceSnapshot>,
    pub research: SelectedSymbolLoad<ResearchContextSnapshot>,
    pub task_failures: TaskFailures,
    pub scheduler_error: Option<String>,
}

impl AppState {
    pub fn from_config(config: TuiConfig) -> Self {
        let mut state = Self {
            watchlist: config.watchlist,
            selected_symbol: 0,
            workspace: config.workspace.current,
            zoomed: false,
            layout: config.layout,
            panels: DockedPanels::from_open_focused(config.panels.open, config.panels.focused),
            floating: config.floating.panes,
            command_palette: CommandPaletteState::default(),
            symbol_search: SymbolSearchState::default(),
            keymap: config.keymap,
            task_log: TaskLog::default(),
            provider_profiles: service::provider_profiles(),
            market_snapshot: None,
            refresh: LoadSlot::new(),
            history: SelectedSymbolLoad::new(),
            evidence: SelectedSymbolLoad::new(),
            research: SelectedSymbolLoad::new(),
            task_failures: TaskFailures::default(),
            scheduler_error: None,
        };
        state.ensure_visible_focus();
        state
    }

    pub fn export_config(&self, base: &TuiConfig) -> TuiConfig {
        let mut config = base.clone();
        config.watchlist = self.watchlist.clone();
        config.workspace = WorkspaceConfig {
            current: self.workspace,
        };
        config.layout = self.layout.clone();
        config.panels = PanelConfig {
            open: self.panels.open_panels().to_vec(),
            focused: self.panels.focused(),
        };
        config.floating = FloatingConfig {
            panes: self
                .floating
                .iter()
                .copied()
                .filter(|pane| pane.kind.persistent())
                .collect(),
        };
        config.keymap = self.keymap.clone();
        config.normalize();
        config
    }

    pub fn refresh_loading(&self) -> bool {
        self.refresh.loading()
    }

    pub fn reduce(&mut self, action: Action) {
        match action {
            Action::Focus(panel) => {
                self.focus_panel(panel);
            }
            Action::MoveCommandSelection(direction) => {
                self.command_palette.shift(direction);
            }
            Action::EditCommandQuery(request) => {
                self.command_palette.edit_query(request);
            }
            Action::MoveSymbolSearchSelection(direction) => {
                self.symbol_search.shift(direction);
            }
            Action::EditSymbolSearchQuery(request) => {
                self.symbol_search.edit_query(&self.watchlist, request);
            }
            Action::AcceptSymbolSearch => {
                if let Some(index) = self.symbol_search.selected_symbol_index() {
                    self.selected_symbol = index;
                    self.close_floating(FloatingKind::SymbolSearch);
                }
            }
            Action::Execute(action) => self.execute(action),
            Action::CloseFocusedPanel => {
                self.panels.close_focused();
                self.clear_zoom();
                self.ensure_visible_focus();
            }
            Action::RestorePanels => {
                self.panels.restore();
                self.clear_zoom();
                self.ensure_visible_focus();
            }
            Action::ShiftWorkspace(direction) => {
                self.set_workspace(self.workspace.shift(direction))
            }
            Action::SetWorkspace(workspace) => self.set_workspace(workspace),
            Action::FocusPanelBy(direction) => self.focus_panel_by(direction),
            Action::ToggleFocusedZoom => {
                if !self.visible_panels().is_empty() {
                    self.zoomed = !self.zoomed;
                }
            }
            Action::CloseFocusedFloating => {
                if let Some(pane) = self.floating.pop() {
                    self.reset_floating_state(pane.kind);
                }
            }
            Action::FocusFloating(kind) => self.focus_floating(kind),
            Action::ResizeFloating { kind, size } => self.resize_floating(kind, size),
            Action::ResetLayout => {
                self.reset_open_floating_state();
                self.floating.clear();
                self.clear_zoom();
                self.layout = LayoutConfig::default();
                self.panels = DockedPanels::default();
                self.ensure_visible_focus();
            }
            Action::ResizeDockedColumns {
                left_ratio,
                main_ratio,
            } => {
                self.layout.left_ratio = left_ratio;
                self.layout.main_ratio = main_ratio;
                self.layout.normalize();
            }
            Action::RefreshStarted(generation) => self.refresh_started(generation),
            Action::SnapshotLoaded {
                generation,
                snapshot,
            } => self.snapshot_loaded(generation, snapshot),
            Action::RefreshFailed { generation, error } => self.refresh_failed(generation, error),
            Action::HistoryStarted { generation, symbol } => {
                self.history_started(generation, symbol);
            }
            Action::HistoryLoaded {
                generation,
                snapshot,
            } => self.history_loaded(generation, snapshot),
            Action::HistoryFailed {
                generation,
                symbol,
                error,
            } => self.history_failed(generation, symbol, error),
            Action::EvidenceStarted { generation, symbol } => {
                self.evidence_started(generation, symbol);
            }
            Action::EvidenceLoaded {
                generation,
                snapshot,
            } => self.evidence_loaded(generation, snapshot),
            Action::EvidenceFailed {
                generation,
                symbol,
                error,
            } => self.evidence_failed(generation, symbol, error),
            Action::ResearchStarted { generation, symbol } => {
                self.research_started(generation, symbol);
            }
            Action::ResearchLoaded {
                generation,
                snapshot,
            } => self.research_loaded(generation, snapshot),
            Action::SchedulerFailed(error) => self.scheduler_failed(error),
            Action::Log(message) => self.task_log.info(message),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Focus(Panel),
    MoveCommandSelection(isize),
    EditCommandQuery(tui_input::InputRequest),
    MoveSymbolSearchSelection(isize),
    EditSymbolSearchQuery(tui_input::InputRequest),
    AcceptSymbolSearch,
    Execute(ActionId),
    FocusPanelBy(isize),
    ToggleFocusedZoom,
    CloseFocusedPanel,
    RestorePanels,
    ShiftWorkspace(isize),
    SetWorkspace(WorkspaceKind),
    CloseFocusedFloating,
    FocusFloating(FloatingKind),
    ResizeFloating {
        kind: FloatingKind,
        size: FloatingSize,
    },
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
mod tests;
