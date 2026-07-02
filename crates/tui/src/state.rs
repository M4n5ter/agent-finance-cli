use agent_finance_core::OrderKind;
use agent_finance_core::submit::SubmitMode;
use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::is_likely_crypto_pair;
use agent_finance_market::model::ProviderProfile;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::service;
use agent_finance_market::snapshot::MarketSnapshot;

use crate::account::AccountSnapshot;
use crate::chart::{ChartGlyphMode, ChartInterval, ChartPreset, ChartState};
use crate::command::{ActionId, CommandPaletteState};
use crate::config::{
    FloatingConfig, LayoutConfig, PanelConfig, ProviderConfig, TuiConfig, WorkspaceConfig,
};
use crate::futures_state_ticket::{FuturesStateTicket, FuturesStateTicketPreview};
use crate::model::{DockedPanels, FloatingKind, FloatingPane, FloatingSize, Panel, WorkspaceKind};
use crate::mouse_target::MousePosition;
use crate::order_ticket::{OrderTicket, OrderTicketPreview, ProtectiveDraftSlot};
use crate::profile_editor::ProfileEditorState;
use crate::profile_snapshot::{ProfileValidationSnapshot, ProfileValidationState};
use crate::scheduler::SymbolTaskKind;
use crate::search::SymbolSearchState;
use crate::settings_editor::SettingsEditorState;
use crate::task_failure::TaskFailures;
use crate::task_log::TaskLog;
use crate::ticket_text_input::{TicketTextInputKind, TicketTextInputState, TicketTextInputTarget};
use crate::transfer_ticket::{TransferTicket, TransferTicketPreview};
use crate::watchlist_editor::WatchlistAddState;

mod config_undo;
mod interaction;
mod lifecycle;
mod load;
mod profile;
mod staged_change;
mod staged_confirmation;
mod workspace;

pub(super) use config_undo::LocalConfigEdit;
use config_undo::LocalConfigHistory;
use load::LoadSlot;
pub use load::{SelectedDataState, SelectedSymbolLoad, SymbolSnapshot};
#[cfg(test)]
pub use staged_change::ProfileRiskChange;
#[cfg(test)]
pub use staged_change::StagedChangeKind;
pub(crate) use staged_change::StagedChangeQueueStatus;
#[cfg(test)]
pub use staged_change::StagedChangeStage;
pub(crate) use staged_change::VISIBLE_REVIEW_LIMIT;
pub use staged_change::{
    CancelReview, FuturesStateReview, OrderTicketReview, ProfileRiskReview, StagedChangeEvent,
    StagedChangeRequest, StagedChangeSubject, StagedChangeView, StagedExecution,
    StagedExecutionRequest, StagedLocalCommitSubject, StagedSubmitRequest, StagedSubmitSubject,
    TransferReview, TypedConfirmation,
};
use staged_change::{
    CloseStagedChangeResult, OpenStagedChangeResult, QueueExecutionResult, StagedChanges,
    TransitionResult,
};
use staged_confirmation::PendingStagedConfirmation;
pub(crate) use staged_confirmation::{PendingStagedConfirmationView, TypedConfirmationGateView};

#[derive(Debug, Clone)]
pub struct AppState {
    pub locale: agent_finance_i18n::LocaleId,
    pub watchlist: Vec<String>,
    pub selected_symbol: usize,
    pub config_changes: Vec<String>,
    config_undo_history: LocalConfigHistory,
    pub workspace: WorkspaceKind,
    pub zoomed: bool,
    pub layout: LayoutConfig,
    pub panels: DockedPanels,
    pub floating: Vec<FloatingPane>,
    pub command_palette: CommandPaletteState,
    pub symbol_search: SymbolSearchState,
    pub watchlist_add: WatchlistAddState,
    pub profile_editor: ProfileEditorState,
    pub ticket_text_input: TicketTextInputState,
    pub keymap: crate::keymap::KeymapConfig,
    pub providers: ProviderConfig,
    pub chart: ChartState,
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
    profile_validation_request: LoadSlot<String>,
    pub profile_validation: ProfileValidationState,
    pub selected_open_order: usize,
    pub task_failures: TaskFailures,
    pub scheduler_error: Option<String>,
    pub theme: crate::theme::ThemeConfig,
    pub default_submit_mode: SubmitMode,
    pub live_writes_enabled: bool,
    pub trading_profile: Option<String>,
    trading_profile_edited: bool,
    pub order_ticket: OrderTicket,
    pub transfer_ticket: TransferTicket,
    pub futures_state_ticket: FuturesStateTicket,
    pub mouse_position: Option<MousePosition>,
    staged_changes: StagedChanges,
    pending_staged_confirmation: Option<PendingStagedConfirmation>,
    pending_staged_execution: Option<StagedExecutionRequest>,
    pending_market_refresh: bool,
    pending_symbol_data_refreshes: Vec<SymbolTaskKind>,
    pending_account_refresh: bool,
    pending_provider_preferences_update: bool,
    pending_config_save: bool,
}

