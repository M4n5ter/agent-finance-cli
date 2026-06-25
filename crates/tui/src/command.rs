use crate::model::{FloatingKind, Panel};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct CommandPaletteState {
    pub selected: usize,
}

impl CommandPaletteState {
    pub fn shift(&mut self, direction: isize) {
        let len = COMMANDS.len() as isize;
        let selected = self.selected as isize;
        self.selected = (selected + direction).rem_euclid(len) as usize;
    }

    pub fn selected_command(&self) -> CommandSpec {
        COMMANDS[self.selected]
    }

    pub fn selected_effect(&self) -> CommandEffect {
        self.selected_command().effect
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CommandSpec {
    pub title: &'static str,
    pub description: &'static str,
    pub effect: CommandEffect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CommandEffect {
    OpenFloating(FloatingKind),
    ResetLayout,
    FocusPanel(Panel),
    TogglePanel(Panel),
    CloseFocusedPanel,
    RestorePanels,
    CloseCommandPalette,
}

pub const COMMANDS: [CommandSpec; 20] = [
    CommandSpec {
        title: "Open help",
        description: "Show cockpit shortcuts and interaction model",
        effect: CommandEffect::OpenFloating(FloatingKind::Help),
    },
    CommandSpec {
        title: "Open provider details",
        description: "Inspect provider capability coverage",
        effect: CommandEffect::OpenFloating(FloatingKind::ProviderDetails),
    },
    CommandSpec {
        title: "Reset layout",
        description: "Restore default docked columns and close overlays",
        effect: CommandEffect::ResetLayout,
    },
    CommandSpec {
        title: "Close focused panel",
        description: "Hide the focused docked panel and move focus to another open panel",
        effect: CommandEffect::CloseFocusedPanel,
    },
    CommandSpec {
        title: "Restore all panels",
        description: "Reopen every docked panel without changing the current symbol",
        effect: CommandEffect::RestorePanels,
    },
    CommandSpec {
        title: "Focus watchlist",
        description: "Move keyboard focus to the symbol list",
        effect: CommandEffect::FocusPanel(Panel::Watchlist),
    },
    CommandSpec {
        title: "Focus quote",
        description: "Move keyboard focus to quote and session summary",
        effect: CommandEffect::FocusPanel(Panel::Quote),
    },
    CommandSpec {
        title: "Focus history",
        description: "Move keyboard focus to historical price chart",
        effect: CommandEffect::FocusPanel(Panel::History),
    },
    CommandSpec {
        title: "Focus crypto evidence",
        description: "Move keyboard focus to crypto provider evidence",
        effect: CommandEffect::FocusPanel(Panel::Evidence),
    },
    CommandSpec {
        title: "Focus research",
        description: "Move keyboard focus to news and prediction markets",
        effect: CommandEffect::FocusPanel(Panel::Research),
    },
    CommandSpec {
        title: "Focus provider health",
        description: "Move keyboard focus to provider health",
        effect: CommandEffect::FocusPanel(Panel::ProviderHealth),
    },
    CommandSpec {
        title: "Focus task log",
        description: "Move keyboard focus to runtime task log",
        effect: CommandEffect::FocusPanel(Panel::TaskLog),
    },
    CommandSpec {
        title: "Toggle watchlist",
        description: "Show or hide the symbol list panel",
        effect: CommandEffect::TogglePanel(Panel::Watchlist),
    },
    CommandSpec {
        title: "Toggle quote",
        description: "Show or hide quote and session summary",
        effect: CommandEffect::TogglePanel(Panel::Quote),
    },
    CommandSpec {
        title: "Toggle history",
        description: "Show or hide the historical price chart",
        effect: CommandEffect::TogglePanel(Panel::History),
    },
    CommandSpec {
        title: "Toggle crypto evidence",
        description: "Show or hide crypto provider evidence",
        effect: CommandEffect::TogglePanel(Panel::Evidence),
    },
    CommandSpec {
        title: "Toggle research",
        description: "Show or hide news and prediction markets",
        effect: CommandEffect::TogglePanel(Panel::Research),
    },
    CommandSpec {
        title: "Toggle provider health",
        description: "Show or hide provider capability coverage",
        effect: CommandEffect::TogglePanel(Panel::ProviderHealth),
    },
    CommandSpec {
        title: "Toggle task log",
        description: "Show or hide the runtime task log",
        effect: CommandEffect::TogglePanel(Panel::TaskLog),
    },
    CommandSpec {
        title: "Close command palette",
        description: "Dismiss this command palette without changing docked panels",
        effect: CommandEffect::CloseCommandPalette,
    },
];
