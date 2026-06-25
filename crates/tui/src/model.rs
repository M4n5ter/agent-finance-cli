#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Panel {
    Watchlist,
    Quote,
    History,
    Evidence,
    Research,
    ProviderHealth,
    TaskLog,
}

impl Panel {
    pub const ALL: [Self; 7] = [
        Self::Watchlist,
        Self::Quote,
        Self::History,
        Self::Evidence,
        Self::Research,
        Self::ProviderHealth,
        Self::TaskLog,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Watchlist => "Watchlist",
            Self::Quote => "Quote / Sessions",
            Self::History => "History Chart",
            Self::Evidence => "Crypto Evidence",
            Self::Research => "News / Research",
            Self::ProviderHealth => "Provider Health",
            Self::TaskLog => "Task Log",
        }
    }

    pub const fn order(self) -> usize {
        match self {
            Self::Watchlist => 0,
            Self::Quote => 1,
            Self::History => 2,
            Self::Evidence => 3,
            Self::Research => 4,
            Self::ProviderHealth => 5,
            Self::TaskLog => 6,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DockedPanels {
    open: Vec<Panel>,
    focused: Panel,
}

impl Default for DockedPanels {
    fn default() -> Self {
        Self::restored()
    }
}

impl DockedPanels {
    pub fn restored() -> Self {
        Self {
            open: Panel::ALL.to_vec(),
            focused: Panel::Watchlist,
        }
    }

    pub fn focused(&self) -> Panel {
        self.focused
    }

    pub fn open_panels(&self) -> &[Panel] {
        &self.open
    }

    pub fn open_count(&self) -> usize {
        self.open.len()
    }

    pub fn contains(&self, panel: Panel) -> bool {
        self.open.contains(&panel)
    }

    pub fn focus(&mut self, panel: Panel) {
        if self.contains(panel) {
            self.focused = panel;
        }
    }

    pub fn close_focused(&mut self) {
        self.close(self.focused);
    }

    pub fn close(&mut self, panel: Panel) {
        if self.open.len() <= 1 {
            return;
        }

        self.open.retain(|open| *open != panel);
        if self.focused == panel {
            self.focused = self.open[0];
        }
    }

    pub fn toggle(&mut self, panel: Panel) {
        if self.contains(panel) {
            self.close(panel);
        } else {
            self.open_panel(panel);
        }
    }

    pub fn open_panel(&mut self, panel: Panel) {
        if self.contains(panel) {
            return;
        }

        self.open.push(panel);
        self.open.sort_by_key(|panel| panel.order());
        self.focused = panel;
    }

    pub fn restore(&mut self) {
        self.open = Panel::ALL.to_vec();
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FloatingKind {
    CommandPalette,
    Help,
    ProviderDetails,
}

impl FloatingKind {
    pub const fn title(self) -> &'static str {
        match self {
            Self::CommandPalette => "Command Palette",
            Self::Help => "Help",
            Self::ProviderDetails => "Provider Details",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FloatingPane {
    pub kind: FloatingKind,
    pub z_index: u16,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TaskLevel {
    Info,
    Warning,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TaskLogEntry {
    pub level: TaskLevel,
    pub message: String,
}

impl TaskLogEntry {
    pub(crate) fn info(message: String) -> Self {
        Self {
            level: TaskLevel::Info,
            message,
        }
    }

    pub(crate) fn warning(message: String) -> Self {
        Self {
            level: TaskLevel::Warning,
            message,
        }
    }
}
