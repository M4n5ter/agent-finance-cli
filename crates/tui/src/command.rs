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
            .map(|command| ACTION_SPECS[*command])
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
    (0..ACTION_SPECS.len()).collect()
}

fn fuzzy_command_indices(query: &str) -> Vec<usize> {
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut utf32_buffer = Vec::new();
    let mut scored = ACTION_SPECS
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
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
    OpenFloating(FloatingKind),
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

pub const ACTION_SPECS: [CommandSpec; 31] = [
    CommandSpec {
        title: "Open help",
        description: "Show cockpit shortcuts and interaction model",
        action: ActionId::OpenFloating(FloatingKind::Help),
    },
    CommandSpec {
        title: "Open provider details",
        description: "Inspect provider capability coverage",
        action: ActionId::OpenFloating(FloatingKind::ProviderDetails),
    },
    CommandSpec {
        title: "Reset layout",
        description: "Restore default docked columns and close overlays",
        action: ActionId::ResetLayout,
    },
    CommandSpec {
        title: "Close focused panel",
        description: "Hide the focused docked panel and move focus to another open panel",
        action: ActionId::CloseFocusedPanel,
    },
    CommandSpec {
        title: "Restore all panels",
        description: "Reopen every docked panel without changing the current symbol",
        action: ActionId::RestorePanels,
    },
    CommandSpec {
        title: "Next pane",
        description: "Move focus to the next workspace pane",
        action: ActionId::FocusPanelBy(1),
    },
    CommandSpec {
        title: "Previous pane",
        description: "Move focus to the previous workspace pane",
        action: ActionId::FocusPanelBy(-1),
    },
    CommandSpec {
        title: "Toggle pane zoom",
        description: "Expand the focused docked pane or restore the workspace layout",
        action: ActionId::ToggleFocusedZoom,
    },
    CommandSpec {
        title: "Next workspace",
        description: "Move to the next workspace tab",
        action: ActionId::ShiftWorkspace(1),
    },
    CommandSpec {
        title: "Previous workspace",
        description: "Move to the previous workspace tab",
        action: ActionId::ShiftWorkspace(-1),
    },
    CommandSpec {
        title: "Workspace overview",
        description: "Show the overview cockpit workspace",
        action: ActionId::SetWorkspace(WorkspaceKind::Overview),
    },
    CommandSpec {
        title: "Workspace research",
        description: "Show news, research, and prediction-market context",
        action: ActionId::SetWorkspace(WorkspaceKind::Research),
    },
    CommandSpec {
        title: "Workspace crypto",
        description: "Show crypto evidence and market context",
        action: ActionId::SetWorkspace(WorkspaceKind::Crypto),
    },
    CommandSpec {
        title: "Workspace providers",
        description: "Show provider health and runtime task status",
        action: ActionId::SetWorkspace(WorkspaceKind::Providers),
    },
    CommandSpec {
        title: "Focus watchlist",
        description: "Move keyboard focus to the symbol list",
        action: ActionId::FocusPanel(Panel::Watchlist),
    },
    CommandSpec {
        title: "Focus quote",
        description: "Move keyboard focus to quote and session summary",
        action: ActionId::FocusPanel(Panel::Quote),
    },
    CommandSpec {
        title: "Focus history",
        description: "Move keyboard focus to historical price chart",
        action: ActionId::FocusPanel(Panel::History),
    },
    CommandSpec {
        title: "Focus crypto evidence",
        description: "Move keyboard focus to crypto provider evidence",
        action: ActionId::FocusPanel(Panel::Evidence),
    },
    CommandSpec {
        title: "Focus Polymarket",
        description: "Move keyboard focus to prediction market signals",
        action: ActionId::FocusPanel(Panel::Polymarket),
    },
    CommandSpec {
        title: "Focus research",
        description: "Move keyboard focus to news and research highlights",
        action: ActionId::FocusPanel(Panel::Research),
    },
    CommandSpec {
        title: "Focus provider health",
        description: "Move keyboard focus to provider health",
        action: ActionId::FocusPanel(Panel::ProviderHealth),
    },
    CommandSpec {
        title: "Focus task log",
        description: "Move keyboard focus to runtime task log",
        action: ActionId::FocusPanel(Panel::TaskLog),
    },
    CommandSpec {
        title: "Toggle watchlist",
        description: "Show or hide the symbol list panel",
        action: ActionId::TogglePanel(Panel::Watchlist),
    },
    CommandSpec {
        title: "Toggle quote",
        description: "Show or hide quote and session summary",
        action: ActionId::TogglePanel(Panel::Quote),
    },
    CommandSpec {
        title: "Toggle history",
        description: "Show or hide the historical price chart",
        action: ActionId::TogglePanel(Panel::History),
    },
    CommandSpec {
        title: "Toggle crypto evidence",
        description: "Show or hide crypto provider evidence",
        action: ActionId::TogglePanel(Panel::Evidence),
    },
    CommandSpec {
        title: "Toggle Polymarket",
        description: "Show or hide prediction market signals",
        action: ActionId::TogglePanel(Panel::Polymarket),
    },
    CommandSpec {
        title: "Toggle research",
        description: "Show or hide news and research highlights",
        action: ActionId::TogglePanel(Panel::Research),
    },
    CommandSpec {
        title: "Toggle provider health",
        description: "Show or hide provider capability coverage",
        action: ActionId::TogglePanel(Panel::ProviderHealth),
    },
    CommandSpec {
        title: "Toggle task log",
        description: "Show or hide the runtime task log",
        action: ActionId::TogglePanel(Panel::TaskLog),
    },
    CommandSpec {
        title: "Close command palette",
        description: "Dismiss this command palette without changing docked panels",
        action: ActionId::CloseCommandPalette,
    },
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
}
