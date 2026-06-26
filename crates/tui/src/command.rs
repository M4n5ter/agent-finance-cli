use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use tui_input::{Input, InputRequest};

use crate::model::{FloatingKind, Panel, WorkspaceKind};

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    input: Input,
    selected: usize,
    matches: Vec<usize>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            input: Input::default(),
            selected: 0,
            matches: all_command_indices(),
        }
    }
}

impl CommandPaletteState {
    pub fn query(&self) -> &str {
        self.input.value()
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn command_at(&self, index: usize) -> Option<CommandSpec> {
        self.matches
            .get(index)
            .and_then(|command| ACTION_REGISTRY[*command].command())
    }

    pub fn shift(&mut self, direction: isize) {
        if self.matches.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.matches.len() as isize;
        let selected = self.selected as isize;
        self.selected = (selected + direction).rem_euclid(len) as usize;
    }

    pub fn selected_command(&self) -> Option<CommandSpec> {
        self.command_at(self.selected)
    }

    pub fn selected_action(&self) -> Option<ActionId> {
        self.selected_command().map(|command| command.action)
    }

    pub fn reset(&mut self) {
        self.input = Input::default();
        self.selected = 0;
        self.matches = all_command_indices();
    }

    pub fn edit_query(&mut self, request: InputRequest) {
        let previous = self.input.value().to_string();
        self.input.handle(request);
        if self.input.value() != previous {
            self.refresh_matches();
        }
    }

    fn refresh_matches(&mut self) {
        let query = self.input.value().trim();
        self.matches = if query.is_empty() {
            all_command_indices()
        } else {
            fuzzy_command_indices(query)
        };
        self.selected = 0;
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
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut utf32_buffer = Vec::new();
    let mut scored = ACTION_REGISTRY
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            let command = action.command()?;
            let text = format!("{} {}", command.title, command.description);
            pattern
                .score(Utf32Str::new(&text, &mut utf32_buffer), &mut matcher)
                .map(|score| (index, score))
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(index, _)| index).collect()
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

pub const ACTION_REGISTRY: [ActionSpec; 35] = [
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
