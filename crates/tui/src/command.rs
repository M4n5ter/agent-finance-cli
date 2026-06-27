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
    StageOrderTicket,
    StageTransferTicket,
    StageFuturesStateTicket,
    StageSelectedOpenOrderCancel,
    SubmitStagedChange,
    SaveConfig,
    DeleteSelectedWatchlistSymbol,
    MoveSelectedWatchlistSymbol(isize),
    CloseCommandPalette,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ActionSpec {
    pub id: &'static str,
    pub action: ActionId,
    command: Option<CommandPresentation>,
}

impl ActionSpec {
    pub const fn command(self) -> Option<CommandSpec> {
        match self.command {
            Some(command) => Some(CommandSpec {
                title: command.title,
                description: command.description,
                action: self.action,
            }),
            None => None,
        }
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
        .map(|spec| spec.id)
}

macro_rules! action {
    ($id:literal, $action:expr, $title:literal, $description:literal) => {
        ActionSpec {
            id: $id,
            action: $action,
            command: Some(CommandPresentation {
                title: $title,
                description: $description,
            }),
        }
    };
}

pub const ACTION_REGISTRY: [ActionSpec; 52] = [
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
        "submit-staged-change",
        ActionId::SubmitStagedChange,
        "Submit staged change",
        "Review the selected ready staged change in a confirmation modal before trading runtime submit"
    ),
    action!(
        "open-trading-profile-editor",
        ActionId::OpenFloating(FloatingKind::TradingProfile),
        "Set trading profile",
        "Edit the default trading profile used by order, cancel, transfer, and futures state tickets"
    ),
    action!(
        "save-config",
        ActionId::SaveConfig,
        "Save config",
        "Persist pending local TUI configuration changes"
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
        "workspace-overview",
        ActionId::SetWorkspace(WorkspaceKind::Overview),
        "Workspace overview",
        "Show the overview cockpit workspace"
    ),
    action!(
        "workspace-research",
        ActionId::SetWorkspace(WorkspaceKind::Research),
        "Workspace research",
        "Show news, research, and prediction-market context"
    ),
    action!(
        "workspace-crypto",
        ActionId::SetWorkspace(WorkspaceKind::Crypto),
        "Workspace crypto",
        "Show crypto evidence and market context"
    ),
    action!(
        "workspace-providers",
        ActionId::SetWorkspace(WorkspaceKind::Providers),
        "Workspace providers",
        "Show provider health and runtime task status"
    ),
    action!(
        "focus-watchlist",
        ActionId::FocusPanel(Panel::Watchlist),
        "Focus watchlist",
        "Move keyboard focus to the symbol list"
    ),
    action!(
        "focus-quote",
        ActionId::FocusPanel(Panel::Quote),
        "Focus quote",
        "Move keyboard focus to quote and session summary"
    ),
    action!(
        "focus-order-ticket",
        ActionId::FocusPanel(Panel::OrderTicket),
        "Focus order ticket",
        "Move keyboard focus to the staged order ticket"
    ),
    action!(
        "focus-intent-review",
        ActionId::FocusPanel(Panel::IntentReview),
        "Focus intent review",
        "Move keyboard focus to staged changes"
    ),
    action!(
        "focus-history",
        ActionId::FocusPanel(Panel::History),
        "Focus history",
        "Move keyboard focus to historical price chart"
    ),
    action!(
        "focus-crypto-evidence",
        ActionId::FocusPanel(Panel::Evidence),
        "Focus crypto evidence",
        "Move keyboard focus to crypto provider evidence"
    ),
    action!(
        "focus-polymarket",
        ActionId::FocusPanel(Panel::Polymarket),
        "Focus Polymarket",
        "Move keyboard focus to prediction market signals"
    ),
    action!(
        "focus-research",
        ActionId::FocusPanel(Panel::Research),
        "Focus research",
        "Move keyboard focus to news and research highlights"
    ),
    action!(
        "focus-provider-health",
        ActionId::FocusPanel(Panel::ProviderHealth),
        "Focus provider health",
        "Move keyboard focus to provider health"
    ),
    action!(
        "focus-task-log",
        ActionId::FocusPanel(Panel::TaskLog),
        "Focus task log",
        "Move keyboard focus to runtime task log"
    ),
    action!(
        "toggle-watchlist",
        ActionId::TogglePanel(Panel::Watchlist),
        "Toggle watchlist",
        "Show or hide the symbol list panel"
    ),
    action!(
        "toggle-quote",
        ActionId::TogglePanel(Panel::Quote),
        "Toggle quote",
        "Show or hide quote and session summary"
    ),
    action!(
        "toggle-order-ticket",
        ActionId::TogglePanel(Panel::OrderTicket),
        "Toggle order ticket",
        "Show or hide the staged order ticket"
    ),
    action!(
        "toggle-intent-review",
        ActionId::TogglePanel(Panel::IntentReview),
        "Toggle intent review",
        "Show or hide staged changes"
    ),
    action!(
        "toggle-history",
        ActionId::TogglePanel(Panel::History),
        "Toggle history",
        "Show or hide the historical price chart"
    ),
    action!(
        "toggle-crypto-evidence",
        ActionId::TogglePanel(Panel::Evidence),
        "Toggle crypto evidence",
        "Show or hide crypto provider evidence"
    ),
    action!(
        "toggle-polymarket",
        ActionId::TogglePanel(Panel::Polymarket),
        "Toggle Polymarket",
        "Show or hide prediction market signals"
    ),
    action!(
        "toggle-research",
        ActionId::TogglePanel(Panel::Research),
        "Toggle research",
        "Show or hide news and research highlights"
    ),
    action!(
        "toggle-provider-health",
        ActionId::TogglePanel(Panel::ProviderHealth),
        "Toggle provider health",
        "Show or hide provider capability coverage"
    ),
    action!(
        "toggle-task-log",
        ActionId::TogglePanel(Panel::TaskLog),
        "Toggle task log",
        "Show or hide the runtime task log"
    ),
    action!(
        "close-command-palette",
        ActionId::CloseCommandPalette,
        "Close command palette",
        "Dismiss this command palette without changing docked panels"
    ),
];

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
        assert!(visible.contains(&"Workspace providers"));
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
            .map(|spec| spec.id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();

        assert_eq!(ids.len(), ACTION_REGISTRY.len());
        for command in ACTION_REGISTRY.iter().filter_map(|action| action.command()) {
            assert!(action_id(command.action).is_some());
        }
    }
}
