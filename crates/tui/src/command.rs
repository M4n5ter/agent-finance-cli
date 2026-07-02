use std::borrow::Cow;
use std::sync::LazyLock;

use tui_input::InputRequest;

use agent_finance_core::OrderKind;

use crate::chart::{ChartGlyphMode, ChartInterval, ChartPreset};
use crate::model::{FloatingKind, Panel, WorkspaceKind};
use crate::order_ticket::ProtectiveDraftSlot;
use crate::search::{SearchListState, fuzzy_indices};

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    list: SearchListState,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            list: SearchListState::with_matches(all_command_indices()),
        }
    }
}

impl CommandPaletteState {
    pub fn query(&self) -> &str {
        self.list.query()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn selected(&self) -> usize {
        self.list.selected()
    }

    pub fn command_at(&self, index: usize) -> Option<CommandSpec> {
        self.list
            .index_at(index)
            .and_then(|command| ACTION_REGISTRY[command].command())
    }

    pub fn shift(&mut self, direction: isize) {
        self.list.shift(direction);
    }

    pub fn selected_command(&self) -> Option<CommandSpec> {
        self.command_at(self.selected())
    }

    pub fn selected_action(&self) -> Option<ActionId> {
        self.selected_command().map(|command| command.action)
    }

    pub fn reset(&mut self) {
        self.list.reset(all_command_indices());
    }

    pub fn edit_query(&mut self, request: InputRequest) {
        self.list.edit_query(request, command_indices_for_query);
    }
}

fn all_command_indices() -> Vec<usize> {
    ACTION_REGISTRY
        .iter()
        .enumerate()
        .filter_map(|(index, action)| action.command().map(|_| index))
        .collect()
}

fn fuzzy_command_indices(query: &str) -> Vec<usize> {
    fuzzy_indices(query, 0..ACTION_REGISTRY.len(), |index| {
        let command = ACTION_REGISTRY[index].command()?;
        Some(format!("{} {}", command.title, command.description))
    })
}

fn command_indices_for_query(query: &str) -> Vec<usize> {
    if query.is_empty() {
        all_command_indices()
    } else {
        fuzzy_command_indices(query)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandSpec {
    pub id: Cow<'static, str>,
    pub title: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub action: ActionId,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActionId {
    SelectSymbolBy(isize),
    OpenFloating(FloatingKind),
    CloseFocusedFloating,
    ResetLayout,
    CloseFocusedPanel,
    RestorePanels,
    FocusPanelBy(isize),
    ToggleFocusedZoom,
    ShiftWorkspace(isize),
    SetWorkspace(WorkspaceKind),
    FocusPanel(Panel),
    TogglePanel(Panel),
    ToggleLiveWrites,
    RefreshMarketSnapshot,
    RefreshSelectedHistory,
    RefreshSelectedEvidence,
    RefreshSelectedResearch,
    SetChartPreset(ChartPreset),
    SetChartInterval(ChartInterval),
    SetChartGlyphMode(ChartGlyphMode),
    ShiftChartPreset(isize),
    ResetChartView,
    ToggleChartOverlays,
    CaptureOrderReferencePrice,
    CaptureSelectedChartReferencePrice,
    CaptureSelectedChartReferenceAs(OrderKind),
    CaptureSelectedChartReferenceForProtectiveDraft(ProtectiveDraftSlot),
    OpenTicketTextInput,
    StageOrderTicket,
    StageTransferTicket,
    StageFuturesStateTicket,
    StageSelectedOpenOrderCancel,
    ExecuteStagedChange,
    RefreshAccountSnapshot,
    RevalidateTradingProfile,
    StageProfileLiveToggle,
    SaveConfig,
    UndoConfigChange,
    DeleteSelectedWatchlistSymbol,
    MoveSelectedWatchlistSymbol(isize),
    CloseCommandPalette,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActionSpec {
    pub id: Cow<'static, str>,
    pub action: ActionId,
    command: Option<CommandPresentation>,
}

impl ActionSpec {
    pub fn command(&self) -> Option<CommandSpec> {
        self.command.as_ref().map(|command| CommandSpec {
            id: self.id.clone(),
            title: command.title.clone(),
            description: command.description.clone(),
            action: self.action,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CommandPresentation {
    title: Cow<'static, str>,
    description: Cow<'static, str>,
}

pub fn action_by_id(id: &str) -> Option<ActionId> {
    ACTION_REGISTRY
        .iter()
        .find(|spec| spec.id == id)
        .map(|spec| spec.action)
}

pub fn action_id(action: ActionId) -> Option<&'static str> {
    ACTION_REGISTRY
        .iter()
        .find(|spec| spec.action == action)
        .map(|spec| spec.id.as_ref())
}

macro_rules! action {
    ($id:expr, $action:expr, $title:expr, $description:expr) => {
        ActionSpec {
            id: Cow::Borrowed($id),
            action: $action,
            command: Some(CommandPresentation {
                title: Cow::Borrowed($title),
                description: Cow::Borrowed($description),
            }),
        }
    };
}

fn chart_preset_action(preset: ChartPreset) -> ActionSpec {
    ActionSpec {
        id: Cow::Owned(preset.command_id()),
        action: ActionId::SetChartPreset(preset),
        command: Some(CommandPresentation {
            title: Cow::Owned(preset.command_title()),
            description: Cow::Borrowed(preset.command_description()),
        }),
    }
}

fn chart_interval_action(interval: ChartInterval) -> ActionSpec {
    ActionSpec {
        id: Cow::Owned(format!("chart-interval-{}", interval.label())),
        action: ActionId::SetChartInterval(interval),
        command: Some(CommandPresentation {
            title: Cow::Owned(format!(
                "Chart interval {}",
                interval.label().to_ascii_uppercase()
            )),
            description: Cow::Borrowed("Override the selected history chart preset interval"),
        }),
    }
}

fn chart_glyph_mode_action(glyph_mode: ChartGlyphMode) -> ActionSpec {
    ActionSpec {
        id: Cow::Owned(format!("chart-glyph-{}", glyph_mode.label())),
        action: ActionId::SetChartGlyphMode(glyph_mode),
        command: Some(CommandPresentation {
            title: Cow::Owned(format!(
                "Chart glyph {}",
                glyph_mode.label().to_ascii_uppercase()
            )),
            description: Cow::Borrowed("Switch the history chart terminal glyph rendering mode"),
        }),
    }
}

pub static ACTION_REGISTRY: LazyLock<Vec<ActionSpec>> = LazyLock::new(|| {
    let mut actions = vec![
        action!(
            "select-next-symbol",
            ActionId::SelectSymbolBy(1),
            "Next symbol",
            "Move watchlist selection to the next symbol"
        ),
        action!(
            "select-previous-symbol",
            ActionId::SelectSymbolBy(-1),
            "Previous symbol",
            "Move watchlist selection to the previous symbol"
        ),
        action!(
            "open-help",
            ActionId::OpenFloating(FloatingKind::Help),
            "Open help",
            "Show cockpit shortcuts and interaction model"
        ),
        action!(
            "open-provider-details",
            ActionId::OpenFloating(FloatingKind::ProviderDetails),
            "Open provider details",
            "Inspect provider capability coverage"
        ),
        action!(
            "open-command-palette",
            ActionId::OpenFloating(FloatingKind::CommandPalette),
            "Open command palette",
            "Search and execute cockpit actions"
        ),
        action!(
            "open-symbol-search",
            ActionId::OpenFloating(FloatingKind::SymbolSearch),
            "Open symbol search",
            "Filter the watchlist and jump to a symbol"
        ),
        action!(
            "open-watchlist-add",
            ActionId::OpenFloating(FloatingKind::WatchlistAdd),
            "Add symbols",
            "Add comma-separated symbols to the watchlist"
        ),
        action!(
            "close-floating",
            ActionId::CloseFocusedFloating,
            "Close overlay",
            "Close the top floating overlay"
        ),
        action!(
            "reset-layout",
            ActionId::ResetLayout,
            "Reset layout",
            "Restore default docked columns and close overlays"
        ),
        action!(
            "close-focused-panel",
            ActionId::CloseFocusedPanel,
            "Close focused panel",
            "Hide the focused docked panel and move focus to another open panel"
        ),
        action!(
            "restore-panels",
            ActionId::RestorePanels,
            "Restore all panels",
            "Reopen every docked panel without changing the current symbol"
        ),
        action!(
            "next-pane",
            ActionId::FocusPanelBy(1),
            "Next pane",
            "Move focus to the next workspace pane"
        ),
        action!(
            "previous-pane",
            ActionId::FocusPanelBy(-1),
            "Previous pane",
            "Move focus to the previous workspace pane"
        ),
        action!(
            "toggle-pane-zoom",
            ActionId::ToggleFocusedZoom,
            "Toggle pane zoom",
            "Expand the focused docked pane or restore the workspace layout"
        ),
        action!(
            "toggle-live-writes",
            ActionId::ToggleLiveWrites,
            "Toggle live writes",
            "Enable live writes after confirmation or disable them for this session"
        ),
        action!(
            "capture-order-reference-price",
            ActionId::CaptureOrderReferencePrice,
            "Capture order reference price",
            "Copy the current selected quote price into the order ticket price field"
        ),
        action!(
            "edit-ticket-text-field",
            ActionId::OpenTicketTextInput,
            "Edit ticket text field",
            "Open text input for the selected ticket field when it supports direct text input"
        ),
        action!(
            "stage-order-ticket",
            ActionId::StageOrderTicket,
            "Stage order ticket",
            "Move the current valid order ticket into intent review"
        ),
        action!(
            "stage-transfer-ticket",
            ActionId::StageTransferTicket,
            "Stage transfer ticket",
            "Move the current valid transfer ticket into intent review"
        ),
        action!(
            "stage-futures-state-ticket",
            ActionId::StageFuturesStateTicket,
            "Stage futures state ticket",
            "Move the current valid USD-M futures state ticket into intent review"
        ),
        action!(
            "stage-selected-open-order-cancel",
            ActionId::StageSelectedOpenOrderCancel,
            "Stage selected cancel",
            "Move the selected open order into intent review as a cancel"
        ),
        action!(
            "execute-staged-change",
            ActionId::ExecuteStagedChange,
            "Execute staged change",
            "Review the selected ready staged change before provider submit or local profile commit"
        ),
        action!(
            "refresh-account-snapshot",
            ActionId::RefreshAccountSnapshot,
            "Refresh account snapshot",
            "Reload signed account balances, positions, open orders, and transfer history"
        ),
        action!(
            "open-trading-profile-editor",
            ActionId::OpenFloating(FloatingKind::TradingProfile),
            "Set trading profile",
            "Edit the default trading profile used by order, cancel, transfer, and futures state tickets"
        ),
        action!(
            "revalidate-trading-profile",
            ActionId::RevalidateTradingProfile,
            "Revalidate trading profile",
            "Reload and validate the selected trading profile from disk"
        ),
        action!(
            "stage-profile-live-toggle",
            ActionId::StageProfileLiveToggle,
            "Stage profile live toggle",
            "Review a risk.allow_live change for the selected trading profile"
        ),
        action!(
            "save-config",
            ActionId::SaveConfig,
            "Save config",
            "Persist pending local TUI configuration changes"
        ),
        action!(
            "undo-config-change",
            ActionId::UndoConfigChange,
            "Undo config change",
            "Revert the latest local TUI configuration edit"
        ),
        action!(
            "delete-selected-watchlist-symbol",
            ActionId::DeleteSelectedWatchlistSymbol,
            "Delete selected symbol",
            "Remove the selected symbol from the persisted watchlist"
        ),
        action!(
            "move-selected-watchlist-symbol-up",
            ActionId::MoveSelectedWatchlistSymbol(-1),
            "Move selected symbol up",
            "Reorder the selected watchlist symbol upward"
        ),
        action!(
            "move-selected-watchlist-symbol-down",
            ActionId::MoveSelectedWatchlistSymbol(1),
            "Move selected symbol down",
            "Reorder the selected watchlist symbol downward"
        ),
        action!(
            "next-workspace",
            ActionId::ShiftWorkspace(1),
            "Next workspace",
            "Move to the next workspace tab"
        ),
        action!(
            "previous-workspace",
            ActionId::ShiftWorkspace(-1),
            "Previous workspace",
            "Move to the previous workspace tab"
        ),
        action!(
            WorkspaceKind::Market.command_id(),
            ActionId::SetWorkspace(WorkspaceKind::Market),
            WorkspaceKind::Market.command_title(),
            WorkspaceKind::Market.command_description()
        ),
        action!(
            WorkspaceKind::Trade.command_id(),
            ActionId::SetWorkspace(WorkspaceKind::Trade),
            WorkspaceKind::Trade.command_title(),
            WorkspaceKind::Trade.command_description()
        ),
        action!(
            WorkspaceKind::Account.command_id(),
            ActionId::SetWorkspace(WorkspaceKind::Account),
            WorkspaceKind::Account.command_title(),
            WorkspaceKind::Account.command_description()
        ),
        action!(
            WorkspaceKind::Research.command_id(),
            ActionId::SetWorkspace(WorkspaceKind::Research),
            WorkspaceKind::Research.command_title(),
            WorkspaceKind::Research.command_description()
        ),
        action!(
            WorkspaceKind::Settings.command_id(),
            ActionId::SetWorkspace(WorkspaceKind::Settings),
            WorkspaceKind::Settings.command_title(),
            WorkspaceKind::Settings.command_description()
        ),
        action!(
            "refresh-market-snapshot",
            ActionId::RefreshMarketSnapshot,
            "Refresh market snapshot",
            "Reload watchlist quotes and market session data now"
        ),
        action!(
            "refresh-selected-history",
            ActionId::RefreshSelectedHistory,
            "Refresh selected history",
            "Reload historical bars for the selected symbol now"
        ),
        action!(
            "chart-next-preset",
            ActionId::ShiftChartPreset(1),
            "Chart next preset",
            "Move the history chart to the next time range preset"
        ),
        action!(
            "chart-previous-preset",
            ActionId::ShiftChartPreset(-1),
            "Chart previous preset",
            "Move the history chart to the previous time range preset"
        ),
        action!(
            "chart-reset-view",
            ActionId::ResetChartView,
            "Chart reset view",
            "Reset the history chart cursor and zoom window"
        ),
        action!(
            "chart-toggle-overlays",
            ActionId::ToggleChartOverlays,
            "Chart toggle overlays",
            "Show or hide history chart price, order, and position reference lines"
        ),
        action!(
            "chart-copy-selected-price",
            ActionId::CaptureSelectedChartReferencePrice,
            "Chart copy selected price",
            "Copy the selected history chart reference line price into the order ticket"
        ),
        action!(
            "chart-draft-stop-loss",
            ActionId::CaptureSelectedChartReferenceAs(OrderKind::StopLoss),
            "Chart draft stop loss",
            "Prepare a stop-loss order ticket from the selected chart reference line"
        ),
        action!(
            "chart-draft-take-profit",
            ActionId::CaptureSelectedChartReferenceAs(OrderKind::TakeProfit),
            "Chart draft take profit",
            "Prepare a take-profit order ticket from the selected chart reference line"
        ),
        action!(
            "chart-oco-stop-loss",
            ActionId::CaptureSelectedChartReferenceForProtectiveDraft(
                ProtectiveDraftSlot::StopLoss
            ),
            "Chart OCO stop loss",
            "Add the selected chart reference line to the protective OCO draft as stop loss"
        ),
        action!(
            "chart-oco-take-profit",
            ActionId::CaptureSelectedChartReferenceForProtectiveDraft(
                ProtectiveDraftSlot::TakeProfit
            ),
            "Chart OCO take profit",
            "Add the selected chart reference line to the protective OCO draft as take profit"
        ),
        action!(
            "refresh-selected-evidence",
            ActionId::RefreshSelectedEvidence,
            "Refresh selected evidence",
            "Reload crypto evidence for the selected symbol now"
        ),
        action!(
            "refresh-selected-research",
            ActionId::RefreshSelectedResearch,
            "Refresh selected research",
            "Reload research context and prediction signals for the selected symbol now"
        ),
    ];
    actions.extend(ChartPreset::ALL.map(chart_preset_action));
    actions.extend(ChartInterval::ALL.map(chart_interval_action));
    actions.extend(ChartGlyphMode::ALL.map(chart_glyph_mode_action));
    actions.extend(panel_action_specs());
    actions.push(action!(
        "close-command-palette",
        ActionId::CloseCommandPalette,
        "Close command palette",
        "Dismiss this command palette without changing docked panels"
    ));
    actions
});

fn panel_action_specs() -> Vec<ActionSpec> {
    Panel::ALL
        .into_iter()
        .flat_map(|panel| {
            [
                ActionSpec {
                    id: panel_command_id("focus", panel),
                    action: ActionId::FocusPanel(panel),
                    command: Some(CommandPresentation {
                        title: Cow::Borrowed(panel.focus_command_title()),
                        description: Cow::Borrowed(panel.focus_command_description()),
                    }),
                },
                ActionSpec {
                    id: panel_command_id("toggle", panel),
                    action: ActionId::TogglePanel(panel),
                    command: Some(CommandPresentation {
                        title: Cow::Borrowed(panel.toggle_command_title()),
                        description: Cow::Borrowed(panel.toggle_command_description()),
                    }),
                },
            ]
        })
        .collect()
}

fn panel_command_id(prefix: &'static str, panel: Panel) -> Cow<'static, str> {
    Cow::Owned(format!("{}-{}", prefix, panel.command_slug()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_palette_fuzzy_search_filters_and_ranks_actions() {
        let mut palette = CommandPaletteState::default();

        for character in "provider".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        let visible = (0..palette.len())
            .filter_map(|index| palette.command_at(index))
            .map(|command| command.title.into_owned())
            .collect::<Vec<_>>();
        assert!(visible.contains(&"Open provider details".to_string()));
        assert!(visible.contains(&"Workspace market".to_string()));
    }

    #[test]
    fn command_palette_can_open_trading_profile_editor() {
        let mut palette = CommandPaletteState::default();

        for character in "trading profile".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::OpenFloating(FloatingKind::TradingProfile))
        );
    }

    #[test]
    fn command_palette_can_revalidate_trading_profile() {
        let mut palette = CommandPaletteState::default();

        for character in "revalidate profile".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::RevalidateTradingProfile)
        );
    }

    #[test]
    fn command_palette_can_stage_profile_live_toggle() {
        let mut palette = CommandPaletteState::default();

        for character in "profile live toggle".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::StageProfileLiveToggle)
        );
    }

    #[test]
    fn command_palette_can_capture_order_reference_price() {
        let mut palette = CommandPaletteState::default();

        for character in "capture order price".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureOrderReferencePrice)
        );
    }

    #[test]
    fn command_palette_can_control_chart_view() {
        let mut palette = CommandPaletteState::default();

        for character in "chart reset view".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(palette.selected_action(), Some(ActionId::ResetChartView));

        palette.reset();
        for character in "toggle chart overlays".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::ToggleChartOverlays)
        );
    }

    #[test]
    fn command_palette_can_copy_selected_chart_price() {
        let mut palette = CommandPaletteState::default();

        for character in "chart copy selected price".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureSelectedChartReferencePrice)
        );
    }

    #[test]
    fn command_palette_can_prepare_protective_chart_drafts() {
        let mut palette = CommandPaletteState::default();

        for character in "chart draft stop loss".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureSelectedChartReferenceAs(
                OrderKind::StopLoss
            ))
        );

        palette.reset();
        for character in "chart draft take profit".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureSelectedChartReferenceAs(
                OrderKind::TakeProfit
            ))
        );

        palette.reset();
        for character in "chart oco stop loss".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureSelectedChartReferenceForProtectiveDraft(
                ProtectiveDraftSlot::StopLoss
            ))
        );

        palette.reset();
        for character in "chart oco take profit".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CaptureSelectedChartReferenceForProtectiveDraft(
                ProtectiveDraftSlot::TakeProfit
            ))
        );
    }

    #[test]
    fn command_palette_can_open_ticket_text_input() {
        let mut palette = CommandPaletteState::default();

        for character in "edit ticket text".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::OpenTicketTextInput)
        );
    }

    #[test]
    fn command_palette_can_route_to_settings_workspace() {
        let mut palette = CommandPaletteState::default();

        for character in "settings".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::SetWorkspace(WorkspaceKind::Settings))
        );
        let visible = (0..palette.len())
            .filter_map(|index| palette.command_at(index))
            .map(|command| command.title.into_owned())
            .collect::<Vec<_>>();
        assert!(visible.contains(&"Focus settings".to_string()));
        assert!(visible.contains(&"Toggle settings".to_string()));
    }

    #[test]
    fn command_palette_can_find_config_undo_action() {
        let mut palette = CommandPaletteState::default();

        for character in "undo config".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(palette.selected_action(), Some(ActionId::UndoConfigChange));
    }

    #[test]
    fn command_palette_can_find_market_data_refresh_actions() {
        for (query, expected) in [
            ("refresh market", ActionId::RefreshMarketSnapshot),
            ("refresh selected history", ActionId::RefreshSelectedHistory),
            (
                "refresh selected evidence",
                ActionId::RefreshSelectedEvidence,
            ),
            (
                "refresh selected research",
                ActionId::RefreshSelectedResearch,
            ),
        ] {
            let mut palette = CommandPaletteState::default();

            for character in query.chars() {
                palette.edit_query(InputRequest::InsertChar(character));
            }

            assert_eq!(palette.selected_action(), Some(expected), "{query}");
        }
    }

    #[test]
    fn command_palette_can_find_chart_interval_actions() {
        let mut palette = CommandPaletteState::default();

        for character in "chart interval 15m".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::SetChartInterval(ChartInterval::FifteenMinutes))
        );
    }

    #[test]
    fn command_palette_can_find_chart_glyph_actions() {
        let mut palette = CommandPaletteState::default();

        for character in "chart glyph readable".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::SetChartGlyphMode(ChartGlyphMode::Readable))
        );
    }

    #[test]
    fn command_palette_selection_stays_in_filtered_bounds() {
        let mut palette = CommandPaletteState::default();

        palette.shift(-1);
        assert_eq!(
            palette.selected_action(),
            Some(ActionId::CloseCommandPalette)
        );
        palette.edit_query(InputRequest::InsertChar('z'));
        palette.edit_query(InputRequest::InsertChar('z'));
        palette.edit_query(InputRequest::InsertChar('z'));

        assert_eq!(palette.len(), 0);
        assert_eq!(palette.selected(), 0);
        assert_eq!(palette.selected_action(), None);
    }

    #[test]
    fn command_palette_query_changes_select_top_ranked_match() {
        let mut palette = CommandPaletteState::default();

        palette.shift(-1);
        for character in "provider".chars() {
            palette.edit_query(InputRequest::InsertChar(character));
        }

        assert_eq!(
            palette.selected_action(),
            Some(ActionId::OpenFloating(FloatingKind::ProviderDetails))
        );
    }

    #[test]
    fn action_registry_keeps_stable_unique_ids_for_palette_actions() {
        let mut ids = ACTION_REGISTRY
            .iter()
            .map(|spec| spec.id.as_ref())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), ACTION_REGISTRY.len());
        for command in ACTION_REGISTRY.iter().filter_map(|action| action.command()) {
            assert!(action_id(command.action).is_some());
        }
    }
}
