use agent_finance_core::submit::SubmitMode;
use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::model::ProviderProfile;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::service;
use agent_finance_market::snapshot::MarketSnapshot;

use crate::account::AccountSnapshot;
use crate::command::{ActionId, CommandPaletteState};
use crate::config::{
    FloatingConfig, LayoutConfig, PanelConfig, ProviderConfig, TuiConfig, WorkspaceConfig,
};
use crate::futures_state_ticket::{FuturesStateTicket, FuturesStateTicketPreview};
use crate::keymap::KeymapConfig;
use crate::model::{DockedPanels, FloatingKind, FloatingPane, FloatingSize, Panel, WorkspaceKind};
use crate::order_ticket::{OrderTicket, OrderTicketPreview};
use crate::profile_editor::ProfileEditorState;
use crate::search::SymbolSearchState;
use crate::settings_editor::SettingsEditorState;
use crate::task_failure::TaskFailures;
use crate::task_log::TaskLog;
use crate::theme::ThemeConfig;
use crate::transfer_ticket::{TransferTicket, TransferTicketPreview};
use crate::watchlist_editor::WatchlistAddState;

mod interaction;
mod lifecycle;
mod load;
mod profile;
mod staged_change;
mod workspace;

use load::LoadSlot;
pub use load::{SelectedDataState, SelectedSymbolLoad, SymbolSnapshot};
#[cfg(test)]
pub use staged_change::StagedChangeStage;
pub(crate) use staged_change::VISIBLE_REVIEW_LIMIT;
pub use staged_change::{
    CancelReview, FuturesStateReview, OrderTicketReview, StagedChangeEvent, StagedChangeRequest,
    StagedChangeSubject, StagedChangeView, StagedSubmitRequest, TransferReview,
};
use staged_change::{
    CloseStagedChangeResult, OpenStagedChangeResult, QueueSubmitResult, StagedChanges,
    TransitionResult,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub watchlist: Vec<String>,
    pub selected_symbol: usize,
    pub config_changes: Vec<String>,
    pub workspace: WorkspaceKind,
    pub zoomed: bool,
    pub layout: LayoutConfig,
    pub panels: DockedPanels,
    pub floating: Vec<FloatingPane>,
    pub command_palette: CommandPaletteState,
    pub symbol_search: SymbolSearchState,
    pub watchlist_add: WatchlistAddState,
    pub profile_editor: ProfileEditorState,
    pub keymap: KeymapConfig,
    pub providers: ProviderConfig,
    pub settings_editor: SettingsEditorState,
    pub task_log: TaskLog,
    pub provider_profiles: Vec<ProviderProfile>,
    pub market_snapshot: Option<MarketSnapshot>,
    refresh: LoadSlot<()>,
    pub history: SelectedSymbolLoad<HistorySnapshot>,
    pub evidence: SelectedSymbolLoad<CryptoQuoteEvidenceSnapshot>,
    pub research: SelectedSymbolLoad<ResearchContextSnapshot>,
    account: LoadSlot<String>,
    pub account_snapshot: Option<AccountSnapshot>,
    pub selected_open_order: usize,
    pub task_failures: TaskFailures,
    pub scheduler_error: Option<String>,
    pub theme: ThemeConfig,
    pub default_submit_mode: SubmitMode,
    pub live_writes_enabled: bool,
    pub trading_profile: Option<String>,
    trading_profile_edited: bool,
    pub order_ticket: OrderTicket,
    pub transfer_ticket: TransferTicket,
    pub futures_state_ticket: FuturesStateTicket,
    staged_changes: StagedChanges,
    pending_staged_confirmation: Option<StagedSubmitRequest>,
    pending_staged_submit: Option<StagedSubmitRequest>,
    pending_provider_preferences_update: bool,
    pending_config_save: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct TrackedLayoutSnapshot {
    layout: LayoutConfig,
    open_panels: Vec<Panel>,
    persistent_floatings: Vec<FloatingPane>,
}

impl AppState {
    pub fn from_config(config: TuiConfig) -> Self {
        let mut state = Self {
            watchlist: config.watchlist,
            selected_symbol: 0,
            config_changes: Vec::new(),
            workspace: config.workspace.current,
            zoomed: false,
            layout: config.layout,
            panels: DockedPanels::from_open_focused(config.panels.open, config.panels.focused),
            floating: config.floating.panes,
            command_palette: CommandPaletteState::default(),
            symbol_search: SymbolSearchState::default(),
            watchlist_add: WatchlistAddState::default(),
            profile_editor: ProfileEditorState::default(),
            keymap: config.keymap,
            providers: config.providers,
            settings_editor: SettingsEditorState::default(),
            task_log: TaskLog::default(),
            provider_profiles: service::provider_profiles(),
            market_snapshot: None,
            refresh: LoadSlot::new(),
            history: SelectedSymbolLoad::new(),
            evidence: SelectedSymbolLoad::new(),
            research: SelectedSymbolLoad::new(),
            account: LoadSlot::new(),
            account_snapshot: None,
            selected_open_order: 0,
            task_failures: TaskFailures::default(),
            scheduler_error: None,
            theme: config.theme,
            default_submit_mode: SubmitMode::DryRun,
            live_writes_enabled: false,
            trading_profile: config.trading.default_profile,
            trading_profile_edited: false,
            order_ticket: OrderTicket::default(),
            transfer_ticket: TransferTicket::default(),
            futures_state_ticket: FuturesStateTicket::default(),
            staged_changes: StagedChanges::default(),
            pending_staged_confirmation: None,
            pending_staged_submit: None,
            pending_provider_preferences_update: false,
            pending_config_save: false,
        };
        state.apply_workspace_entry_policy();
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
        config.providers = self.providers.clone();
        config.theme = self.theme.clone();
        config.trading.default_profile = self.trading_profile.clone();
        config.normalize();
        config
    }

    pub const fn preserve_launch_profile_override(&self) -> bool {
        !self.trading_profile_edited
    }

    pub fn refresh_loading(&self) -> bool {
        self.refresh.loading()
    }

    pub fn account_loading(&self) -> bool {
        self.account.loading()
    }

    pub fn staged_change_views(&self) -> Vec<StagedChangeView> {
        self.staged_changes.views()
    }

    pub fn staged_change_review_views(&self) -> Vec<StagedChangeView> {
        self.staged_changes.review_views()
    }

    pub fn staged_change_count(&self) -> usize {
        self.staged_changes.len()
    }

    pub fn pending_staged_confirmation(&self) -> Option<&StagedSubmitRequest> {
        self.pending_staged_confirmation.as_ref()
    }

    pub fn take_pending_staged_submit(&mut self) -> Option<StagedSubmitRequest> {
        self.pending_staged_submit.take()
    }

    pub fn take_pending_provider_preferences_update(&mut self) -> Option<ProviderConfig> {
        self.pending_provider_preferences_update.then(|| {
            self.pending_provider_preferences_update = false;
            self.providers.clone()
        })
    }

    pub fn invalidate_provider_backed_loads(&mut self) {
        let cancelled_refresh = self.refresh.cancel();
        let cancelled_history = self.history.reset();
        let cancelled_evidence = self.evidence.reset();
        self.market_snapshot = None;
        if cancelled_refresh.is_some()
            || cancelled_history.is_some()
            || cancelled_evidence.is_some()
        {
            self.task_log
                .info("provider preference change invalidated in-flight market data".to_string());
        }
    }

    pub fn take_pending_config_save(&mut self) -> bool {
        let pending = self.pending_config_save;
        self.pending_config_save = false;
        pending
    }

    pub const fn effective_submit_mode(&self) -> SubmitMode {
        if self.live_writes_enabled {
            self.default_submit_mode
        } else {
            SubmitMode::DryRun
        }
    }

    pub fn order_ticket_preview(&self) -> OrderTicketPreview {
        self.order_ticket.preview(
            self.selected_symbol(),
            self.trading_profile.as_deref(),
            self.live_writes_enabled,
            self.effective_submit_mode(),
            self.selected_quote_price(),
        )
    }

    pub fn transfer_ticket_preview(&self) -> TransferTicketPreview {
        self.transfer_ticket.preview(
            self.trading_profile.as_deref(),
            self.live_writes_enabled,
            self.effective_submit_mode(),
        )
    }

    pub fn futures_state_ticket_preview(&self) -> FuturesStateTicketPreview {
        self.futures_state_ticket.preview(
            self.selected_symbol(),
            self.trading_profile.as_deref(),
            self.live_writes_enabled,
            self.effective_submit_mode(),
        )
    }

    fn stage_order_ticket(&mut self) {
        let preview = self.order_ticket_preview();
        self.focus_panel(Panel::IntentReview);
        if !preview.ready {
            self.task_log.warning_event(format!(
                "order ticket is not ready: {}",
                preview.blockers.join("; ")
            ));
            return;
        }

        let Some(review) = order_ticket_review(&preview) else {
            self.task_log
                .warning_event("order ticket review snapshot could not be built".to_string());
            return;
        };
        let request = StagedChangeRequest {
            id: order_ticket_staged_change_id(&review),
            subject: StagedChangeSubject::OrderTicket(review),
        };
        let change_id = request.id.clone();
        match self
            .staged_changes
            .open_ready(request, self.effective_submit_mode())
        {
            OpenStagedChangeResult::Opened => {
                self.task_log
                    .info(format!("staged order ticket {change_id}"));
            }
            OpenStagedChangeResult::Rejected => {
                self.task_log.warning_event(
                    "order ticket cannot replace an active staged change".to_string(),
                );
            }
        }
    }

    fn stage_transfer_ticket(&mut self) {
        let preview = self.transfer_ticket_preview();
        self.focus_panel(Panel::IntentReview);
        if !preview.ready {
            self.task_log.warning_event(format!(
                "transfer ticket is not ready: {}",
                preview.blockers.join("; ")
            ));
            return;
        }

        let Some(review) = transfer_review(&preview) else {
            self.task_log
                .warning_event("transfer review snapshot could not be built".to_string());
            return;
        };
        let request = StagedChangeRequest {
            id: transfer_staged_change_id(&review),
            subject: StagedChangeSubject::Transfer(review),
        };
        let change_id = request.id.clone();
        match self
            .staged_changes
            .open_ready(request, self.effective_submit_mode())
        {
            OpenStagedChangeResult::Opened => {
                self.task_log.info(format!("staged transfer {change_id}"));
            }
            OpenStagedChangeResult::Rejected => {
                self.task_log.warning_event(
                    "transfer ticket cannot replace an active staged change".to_string(),
                );
            }
        }
    }

    fn stage_futures_state_ticket(&mut self) {
        let preview = self.futures_state_ticket_preview();
        self.focus_panel(Panel::IntentReview);
        if !preview.ready {
            self.task_log.warning_event(format!(
                "futures state ticket is not ready: {}",
                preview.blockers.join("; ")
            ));
            return;
        }

        let Some(review) = futures_state_review(&preview) else {
            self.task_log
                .warning_event("futures state review snapshot could not be built".to_string());
            return;
        };
        let request = StagedChangeRequest {
            id: futures_state_staged_change_id(&review),
            subject: StagedChangeSubject::FuturesState(review),
        };
        let change_id = request.id.clone();
        match self
            .staged_changes
            .open_ready(request, self.effective_submit_mode())
        {
            OpenStagedChangeResult::Opened => {
                self.task_log
                    .info(format!("staged futures state {change_id}"));
            }
            OpenStagedChangeResult::Rejected => {
                self.task_log.warning_event(
                    "futures state ticket cannot replace an active staged change".to_string(),
                );
            }
        }
    }

    fn move_open_order_selection(&mut self, direction: isize) {
        let len = self
            .account_snapshot
            .as_ref()
            .map(|snapshot| snapshot.open_orders().len())
            .unwrap_or_default();
        if len == 0 {
            self.selected_open_order = 0;
            return;
        }
        self.selected_open_order =
            shift_index(self.selected_open_order.min(len - 1), len, direction);
    }

    fn adjust_selected_setting(&mut self, direction: isize) {
        let row = self.settings_editor.selected();
        let Some(change) = row.adjust(&mut self.providers, &mut self.theme, direction) else {
            return;
        };

        if change.requires_provider_reload {
            self.pending_provider_preferences_update = true;
        }
        self.mark_config_changed(change.section);
        self.task_log.info(format!(
            "setting updated: {}={}",
            row.label(),
            row.value(&self.providers, &self.theme)
        ));
    }

    pub(super) fn mark_config_changed(&mut self, section: &str) {
        if !self.config_changes.iter().any(|change| change == section) {
            self.config_changes.push(section.to_string());
        }
    }

    fn request_config_save(&mut self) {
        if self.config_changes.is_empty() {
            self.task_log.info("config already saved".to_string());
            return;
        }
        self.pending_config_save = true;
        self.task_log.info(format!(
            "config save requested for {}",
            self.config_changes.join(", ")
        ));
    }

    fn config_saved(&mut self) {
        self.config_changes.clear();
        self.pending_config_save = false;
        self.task_log.info("config saved".to_string());
    }

    fn config_save_failed(&mut self, error: String) {
        self.pending_config_save = false;
        self.task_log
            .warning_event(format!("config save failed: {error}"));
    }

    fn tracked_layout_snapshot(&self) -> TrackedLayoutSnapshot {
        let mut persistent_floatings = self
            .floating
            .iter()
            .copied()
            .filter(|pane| pane.kind.persistent())
            .collect::<Vec<_>>();
        persistent_floatings.sort_by_key(|pane| pane.kind.title());
        TrackedLayoutSnapshot {
            layout: self.layout.clone(),
            open_panels: self.panels.open_panels().to_vec(),
            persistent_floatings,
        }
    }

    fn track_layout_change(&mut self, mutate: impl FnOnce(&mut Self)) {
        let before = self.tracked_layout_snapshot();
        mutate(self);
        if self.tracked_layout_snapshot() != before {
            self.mark_config_changed("layout");
        }
    }

    fn add_watchlist_symbols(&mut self) {
        let symbols = self.watchlist_add.symbols();
        if symbols.is_empty() {
            self.task_log
                .warning_event("watchlist add requires a symbol".to_string());
            return;
        }

        let mut added = Vec::new();
        let mut selected = None;
        for symbol in symbols {
            if let Some(index) = self
                .watchlist
                .iter()
                .position(|candidate| candidate == &symbol)
            {
                selected.get_or_insert(index);
            } else {
                self.watchlist.push(symbol.clone());
                added.push(symbol);
                selected.get_or_insert(self.watchlist.len() - 1);
            }
        }

        if let Some(index) = selected {
            self.selected_symbol = index;
        }
        if added.is_empty() {
            self.task_log
                .info("watchlist already contains symbol".to_string());
        } else {
            self.mark_config_changed("watchlist");
            self.task_log
                .info(format!("added {} to watchlist", added.join(", ")));
        }
        self.watchlist_add.reset();
        self.close_floating(FloatingKind::WatchlistAdd);
    }

    fn delete_selected_watchlist_symbol(&mut self) {
        if self.watchlist.len() <= 1 {
            self.task_log
                .warning_event("watchlist must keep at least one symbol".to_string());
            return;
        }
        let index = self.selected_symbol.min(self.watchlist.len() - 1);
        let removed = self.watchlist.remove(index);
        self.selected_symbol = index.min(self.watchlist.len() - 1);
        self.mark_config_changed("watchlist");
        self.task_log
            .info(format!("removed {removed} from watchlist"));
    }

    fn move_selected_watchlist_symbol(&mut self, direction: isize) {
        if self.watchlist.len() < 2 {
            return;
        }
        let current = self.selected_symbol.min(self.watchlist.len() - 1);
        let next = current as isize + direction;
        if next < 0 || next >= self.watchlist.len() as isize {
            return;
        }
        let next = next as usize;
        let symbol = self.watchlist.remove(current);
        self.watchlist.insert(next, symbol);
        self.selected_symbol = next;
        self.mark_config_changed("watchlist");
    }

    fn stage_selected_open_order_cancel(&mut self) {
        let Some(profile) = self.trading_profile.clone() else {
            self.task_log
                .warning_event("no trading profile selected for cancel".to_string());
            return;
        };
        let Some(snapshot) = self.account_snapshot.as_ref() else {
            self.task_log
                .warning_event("no open order selected for cancel".to_string());
            return;
        };
        if snapshot.profile != profile {
            self.task_log.warning_event(
                "account snapshot profile does not match selected trading profile".to_string(),
            );
            return;
        }
        let Some(order) = snapshot
            .open_orders()
            .get(self.selected_open_order)
            .cloned()
        else {
            self.task_log
                .warning_event("no open order selected for cancel".to_string());
            return;
        };
        let Some(target) = order.cancel_target() else {
            self.task_log
                .warning_event("selected open order has no cancellable identifier".to_string());
            return;
        };
        let review = CancelReview {
            profile,
            market: order.market,
            symbol: order.symbol,
            target,
            effective_mode: self.effective_submit_mode(),
        };
        let request = StagedChangeRequest {
            id: cancel_staged_change_id(&review),
            subject: StagedChangeSubject::Cancel(review),
        };
        let change_id = request.id.clone();
        self.focus_panel(Panel::IntentReview);
        match self
            .staged_changes
            .open_ready(request, self.effective_submit_mode())
        {
            OpenStagedChangeResult::Opened => {
                self.task_log.info(format!("staged cancel {change_id}"));
            }
            OpenStagedChangeResult::Rejected => {
                self.task_log
                    .warning_event("cancel cannot replace an active staged change".to_string());
            }
        }
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
            Action::EditWatchlistAddQuery(request) => {
                self.watchlist_add.edit_query(request);
            }
            Action::EditTradingProfileQuery(request) => {
                self.profile_editor.edit_query(request);
            }
            Action::AcceptSymbolSearch => {
                if let Some(index) = self.symbol_search.selected_symbol_index() {
                    self.selected_symbol = index;
                    self.close_floating(FloatingKind::SymbolSearch);
                }
            }
            Action::AcceptWatchlistAdd => self.add_watchlist_symbols(),
            Action::AcceptTradingProfile => self.accept_trading_profile(),
            Action::DeleteSelectedWatchlistSymbol => self.delete_selected_watchlist_symbol(),
            Action::MoveSelectedWatchlistSymbol(direction) => {
                self.move_selected_watchlist_symbol(direction);
            }
            Action::MoveOrderTicketField(direction) => {
                self.order_ticket.move_field(direction);
            }
            Action::AdjustOrderTicketField(direction) => {
                self.order_ticket
                    .adjust_selected_field(direction, self.selected_quote_price());
            }
            Action::MoveTransferTicketField(direction) => {
                self.transfer_ticket.move_field(direction);
            }
            Action::AdjustTransferTicketField(direction) => {
                self.transfer_ticket.adjust_selected_field(direction);
            }
            Action::MoveFuturesStateTicketField(direction) => {
                self.futures_state_ticket.move_field(direction);
            }
            Action::AdjustFuturesStateTicketField(direction) => {
                let symbol = self.selected_symbol().map(ToString::to_string);
                self.futures_state_ticket
                    .adjust_selected_field(direction, symbol.as_deref());
            }
            Action::MoveOpenOrderSelection(direction) => self.move_open_order_selection(direction),
            Action::MoveSettingsSelection(direction) => {
                self.settings_editor.move_selection(direction);
            }
            Action::AdjustSelectedSetting(direction) => self.adjust_selected_setting(direction),
            Action::StageOrderTicket => self.stage_order_ticket(),
            Action::StageTransferTicket => self.stage_transfer_ticket(),
            Action::StageFuturesStateTicket => self.stage_futures_state_ticket(),
            Action::StageSelectedOpenOrderCancel => self.stage_selected_open_order_cancel(),
            Action::MoveStagedChangeSelection(direction) => {
                self.staged_changes.move_selection(direction);
            }
            Action::SubmitStagedChange => self.request_staged_submit_confirmation(),
            Action::ConfirmStagedSubmit => self.confirm_staged_submit(),
            Action::CancelStagedSubmitConfirmation => self.cancel_staged_submit_confirmation(),
            Action::RequestConfigSave => self.request_config_save(),
            Action::ConfigSaved => self.config_saved(),
            Action::ConfigSaveFailed(error) => self.config_save_failed(error),
            Action::Execute(action) => self.execute(action),
            Action::CloseFocusedPanel => {
                self.track_layout_change(|state| {
                    state.panels.close_focused();
                    state.clear_zoom();
                    state.ensure_visible_focus();
                });
            }
            Action::RestorePanels => {
                self.track_layout_change(|state| {
                    state.panels.restore();
                    state.clear_zoom();
                    state.ensure_visible_focus();
                });
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
                if self
                    .floating
                    .last()
                    .is_some_and(|pane| pane.kind == FloatingKind::StagedSubmitConfirmation)
                {
                    self.cancel_staged_submit_confirmation();
                } else {
                    self.close_top_floating();
                }
            }
            Action::FocusFloating(kind) => self.focus_floating(kind),
            Action::ResizeFloating { kind, size } => self.resize_floating(kind, size),
            Action::ResetLayout => {
                self.track_layout_change(|state| {
                    state.reset_open_floating_state();
                    state.floating.clear();
                    state.clear_zoom();
                    state.layout = LayoutConfig::default();
                    state.panels = DockedPanels::default();
                    state.ensure_visible_focus();
                });
            }
            Action::ResizeDockedColumns {
                left_ratio,
                main_ratio,
            } => {
                self.track_layout_change(|state| {
                    state.layout.left_ratio = left_ratio;
                    state.layout.main_ratio = main_ratio;
                    state.layout.normalize();
                });
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
            Action::AccountStarted {
                generation,
                profile,
            } => self.account_started(generation, profile),
            Action::AccountLoaded {
                generation,
                snapshot,
            } => self.account_loaded(generation, snapshot),
            Action::AccountFailed {
                generation,
                profile,
                error,
            } => self.account_failed(generation, profile, error),
            Action::SchedulerFailed(error) => self.scheduler_failed(error),
            Action::SetDefaultSubmitMode(mode) => {
                self.default_submit_mode = mode;
                self.task_log
                    .info(format!("default write mode set to {mode}"));
            }
            Action::SetLiveWritesEnabled(enabled) => {
                self.live_writes_enabled = enabled;
                self.close_floating(FloatingKind::LiveWritesConfirmation);
                self.task_log.info(if enabled {
                    "live writes enabled for this TUI session".to_string()
                } else {
                    "live writes disabled for this TUI session".to_string()
                });
                if !enabled {
                    self.cancel_staged_submit_confirmation();
                    let abandoned = self.staged_changes.disable_live();
                    if abandoned > 0 {
                        self.task_log.warning_event(format!(
                            "abandoned {abandoned} pending live staged change(s)"
                        ));
                    }
                }
            }
            Action::OpenStagedChange(request) => {
                match self
                    .staged_changes
                    .open(request, self.effective_submit_mode())
                {
                    OpenStagedChangeResult::Opened => {}
                    OpenStagedChangeResult::Rejected => self
                        .task_log
                        .warning_event("staged change cannot replace an active change".to_string()),
                }
            }
            Action::ApplyStagedChangeEvent { id, event } => {
                match self.staged_changes.apply(&id, event) {
                    TransitionResult::Applied => {}
                    TransitionResult::Missing => self
                        .task_log
                        .warning_event(format!("staged change {id} is no longer present")),
                    TransitionResult::Rejected { current, event } => {
                        self.task_log.warning_event(format!(
                            "staged change {id} cannot apply {event:?} from {current}"
                        ));
                    }
                }
            }
            Action::CloseStagedChange(id) => match self.staged_changes.close(&id) {
                CloseStagedChangeResult::Closed => {}
                CloseStagedChangeResult::Missing => self
                    .task_log
                    .warning_event(format!("staged change {id} is no longer present")),
                CloseStagedChangeResult::Rejected { current } => self
                    .task_log
                    .warning_event(format!("staged change {id} cannot close while {current}")),
            },
            Action::CloseSelectedStagedChange => match self.staged_changes.close_selected() {
                CloseStagedChangeResult::Closed => {}
                CloseStagedChangeResult::Missing => self
                    .task_log
                    .warning_event("no staged change selected".to_string()),
                CloseStagedChangeResult::Rejected { current } => self
                    .task_log
                    .warning_event(format!("staged change cannot close while {current}")),
            },
            Action::Log(message) => self.task_log.info(message),
        }
    }

    fn request_staged_submit_confirmation(&mut self) {
        if self.pending_staged_confirmation.is_some() {
            self.open_floating(FloatingKind::StagedSubmitConfirmation);
            return;
        }
        match self.staged_changes.selected_submit_request() {
            QueueSubmitResult::Queued(request) => {
                self.task_log.info(format!(
                    "staged {} change {} awaiting submit confirmation as {}",
                    request.subject.summary(),
                    request.id,
                    request.mode
                ));
                self.pending_staged_confirmation = Some(request);
                self.open_floating(FloatingKind::StagedSubmitConfirmation);
            }
            QueueSubmitResult::Missing => self
                .task_log
                .warning_event("no selected staged change to submit".to_string()),
            QueueSubmitResult::Rejected { current } => self.task_log.warning_event(format!(
                "selected staged change cannot submit from current state {current}"
            )),
        }
    }

    fn confirm_staged_submit(&mut self) {
        let Some(request) = self.pending_staged_confirmation.take() else {
            self.task_log
                .warning_event("no staged submit confirmation is pending".to_string());
            self.close_floating(FloatingKind::StagedSubmitConfirmation);
            return;
        };
        match self.staged_changes.queue_submit_request(&request) {
            QueueSubmitResult::Queued(request) => {
                self.task_log.info(format!(
                    "submitting staged {} change {} as {}",
                    request.subject.summary(),
                    request.id,
                    request.mode
                ));
                self.pending_staged_submit = Some(request);
            }
            QueueSubmitResult::Missing => self
                .task_log
                .warning_event("pending staged submit confirmation disappeared".to_string()),
            QueueSubmitResult::Rejected { current } => self.task_log.warning_event(format!(
                "pending staged submit confirmation cannot submit from {current}"
            )),
        }
        self.close_floating(FloatingKind::StagedSubmitConfirmation);
    }

    fn cancel_staged_submit_confirmation(&mut self) {
        let Some(request) = self.pending_staged_confirmation.take() else {
            self.close_floating(FloatingKind::StagedSubmitConfirmation);
            return;
        };
        self.task_log.info(format!(
            "cancelled staged submit confirmation for {}",
            request.id
        ));
        self.close_floating(FloatingKind::StagedSubmitConfirmation);
    }
}

fn order_ticket_review(preview: &OrderTicketPreview) -> Option<OrderTicketReview> {
    Some(OrderTicketReview {
        symbol: preview.symbol.clone()?,
        profile: preview.profile.clone()?,
        market: preview.market,
        side: preview.side,
        kind: preview.kind,
        quantity: preview.quantity.clone()?,
        price: preview.price.clone(),
        time_in_force: preview.time_in_force,
        reduce_only: preview.reduce_only,
        parsed_quantity: preview.parsed_quantity.clone()?,
        order_spec: preview.order_spec.clone()?,
        effective_mode: preview.effective_mode,
    })
}

fn order_ticket_staged_change_id(review: &OrderTicketReview) -> String {
    let mut parts = vec![
        "order-ticket".to_string(),
        review.profile.clone(),
        review.effective_mode.to_string(),
        review.market.to_string(),
        review.side.to_string(),
        review.kind.to_string(),
        review.time_in_force.to_string(),
        review.reduce_only.to_string(),
        review.symbol.clone(),
        review.quantity.clone(),
    ];
    parts.extend(review.price.clone());
    sanitize_staged_change_id(&parts.join("-"))
}

fn transfer_review(preview: &TransferTicketPreview) -> Option<TransferReview> {
    Some(TransferReview {
        profile: preview.profile.clone()?,
        direction: preview.direction,
        asset: preview.asset.clone(),
        amount: preview.amount.clone()?,
        parsed_amount: preview.parsed_amount.clone()?,
        effective_mode: preview.effective_mode,
    })
}

fn transfer_staged_change_id(review: &TransferReview) -> String {
    sanitize_staged_change_id(&format!(
        "transfer-{}-{}-{}-{}-{}",
        review.profile, review.effective_mode, review.direction, review.asset, review.amount
    ))
}

fn futures_state_review(preview: &FuturesStateTicketPreview) -> Option<FuturesStateReview> {
    Some(FuturesStateReview {
        profile: preview.profile.clone()?,
        change: preview.change.clone()?,
        effective_mode: preview.effective_mode,
    })
}

fn futures_state_staged_change_id(review: &FuturesStateReview) -> String {
    sanitize_staged_change_id(&format!(
        "futures-state-{}-{}-{}-{}",
        review.profile,
        review.effective_mode,
        review.change.kind(),
        review.change.review_label()
    ))
}

fn cancel_staged_change_id(review: &CancelReview) -> String {
    sanitize_staged_change_id(&format!(
        "cancel-{}-{}-{}-{}-{}",
        review.profile,
        review.effective_mode,
        review.market,
        review.symbol,
        review.identifier()
    ))
}

fn shift_index(index: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    (index as isize + direction).rem_euclid(len) as usize
}

fn sanitize_staged_change_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .fold(String::new(), |mut normalized, character| {
            if character != '-' || !normalized.ends_with('-') {
                normalized.push(character);
            }
            normalized
        })
        .trim_matches('-')
        .to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Focus(Panel),
    MoveCommandSelection(isize),
    EditCommandQuery(tui_input::InputRequest),
    MoveSymbolSearchSelection(isize),
    EditSymbolSearchQuery(tui_input::InputRequest),
    EditWatchlistAddQuery(tui_input::InputRequest),
    EditTradingProfileQuery(tui_input::InputRequest),
    AcceptSymbolSearch,
    AcceptWatchlistAdd,
    AcceptTradingProfile,
    DeleteSelectedWatchlistSymbol,
    MoveSelectedWatchlistSymbol(isize),
    MoveOrderTicketField(isize),
    AdjustOrderTicketField(isize),
    MoveTransferTicketField(isize),
    AdjustTransferTicketField(isize),
    MoveFuturesStateTicketField(isize),
    AdjustFuturesStateTicketField(isize),
    MoveOpenOrderSelection(isize),
    MoveSettingsSelection(isize),
    AdjustSelectedSetting(isize),
    StageOrderTicket,
    StageTransferTicket,
    StageFuturesStateTicket,
    StageSelectedOpenOrderCancel,
    MoveStagedChangeSelection(isize),
    SubmitStagedChange,
    ConfirmStagedSubmit,
    CancelStagedSubmitConfirmation,
    RequestConfigSave,
    ConfigSaved,
    ConfigSaveFailed(String),
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
    AccountStarted {
        generation: u64,
        profile: String,
    },
    AccountLoaded {
        generation: u64,
        snapshot: AccountSnapshot,
    },
    AccountFailed {
        generation: u64,
        profile: String,
        error: String,
    },
    SchedulerFailed(String),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "submit mode changes are reserved for a confirmed write-mode selector"
        )
    )]
    SetDefaultSubmitMode(SubmitMode),
    SetLiveWritesEnabled(bool),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "staged change actions are part of the state contract before staged change panels bind them"
        )
    )]
    OpenStagedChange(StagedChangeRequest),
    ApplyStagedChangeEvent {
        id: String,
        event: StagedChangeEvent,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "staged change actions are part of the state contract before staged change panels bind them"
        )
    )]
    CloseStagedChange(String),
    CloseSelectedStagedChange,
    Log(String),
}

#[cfg(test)]
mod tests;
