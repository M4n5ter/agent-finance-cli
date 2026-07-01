use crate::command::ActionId;
use crate::model::{FloatingKind, Panel, WorkspaceKind};
use crate::scheduler::SymbolTaskKind;
use crate::state::{Action, AppState};

impl AppState {
    pub(super) fn execute(&mut self, action: ActionId) {
        match action {
            ActionId::SelectSymbolBy(direction) => {
                self.close_text_input_floatings();
                self.shift_symbol(direction);
            }
            ActionId::OpenFloating(kind) => {
                if kind.text_input() {
                    self.close_text_input_floatings_except(kind);
                } else {
                    self.close_text_input_floatings();
                }
                self.open_floating(kind);
            }
            ActionId::CloseFocusedFloating => {
                self.reduce(Action::CloseFocusedFloating);
            }
            ActionId::ResetLayout => {
                self.reduce(Action::ResetLayout);
            }
            ActionId::FocusPanel(panel) => self.focus_panel_from_command(panel),
            ActionId::TogglePanel(panel) => self.toggle_panel_from_command(panel),
            ActionId::ShiftWorkspace(direction) => {
                self.close_text_input_floatings();
                self.reduce(Action::ShiftWorkspace(direction));
            }
            ActionId::SetWorkspace(workspace) => self.set_workspace_from_command(workspace),
            ActionId::CloseFocusedPanel => {
                self.close_text_input_floatings();
                self.reduce(Action::CloseFocusedPanel);
            }
            ActionId::RestorePanels => {
                self.close_text_input_floatings();
                self.reduce(Action::RestorePanels);
            }
            ActionId::FocusPanelBy(direction) => {
                self.close_text_input_floatings();
                self.reduce(Action::FocusPanelBy(direction));
            }
            ActionId::ToggleFocusedZoom => {
                self.close_text_input_floatings();
                self.reduce(Action::ToggleFocusedZoom);
            }
            ActionId::ToggleLiveWrites => {
                self.close_text_input_floatings();
                if self.live_writes_enabled {
                    self.reduce(Action::SetLiveWritesEnabled(false));
                } else {
                    self.open_floating(FloatingKind::LiveWritesConfirmation);
                }
            }
            ActionId::RefreshMarketSnapshot => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestMarketRefresh);
            }
            ActionId::RefreshSelectedHistory => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestSymbolDataRefresh(SymbolTaskKind::History));
            }
            ActionId::SetChartPreset(preset) => {
                self.close_text_input_floatings();
                self.reduce(Action::SetChartPreset(preset));
            }
            ActionId::SetChartInterval(interval) => {
                self.close_text_input_floatings();
                self.reduce(Action::SetChartInterval(interval));
            }
            ActionId::ShiftChartPreset(direction) => {
                self.close_text_input_floatings();
                self.reduce(Action::ShiftChartPreset(direction));
            }
            ActionId::ResetChartView => {
                self.close_text_input_floatings();
                self.reduce(Action::ResetChartView);
            }
            ActionId::ToggleChartOverlays => {
                self.close_text_input_floatings();
                self.reduce(Action::ToggleChartOverlays);
            }
            ActionId::RefreshSelectedEvidence => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestSymbolDataRefresh(SymbolTaskKind::Evidence));
            }
            ActionId::RefreshSelectedResearch => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestSymbolDataRefresh(SymbolTaskKind::Research));
            }
            ActionId::CaptureOrderReferencePrice => {
                self.close_text_input_floatings();
                self.reduce(Action::CaptureOrderReferencePrice);
            }
            ActionId::OpenTicketTextInput => {
                self.close_text_input_floatings();
                self.reduce(Action::OpenTicketTextInput);
            }
            ActionId::StageOrderTicket => {
                self.close_text_input_floatings();
                self.reduce(Action::StageOrderTicket);
            }
            ActionId::StageTransferTicket => {
                self.close_text_input_floatings();
                self.reduce(Action::StageTransferTicket);
            }
            ActionId::StageFuturesStateTicket => {
                self.close_text_input_floatings();
                self.reduce(Action::StageFuturesStateTicket);
            }
            ActionId::StageSelectedOpenOrderCancel => {
                self.close_text_input_floatings();
                self.reduce(Action::StageSelectedOpenOrderCancel);
            }
            ActionId::ExecuteStagedChange => {
                self.close_text_input_floatings();
                self.reduce(Action::ExecuteStagedChange);
            }
            ActionId::RefreshAccountSnapshot => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestAccountRefresh);
            }
            ActionId::RevalidateTradingProfile => {
                self.close_text_input_floatings();
                self.revalidate_trading_profile();
            }
            ActionId::StageProfileLiveToggle => {
                self.close_text_input_floatings();
                self.stage_profile_live_toggle();
            }
            ActionId::SaveConfig => {
                self.close_text_input_floatings();
                self.reduce(Action::RequestConfigSave);
            }
            ActionId::UndoConfigChange => {
                self.close_text_input_floatings();
                self.reduce(Action::UndoConfigChange);
            }
            ActionId::DeleteSelectedWatchlistSymbol => {
                self.close_text_input_floatings();
                self.reduce(Action::DeleteSelectedWatchlistSymbol);
            }
            ActionId::MoveSelectedWatchlistSymbol(direction) => {
                self.close_text_input_floatings();
                self.reduce(Action::MoveSelectedWatchlistSymbol(direction));
            }
            ActionId::CloseCommandPalette => {
                self.close_floating(FloatingKind::CommandPalette);
            }
        }
    }

    fn focus_panel_from_command(&mut self, panel: Panel) {
        self.close_text_input_floatings();
        self.reduce(Action::Focus(panel));
    }

    fn toggle_panel_from_command(&mut self, panel: Panel) {
        self.close_text_input_floatings();
        self.toggle_panel(panel);
    }

    fn set_workspace_from_command(&mut self, workspace: WorkspaceKind) {
        self.close_text_input_floatings();
        self.reduce(Action::SetWorkspace(workspace));
    }
}
