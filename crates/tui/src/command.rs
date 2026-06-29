use std::borrow::Cow;
use std::sync::LazyLock;

use tui_input::InputRequest;

use crate::model::{FloatingKind, Panel, WorkspaceKind};
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CommandSpec {
    pub title: &'static str,
    pub description: &'static str,
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
    CaptureOrderReferencePrice,
    OpenTicketTextInput,
    StageOrderTicket,
    StageTransferTicket,
    StageFuturesStateTicket,
    StageSelectedOpenOrderCancel,
    ExecuteStagedChange,
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
        self.command.map(|command| CommandSpec {
            title: command.title,
            description: command.description,
            action: self.action,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct CommandPresentation {
    title: &'static str,
    description: &'static str,
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
                title: $title,
                description: $description,
            }),
        }
    };
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
    ];
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
                        title: panel.focus_command_title(),
                        description: panel.focus_command_description(),
                    }),
                },
                ActionSpec {
                    id: panel_command_id("toggle", panel),
                    action: ActionId::TogglePanel(panel),
                    command: Some(CommandPresentation {
                        title: panel.toggle_command_title(),
                        description: panel.toggle_command_description(),
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
            .map(|command| command.title)
            .collect::<Vec<_>>();
        assert!(visible.contains(&"Open provider details"));
        assert!(visible.contains(&"Workspace market"));
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
            .map(|command| command.title)
            .collect::<Vec<_>>();
        assert!(visible.contains(&"Focus settings"));
        assert!(visible.contains(&"Toggle settings"));
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