fn symbol_task_label(kind: SymbolTaskKind) -> &'static str {
    match kind {
        SymbolTaskKind::History => "history",
        SymbolTaskKind::Evidence => "evidence",
        SymbolTaskKind::Research => "research",
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct TrackedLayoutSnapshot {
    layout: LayoutConfig,
    open_panels: Vec<Panel>,
    persistent_floatings: Vec<FloatingPane>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ChartReferenceSelectionFailure {
    NoSelectedSymbol,
    HiddenOverlays,
    NoSelectedLine,
    StaleSelection,
}

impl ChartReferenceSelectionFailure {
    const fn warning(self) -> &'static str {
        match self {
            Self::NoSelectedSymbol => "no selected symbol for chart price capture",
            Self::HiddenOverlays => "chart reference lines are hidden",
            Self::NoSelectedLine => "no chart reference line selected",
            Self::StaleSelection => "selected chart reference line is no longer available",
        }
    }
}

impl AppState {
    pub fn from_config(config: TuiConfig) -> Self {
        let mut state = Self {
            locale: config
                .locale
                .current
                .unwrap_or(agent_finance_i18n::LocaleId::DEFAULT),
            watchlist: config.watchlist,
            selected_symbol: 0,
            config_changes: Vec::new(),
            config_undo_history: LocalConfigHistory::default(),
            workspace: config.workspace.current,
            zoomed: false,
            layout: config.layout,
            panels: DockedPanels::from_open_focused(config.panels.open, config.panels.focused),
            floating: config.floating.panes,
            command_palette: CommandPaletteState::default(),
            symbol_search: SymbolSearchState::default(),
            watchlist_add: WatchlistAddState::default(),
            profile_editor: ProfileEditorState::default(),
            ticket_text_input: TicketTextInputState::default(),
            keymap: config.keymap,
            providers: config.providers,
            chart: ChartState::new(config.chart.preset),
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
            profile_validation_request: LoadSlot::new(),
            profile_validation: ProfileValidationState::idle(),
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
            mouse_position: None,
            staged_changes: StagedChanges::default(),
            pending_staged_confirmation: None,
            pending_staged_execution: None,
            pending_market_refresh: false,
            pending_symbol_data_refreshes: Vec::new(),
            pending_account_refresh: false,
            pending_provider_preferences_update: false,
            pending_config_save: false,
        };
        state.apply_workspace_entry_policy();
        state.ensure_visible_focus();
        state
    }

    pub fn export_config(&self, base: &TuiConfig) -> TuiConfig {
        let mut config = base.clone();
        config.locale.current = Some(self.locale);
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
        config.chart.preset = self.chart.preset();
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

    pub fn profile_validation_loading(&self) -> bool {
        self.profile_validation_request.loading()
    }

    pub fn has_current_profile_validation(&self) -> bool {
        self.trading_profile
            .as_ref()
            .is_some_and(|profile| self.profile_validation.terminal_for(profile))
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

    pub fn config_undo_available(&self) -> bool {
        self.config_undo_history.available()
    }

    pub fn pending_staged_confirmation(&self) -> Option<&StagedExecutionRequest> {
        self.pending_staged_confirmation
            .as_ref()
            .map(PendingStagedConfirmation::request)
    }

    pub(crate) fn pending_staged_confirmation_view(
        &self,
    ) -> Option<PendingStagedConfirmationView<'_>> {
        self.pending_staged_confirmation
            .as_ref()
            .map(PendingStagedConfirmation::view)
    }

    pub(crate) fn pending_staged_confirmation_gate(&self) -> Option<TypedConfirmationGateView<'_>> {
        self.pending_staged_confirmation
            .as_ref()
            .and_then(PendingStagedConfirmation::typed_gate)
    }

    pub(crate) fn pending_staged_confirmation_accepts_text_input(&self) -> bool {
        self.pending_staged_confirmation
            .as_ref()
            .is_some_and(PendingStagedConfirmation::accepts_text_input)
    }

    pub fn take_pending_staged_execution(&mut self) -> Option<StagedExecutionRequest> {
        self.pending_staged_execution.take()
    }

    pub fn take_pending_market_refresh(&mut self) -> bool {
        std::mem::take(&mut self.pending_market_refresh)
    }

    pub fn take_pending_symbol_data_refreshes(&mut self) -> Vec<SymbolTaskKind> {
        std::mem::take(&mut self.pending_symbol_data_refreshes)
    }

    pub fn take_pending_account_refresh(&mut self) -> bool {
        std::mem::take(&mut self.pending_account_refresh)
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

    fn capture_order_reference_price(&mut self) {
        let symbol = self.selected_symbol().map(ToString::to_string);
        let Some(price) = self.selected_quote_price() else {
            self.task_log.warning_event(format!(
                "no quote price available for {}",
                symbol.as_deref().unwrap_or("selected symbol")
            ));
            return;
        };
        self.order_ticket.capture_reference_price(price);
        self.focus_panel(Panel::OrderTicket);
        self.task_log.info(format!(
            "captured order reference price {} for {}",
            self.order_ticket_preview().price.as_deref().unwrap_or("-"),
            symbol.as_deref().unwrap_or("selected symbol")
        ));
    }

    fn capture_chart_price(&mut self, price: f64) {
        let symbol = self.selected_symbol().map(ToString::to_string);
        self.order_ticket.capture_reference_price(price);
        self.focus_panel(Panel::OrderTicket);
        self.task_log.info(format!(
            "captured chart price {} for {}",
            self.order_ticket_preview().price.as_deref().unwrap_or("-"),
            symbol.as_deref().unwrap_or("selected symbol")
        ));
    }

    fn capture_selected_chart_reference_price(&mut self) {
        let (symbol, line) = match self.selected_chart_reference_line() {
            Ok(selection) => selection,
            Err(reason) => {
                self.task_log.warning_event(reason.warning().to_string());
                return;
            }
        };
        self.order_ticket.capture_reference_price(line.price);
        self.focus_panel(Panel::OrderTicket);
        self.task_log.info(format!(
            "captured chart reference {} {} for {}",
            line.label,
            self.order_ticket_preview().price.as_deref().unwrap_or("-"),
            symbol
        ));
    }

    fn capture_selected_chart_reference_as(&mut self, kind: OrderKind) {
        let (symbol, line) = match self.selected_chart_reference_line() {
            Ok(selection) => selection,
            Err(reason) => {
                self.task_log.warning_event(reason.warning().to_string());
                return;
            }
        };
        self.order_ticket
            .capture_reference_price_as(line.price, kind);
        self.focus_panel(Panel::OrderTicket);
        self.task_log.info(format!(
            "prepared {} from chart reference {} {} for {}",
            kind,
            line.label,
            self.order_ticket_preview().price.as_deref().unwrap_or("-"),
            symbol
        ));
    }

    fn capture_selected_chart_reference_for_protective_draft(&mut self, slot: ProtectiveDraftSlot) {
        let (symbol, line) = match self.selected_chart_reference_line() {
            Ok(selection) => selection,
            Err(reason) => {
                self.task_log.warning_event(reason.warning().to_string());
                return;
            }
        };
        self.order_ticket
            .capture_protective_reference(line.price, slot);
        self.focus_panel(Panel::OrderTicket);
        let draft = &self.order_ticket_preview().protective_draft;
        self.task_log.info(format!(
            "added {} chart reference {} to protective draft for {} (stop-loss={} take-profit={})",
            slot,
            line.label,
            symbol,
            draft.stop_loss.as_deref().unwrap_or("-"),
            draft.take_profit.as_deref().unwrap_or("-")
        ));
    }

    fn selected_chart_reference_line(
        &self,
    ) -> Result<(String, crate::chart_overlay::ChartOverlayLine), ChartReferenceSelectionFailure>
    {
        let symbol = self
            .selected_symbol()
            .map(ToString::to_string)
            .ok_or(ChartReferenceSelectionFailure::NoSelectedSymbol)?;
        if !self.chart.overlays_visible() {
            return Err(ChartReferenceSelectionFailure::HiddenOverlays);
        }
        let index = self
            .chart
            .selected_reference_line()
            .ok_or(ChartReferenceSelectionFailure::NoSelectedLine)?;
        let line = crate::chart_overlay::lines_for_state(self, &symbol)
            .get(index)
            .cloned()
            .ok_or(ChartReferenceSelectionFailure::StaleSelection)?;
        Ok((symbol, line))
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

    fn select_open_order(&mut self, index: usize) {
        let len = self
            .account_snapshot
            .as_ref()
            .map(|snapshot| snapshot.open_orders().len())
            .unwrap_or_default();
        if len == 0 {
            self.selected_open_order = 0;
        } else {
            self.selected_open_order = index.min(len - 1);
        }
    }

    fn select_watchlist_symbol(&mut self, index: usize) {
        self.select_symbol_index(index);
    }

    pub(super) fn select_symbol_index(&mut self, index: usize) {
        let before = self.selected_symbol;
        self.selected_symbol = if self.watchlist.is_empty() {
            0
        } else {
            index.min(self.watchlist.len() - 1)
        };
        if self.selected_symbol != before {
            self.chart.reset_view();
            self.normalize_chart_interval_for_selected_symbol();
        }
    }

    pub(super) fn normalize_chart_interval_for_selected_symbol(&mut self) {
        let Some(symbol) = self.selected_symbol().map(ToString::to_string) else {
            return;
        };
        if self
            .chart
            .normalize_interval_for(&symbol, self.providers.equity.provider())
        {
            self.task_log.info(format!(
                "chart interval reset to auto for {} on {}",
                symbol, self.providers.equity
            ));
        }
    }

    fn adjust_order_ticket_field_at(&mut self, index: usize, direction: isize) {
        self.order_ticket.select_field(index);
        self.order_ticket
            .adjust_selected_field(direction, self.selected_quote_price());
    }

    fn adjust_transfer_ticket_field_at(&mut self, index: usize, direction: isize) {
        self.transfer_ticket.select_field(index);
        self.transfer_ticket.adjust_selected_field(direction);
    }

    fn adjust_futures_state_ticket_field_at(&mut self, index: usize, direction: isize) {
        if !self.futures_state_ticket.select_field(index) {
            return;
        }
        let symbol = self.selected_symbol().map(ToString::to_string);
        self.futures_state_ticket
            .adjust_selected_field(direction, symbol.as_deref());
    }

    fn adjust_selected_setting(&mut self, direction: isize) {
        let row = self.settings_editor.selected();
        let Some(change) = self.edit_local_config(|state| {
            row.adjust(
                &mut state.locale,
                &mut state.providers,
                &mut state.theme,
                &mut state.keymap,
                direction,
            )
            .map(|change| LocalConfigEdit::new(change.section, change))
        }) else {
            return;
        };

        if change.requires_provider_reload {
            self.pending_provider_preferences_update = true;
            self.normalize_chart_interval_for_selected_symbol();
        }
        self.task_log.info(format!(
            "setting updated: {}={}",
            row.label(),
            row.value(&self.locale, &self.providers, &self.theme, &self.keymap)
        ));
    }

    fn adjust_setting_row(&mut self, index: usize, direction: isize) {
        self.settings_editor.select(index);
        self.adjust_selected_setting(direction);
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

    fn request_account_refresh(&mut self) {
        self.pending_account_refresh = true;
    }

    fn request_market_refresh(&mut self) {
        if self.refresh_loading() {
            self.task_log.info("market snapshot is already refreshing");
            return;
        }
        self.pending_market_refresh = true;
        self.task_log.info("market snapshot refresh requested");
    }

    fn request_symbol_data_refresh(&mut self, kind: SymbolTaskKind) {
        let label = symbol_task_label(kind);
        let Some(symbol) = self.selected_symbol().map(ToString::to_string) else {
            self.task_log
                .warning_event(format!("no selected symbol for {label} refresh"));
            return;
        };
        if kind == SymbolTaskKind::Evidence && !is_likely_crypto_pair(&symbol) {
            self.task_log.warning_event(format!(
                "crypto evidence refresh is only available for crypto pairs; selected {symbol}"
            ));
            return;
        }
        let already_loading = match kind {
            SymbolTaskKind::History => self.history.loading(),
            SymbolTaskKind::Evidence => self.evidence.loading(),
            SymbolTaskKind::Research => self.research.loading(),
        };
        if already_loading {
            self.task_log
                .info(format!("{label} for {symbol} is already loading"));
            return;
        }
        if !self.pending_symbol_data_refreshes.contains(&kind) {
            self.pending_symbol_data_refreshes.push(kind);
        }
        self.task_log
            .info(format!("{label} refresh requested for {symbol}"));
    }

    fn config_saved(&mut self) {
        self.config_changes.clear();
        self.config_undo_history.clear();
        self.pending_config_save = false;
        self.task_log.info("config saved".to_string());
    }

    fn config_save_failed(&mut self, error: String) {
        self.pending_config_save = false;
        self.task_log
            .warning_event(format!("config save failed: {error}"));
    }

    fn undo_config_change(&mut self) {
        let Some(snapshot) = self.config_undo_history.pop() else {
            self.task_log
                .info("no local config change to undo".to_string());
            return;
        };

        let provider_changed = self.providers != snapshot.config.providers;
        let trading_profile_changed =
            self.trading_profile != snapshot.config.trading.default_profile;
        self.restore_local_config_snapshot(snapshot);
        if provider_changed {
            self.invalidate_provider_backed_loads();
            self.pending_provider_preferences_update = true;
        }
        if trading_profile_changed {
            self.invalidate_account_snapshot_for_profile_change();
        }
        self.normalize_chart_interval_for_selected_symbol();
        self.task_log.info(if self.config_changes.is_empty() {
            "undid local config change; config is clean".to_string()
        } else {
            format!(
                "undid local config change; pending config: {}",
                self.config_changes.join(", ")
            )
        });
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
        self.edit_local_config(|state| {
            mutate(state);
            (state.tracked_layout_snapshot() != before)
                .then_some(LocalConfigEdit::new("layout", ()))
        });
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
        self.edit_local_config(|state| {
            for symbol in symbols {
                if let Some(index) = state
                    .watchlist
                    .iter()
                    .position(|candidate| candidate == &symbol)
                {
                    selected.get_or_insert(index);
                } else {
                    state.watchlist.push(symbol.clone());
                    added.push(symbol);
                    selected.get_or_insert(state.watchlist.len() - 1);
                }
            }
            (!added.is_empty()).then_some(LocalConfigEdit::new("watchlist", ()))
        });

        if let Some(index) = selected {
            self.select_symbol_index(index);
        }
        if added.is_empty() {
            self.task_log
                .info("watchlist already contains symbol".to_string());
        } else {
            self.task_log
                .info(format!("added {} to watchlist", added.join(", ")));
        }
        self.watchlist_add.reset();
        self.close_floating(FloatingKind::WatchlistAdd);
    }

    fn open_ticket_text_input(&mut self) {
        if self.selected_ticket_text_input().is_none() {
            self.task_log
                .warning_event("selected ticket field cannot be edited as text".to_string());
            return;
        }
        self.open_floating(FloatingKind::TicketTextInput);
    }

    fn accept_ticket_text_input(&mut self) {
        let target = self.ticket_text_input.target();
        let value = self.ticket_text_input.committed_value();
        let result = match target.kind() {
            TicketTextInputKind::Order => self.order_ticket.apply_text_input(target, value.clone()),
            TicketTextInputKind::Transfer => {
                self.transfer_ticket.apply_text_input(target, value.clone())
            }
            TicketTextInputKind::FuturesState => self
                .futures_state_ticket
                .apply_text_input(target, value.clone()),
        };
        if let Err(error) = result {
            self.task_log.warning_event(error);
            return;
        }
        self.task_log.info(format!(
            "updated {} {} to {}",
            target.ticket_label(),
            target.field_label(),
            value.as_deref().unwrap_or("blank")
        ));
        self.close_floating(FloatingKind::TicketTextInput);
    }

    pub(super) fn selected_ticket_text_input(
        &self,
    ) -> Option<(TicketTextInputTarget, Option<String>)> {
        match self.panels.focused() {
            Panel::OrderTicket => self
                .order_ticket
                .selected_text_input()
                .map(|(target, value)| (target, value.map(ToString::to_string))),
            Panel::TransferTicket => self
                .transfer_ticket
                .selected_text_input()
                .map(|(target, value)| (target, value.map(ToString::to_string))),
            Panel::FuturesState => self.futures_state_ticket.selected_text_input(),
            _ => None,
        }
    }

    fn delete_selected_watchlist_symbol(&mut self) {
        if self.watchlist.len() <= 1 {
            self.task_log
                .warning_event("watchlist must keep at least one symbol".to_string());
            return;
        }
        let removed = self
            .edit_local_config(|state| {
                let index = state.selected_symbol.min(state.watchlist.len() - 1);
                let removed = state.watchlist.remove(index);
                state.selected_symbol = index.min(state.watchlist.len() - 1);
                Some(LocalConfigEdit::new("watchlist", removed))
            })
            .expect("prevalidated watchlist delete must produce an edit");
        self.chart.reset_view();
        self.normalize_chart_interval_for_selected_symbol();
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
        self.edit_local_config(|state| {
            let next = next as usize;
            let symbol = state.watchlist.remove(current);
            state.watchlist.insert(next, symbol);
            state.selected_symbol = next;
            Some(LocalConfigEdit::new("watchlist", ()))
        });
        self.chart.reset_view();
        self.normalize_chart_interval_for_selected_symbol();
    }

    fn set_chart_preset(&mut self, preset: ChartPreset) {
        let Some((before, after)) = self.edit_local_config(|state| {
            let before = state.chart.preset();
            if !state.chart.set_preset(preset) {
                return None;
            }
            Some(LocalConfigEdit::new(
                "chart",
                (before, state.chart.preset()),
            ))
        }) else {
            return;
        };
        self.task_log
            .info(format!("chart preset changed from {before} to {after}"));
        self.request_symbol_data_refresh(SymbolTaskKind::History);
    }

    fn shift_chart_preset(&mut self, direction: isize) {
        let Some((before, after)) = self.edit_local_config(|state| {
            let before = state.chart.preset();
            if !state.chart.shift_preset(direction) {
                return None;
            }
            Some(LocalConfigEdit::new(
                "chart",
                (before, state.chart.preset()),
            ))
        }) else {
            return;
        };
        self.task_log
            .info(format!("chart preset changed from {before} to {after}"));
        self.request_symbol_data_refresh(SymbolTaskKind::History);
    }

    fn set_chart_interval(&mut self, interval: ChartInterval) {
        let Some(symbol) = self.selected_symbol().map(ToString::to_string) else {
            self.task_log
                .warning_event("no selected symbol for chart interval change".to_string());
            return;
        };
        if !interval.supported_for(&symbol, self.providers.equity.provider()) {
            self.task_log.warning_event(format!(
                "chart interval {interval} is not supported by {} for {symbol}",
                self.providers.equity
            ));
            return;
        }
        let before = self.chart.interval();
        if !self.chart.set_interval(interval) {
            return;
        }
        self.task_log.info(format!(
            "chart interval changed from {before} to {interval}"
        ));
        self.request_symbol_data_refresh(SymbolTaskKind::History);
    }

    fn set_chart_glyph_mode(&mut self, glyph_mode: ChartGlyphMode) {
        let before = self.chart.glyph_mode();
        if !self.chart.set_glyph_mode(glyph_mode) {
            return;
        }
        self.task_log
            .info(format!("chart glyph changed from {before} to {glyph_mode}"));
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
            Action::TrackMousePosition(position) => {
                self.mouse_position = position;
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
            Action::EditTicketTextInput(request) => {
                self.ticket_text_input.edit_query(request);
            }
            Action::EditStagedExecutionConfirmation(request) => {
                self.edit_staged_execution_confirmation(request);
            }
            Action::AcceptSymbolSearch => {
                if let Some(index) = self.symbol_search.selected_symbol_index() {
                    self.select_symbol_search_symbol(index);
                }
            }
            Action::SelectSymbolSearchSymbol(index) => self.select_symbol_search_symbol(index),
            Action::AcceptWatchlistAdd => self.add_watchlist_symbols(),
            Action::AcceptTradingProfile => self.accept_trading_profile(),
            Action::AcceptTicketTextInput => self.accept_ticket_text_input(),
            Action::SelectWatchlistSymbol(index) => self.select_watchlist_symbol(index),
            Action::DeleteSelectedWatchlistSymbol => self.delete_selected_watchlist_symbol(),
            Action::MoveSelectedWatchlistSymbol(direction) => {
                self.move_selected_watchlist_symbol(direction);
            }
            Action::MoveOrderTicketField(direction) => {
                self.order_ticket.move_field(direction);
            }
            Action::SelectOrderTicketField(index) => self.order_ticket.select_field(index),
            Action::AdjustOrderTicketField(direction) => {
                self.order_ticket
                    .adjust_selected_field(direction, self.selected_quote_price());
            }
            Action::AdjustOrderTicketFieldAt { index, direction } => {
                self.adjust_order_ticket_field_at(index, direction);
            }
            Action::CaptureOrderReferencePrice => self.capture_order_reference_price(),
            Action::CaptureChartPrice { price } => self.capture_chart_price(price),
            Action::CaptureSelectedChartReferencePrice => {
                self.capture_selected_chart_reference_price();
            }
            Action::CaptureSelectedChartReferenceAs(kind) => {
                self.capture_selected_chart_reference_as(kind);
            }
            Action::CaptureSelectedChartReferenceForProtectiveDraft(kind) => {
                self.capture_selected_chart_reference_for_protective_draft(kind);
            }
            Action::OpenTicketTextInput => self.open_ticket_text_input(),
            Action::MoveTransferTicketField(direction) => {
                self.transfer_ticket.move_field(direction);
            }
            Action::SelectTransferTicketField(index) => self.transfer_ticket.select_field(index),
            Action::AdjustTransferTicketField(direction) => {
                self.transfer_ticket.adjust_selected_field(direction);
            }
            Action::AdjustTransferTicketFieldAt { index, direction } => {
                self.adjust_transfer_ticket_field_at(index, direction);
            }
            Action::ApplyTransferTicketPreset(preset) => {
                let direction = preset.direction;
                let asset = preset.asset.clone();
                let amount = preset.amount.clone();
                self.transfer_ticket.apply_preset(preset);
                self.focus_panel(Panel::TransferTicket);
                self.task_log.info(format!(
                    "prepared transfer ticket {direction} {amount} {asset}"
                ));
            }
            Action::MoveFuturesStateTicketField(direction) => {
                self.futures_state_ticket.move_field(direction);
            }
            Action::SelectFuturesStateTicketField(index) => {
                self.futures_state_ticket.select_field(index);
            }
            Action::AdjustFuturesStateTicketField(direction) => {
                let symbol = self.selected_symbol().map(ToString::to_string);
                self.futures_state_ticket
                    .adjust_selected_field(direction, symbol.as_deref());
            }
            Action::AdjustFuturesStateTicketFieldAt { index, direction } => {
                self.adjust_futures_state_ticket_field_at(index, direction);
            }
            Action::ApplyFuturesStateTicketPreset(preset) => {
                let symbol = preset.symbol.clone();
                self.futures_state_ticket.apply_preset(preset);
                self.focus_panel(Panel::FuturesState);
                self.task_log
                    .info(format!("prepared futures state ticket for {symbol}"));
            }
            Action::MoveOpenOrderSelection(direction) => self.move_open_order_selection(direction),
            Action::SelectOpenOrder(index) => self.select_open_order(index),
            Action::MoveSettingsSelection(direction) => {
                self.settings_editor.move_selection(direction);
            }
            Action::SelectSettingRow(index) => self.settings_editor.select(index),
            Action::AdjustSelectedSetting(direction) => self.adjust_selected_setting(direction),
            Action::AdjustSettingRow { index, direction } => {
                self.adjust_setting_row(index, direction);
            }
            Action::StageOrderTicket => self.stage_order_ticket(),
            Action::StageTransferTicket => self.stage_transfer_ticket(),
            Action::StageFuturesStateTicket => self.stage_futures_state_ticket(),
            Action::StageSelectedOpenOrderCancel => self.stage_selected_open_order_cancel(),
            Action::RequestMarketRefresh => self.request_market_refresh(),
            Action::RequestSymbolDataRefresh(kind) => self.request_symbol_data_refresh(kind),
            Action::SetChartPreset(preset) => self.set_chart_preset(preset),
            Action::SetChartInterval(interval) => self.set_chart_interval(interval),
            Action::SetChartGlyphMode(glyph_mode) => self.set_chart_glyph_mode(glyph_mode),
            Action::ShiftChartPreset(direction) => self.shift_chart_preset(direction),
            Action::MoveChartCursor(direction) => self.chart.move_cursor(direction),
            Action::ZoomChartWindow(direction) => self.chart.zoom_window(direction),
            Action::MoveChartReferenceLine {
                direction,
                line_count,
            } => {
                self.chart.shift_reference_line(direction, line_count);
            }
            Action::ToggleChartOverlays => {
                let visible = self.chart.toggle_overlays();
                let status = if visible { "shown" } else { "hidden" };
                self.task_log.info(format!("chart overlays {status}"));
            }
            Action::SelectChartWindow { start_bps, end_bps } => {
                self.chart.select_window(start_bps, end_bps);
            }
            Action::ResetChartView => self.chart.reset_view(),
            Action::RequestAccountRefresh => self.request_account_refresh(),
            Action::MoveStagedChangeSelection(direction) => {
                self.staged_changes.move_selection(direction);
            }
            Action::SelectStagedChange(index) => self.staged_changes.select_visible(index),
            Action::ExecuteStagedChange => self.request_staged_change_confirmation(),
            Action::ConfirmStagedExecution => self.confirm_staged_execution(),
            Action::CancelStagedExecutionConfirmation => {
                self.cancel_staged_execution_confirmation()
            }
            Action::UndoConfigChange => self.undo_config_change(),
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
                    .is_some_and(|pane| pane.kind == FloatingKind::StagedExecutionConfirmation)
                {
                    self.cancel_staged_execution_confirmation();
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
            Action::ProfileValidationStarted {
                generation,
                profile,
            } => self.profile_validation_started(generation, profile),
            Action::ProfileValidationLoaded {
                generation,
                snapshot,
            } => self.profile_validation_loaded(generation, snapshot),
            Action::ProfileValidationFailed {
                generation,
                profile,
                error,
            } => self.profile_validation_failed(generation, profile, error),
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
                    self.cancel_staged_execution_confirmation();
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
            Action::ProfileRiskCommitSucceeded {
                id,
                snapshot,
                message,
            } => {
                self.apply_profile_risk_commit_success(id, snapshot, message);
            }
            Action::ProfileRiskCommitFailed { id, error } => {
                self.apply_profile_risk_commit_failure(id, error);
            }
            Action::Log(message) => self.task_log.info(message),
            Action::LogWarning(message) => self.task_log.warning_event(message),
        }
    }

    fn request_staged_change_confirmation(&mut self) {
        if self.pending_staged_confirmation.is_some() {
            self.open_floating(FloatingKind::StagedExecutionConfirmation);
            return;
        }
        match self.staged_changes.selected_execution_request() {
            QueueExecutionResult::Queued(request) => {
                self.task_log.info(format!(
                    "staged {} change {} awaiting execution confirmation",
                    request.summary(),
                    request.id
                ));
                self.pending_staged_confirmation = Some(PendingStagedConfirmation::new(request));
                self.open_floating(FloatingKind::StagedExecutionConfirmation);
            }
            QueueExecutionResult::Missing => self
                .task_log
                .warning_event("no selected staged change to execute".to_string()),
            QueueExecutionResult::Rejected { current } => self.task_log.warning_event(format!(
                "selected staged change cannot execute from current state {current}"
            )),
        }
    }

    fn confirm_staged_execution(&mut self) {
        let Some(pending) = self.pending_staged_confirmation.as_ref() else {
            self.task_log
                .warning_event("no staged execution confirmation is pending".to_string());
            self.close_floating(FloatingKind::StagedExecutionConfirmation);
            return;
        };
        if !pending.can_confirm() {
            if let Some(message) = pending.missing_confirmation_message() {
                self.task_log.warning_event(message);
            }
            self.open_floating(FloatingKind::StagedExecutionConfirmation);
            return;
        }
        let request = self
            .pending_staged_confirmation
            .take()
            .expect("pending staged confirmation was checked")
            .into_request();
        match self.staged_changes.queue_execution_request(&request) {
            QueueExecutionResult::Queued(request) => {
                self.task_log.info(format!(
                    "executing staged {} change {}",
                    request.summary(),
                    request.id
                ));
                self.pending_staged_execution = Some(request);
            }
            QueueExecutionResult::Missing => self
                .task_log
                .warning_event("pending staged execution confirmation disappeared".to_string()),
            QueueExecutionResult::Rejected { current } => self.task_log.warning_event(format!(
                "pending staged execution confirmation cannot execute from {current}"
            )),
        }
        self.close_floating(FloatingKind::StagedExecutionConfirmation);
    }

    fn cancel_staged_execution_confirmation(&mut self) {
        let Some(pending) = self.pending_staged_confirmation.take() else {
            self.close_floating(FloatingKind::StagedExecutionConfirmation);
            return;
        };
        let request = pending.request();
        self.task_log.info(format!(
            "cancelled staged execution confirmation for {}",
            request.id
        ));
        self.close_floating(FloatingKind::StagedExecutionConfirmation);
    }

    fn edit_staged_execution_confirmation(&mut self, request: tui_input::InputRequest) {
        if let Some(pending) = self.pending_staged_confirmation.as_mut() {
            pending.edit(request);
        }
    }

    fn apply_profile_risk_commit_success(
        &mut self,
        id: String,
        snapshot: ProfileValidationSnapshot,
        message: String,
    ) {
        self.reduce(Action::ApplyStagedChangeEvent {
            id,
            event: StagedChangeEvent::LocalCommitSucceeded,
        });
        if self.trading_profile.as_deref() == Some(snapshot.profile.as_str()) {
            self.profile_validation = ProfileValidationState::ready(snapshot);
        }
        self.task_log.info(message);
    }

    fn apply_profile_risk_commit_failure(&mut self, id: String, error: String) {
        self.reduce(Action::ApplyStagedChangeEvent {
            id,
            event: StagedChangeEvent::LocalCommitFailed,
        });
        self.task_log.warning_event(error);
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
    TrackMousePosition(Option<MousePosition>),
    MoveCommandSelection(isize),
    EditCommandQuery(tui_input::InputRequest),
    MoveSymbolSearchSelection(isize),
    EditSymbolSearchQuery(tui_input::InputRequest),
    EditWatchlistAddQuery(tui_input::InputRequest),
    EditTradingProfileQuery(tui_input::InputRequest),
    EditTicketTextInput(tui_input::InputRequest),
    EditStagedExecutionConfirmation(tui_input::InputRequest),
    AcceptSymbolSearch,
    SelectSymbolSearchSymbol(usize),
    AcceptWatchlistAdd,
    AcceptTradingProfile,
    AcceptTicketTextInput,
    SelectWatchlistSymbol(usize),
    DeleteSelectedWatchlistSymbol,
    MoveSelectedWatchlistSymbol(isize),
    MoveOrderTicketField(isize),
    SelectOrderTicketField(usize),
    AdjustOrderTicketField(isize),
    AdjustOrderTicketFieldAt {
        index: usize,
        direction: isize,
    },
    CaptureOrderReferencePrice,
    CaptureSelectedChartReferencePrice,
    CaptureSelectedChartReferenceAs(OrderKind),
    CaptureSelectedChartReferenceForProtectiveDraft(ProtectiveDraftSlot),
    CaptureChartPrice {
        price: f64,
    },
    OpenTicketTextInput,
    MoveTransferTicketField(isize),
    SelectTransferTicketField(usize),
    AdjustTransferTicketField(isize),
    AdjustTransferTicketFieldAt {
        index: usize,
        direction: isize,
    },
    ApplyTransferTicketPreset(crate::transfer_ticket::TransferTicketPreset),
    MoveFuturesStateTicketField(isize),
    SelectFuturesStateTicketField(usize),
    AdjustFuturesStateTicketField(isize),
    AdjustFuturesStateTicketFieldAt {
        index: usize,
        direction: isize,
    },
    ApplyFuturesStateTicketPreset(crate::futures_state_ticket::FuturesStateTicketPreset),
    MoveOpenOrderSelection(isize),
    SelectOpenOrder(usize),
    MoveSettingsSelection(isize),
    SelectSettingRow(usize),
    AdjustSelectedSetting(isize),
    AdjustSettingRow {
        index: usize,
        direction: isize,
    },
    StageOrderTicket,
    StageTransferTicket,
    StageFuturesStateTicket,
    StageSelectedOpenOrderCancel,
    RequestMarketRefresh,
    RequestSymbolDataRefresh(SymbolTaskKind),
    RequestAccountRefresh,
    MoveStagedChangeSelection(isize),
    SelectStagedChange(usize),
    ExecuteStagedChange,
    ConfirmStagedExecution,
    CancelStagedExecutionConfirmation,
    ProfileRiskCommitSucceeded {
        id: String,
        snapshot: ProfileValidationSnapshot,
        message: String,
    },
    ProfileRiskCommitFailed {
        id: String,
        error: String,
    },
    RequestConfigSave,
    UndoConfigChange,
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
    SetChartPreset(ChartPreset),
    SetChartInterval(ChartInterval),
    SetChartGlyphMode(ChartGlyphMode),
    ShiftChartPreset(isize),
    MoveChartCursor(isize),
    ZoomChartWindow(isize),
    MoveChartReferenceLine {
        direction: isize,
        line_count: usize,
    },
    ToggleChartOverlays,
    SelectChartWindow {
        start_bps: u16,
        end_bps: u16,
    },
    ResetChartView,
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
    ProfileValidationStarted {
        generation: u64,
        profile: String,
    },
    ProfileValidationLoaded {
        generation: u64,
        snapshot: ProfileValidationSnapshot,
    },
    ProfileValidationFailed {
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
    LogWarning(String),
}

#[cfg(test)]
mod tests;
