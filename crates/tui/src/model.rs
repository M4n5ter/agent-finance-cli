use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceKind {
    #[default]
    Overview,
    Trade,
    Account,
    Research,
    Crypto,
    Providers,
    Settings,
}

impl WorkspaceKind {
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Trade,
        Self::Account,
        Self::Research,
        Self::Crypto,
        Self::Providers,
        Self::Settings,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Trade => "Trade",
            Self::Account => "Account",
            Self::Research => "Research",
            Self::Crypto => "Crypto",
            Self::Providers => "Providers",
            Self::Settings => "Settings",
        }
    }

    pub const fn panels(self) -> &'static [Panel] {
        match self {
            Self::Overview => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::History,
                Panel::ProviderHealth,
                Panel::TaskLog,
            ],
            Self::Trade => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::OrderTicket,
                Panel::IntentReview,
                Panel::TaskLog,
                Panel::ProviderHealth,
            ],
            Self::Account => &[
                Panel::Account,
                Panel::ProviderHealth,
                Panel::TaskLog,
                Panel::Watchlist,
                Panel::Quote,
            ],
            Self::Research => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::Polymarket,
                Panel::Research,
                Panel::TaskLog,
            ],
            Self::Crypto => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::History,
                Panel::Evidence,
                Panel::ProviderHealth,
            ],
            Self::Providers => &[
                Panel::Watchlist,
                Panel::ProviderHealth,
                Panel::TaskLog,
                Panel::Quote,
            ],
            Self::Settings => &[
                Panel::Settings,
                Panel::Watchlist,
                Panel::ProviderHealth,
                Panel::TaskLog,
            ],
        }
    }

    pub const fn default_panel(self) -> Panel {
        self.panels()[0]
    }

    pub const fn entry_focus_panel(self) -> Option<Panel> {
        match self {
            Self::Settings => Some(Self::Settings.default_panel()),
            _ => None,
        }
    }

    pub fn shift(self, direction: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|workspace| *workspace == self)
            .unwrap_or(0) as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Trade => "trade",
            Self::Account => "account",
            Self::Research => "research",
            Self::Crypto => "crypto",
            Self::Providers => "providers",
            Self::Settings => "settings",
        }
    }

    pub const fn labels() -> &'static [&'static str] {
        &[
            "overview",
            "trade",
            "account",
            "research",
            "crypto",
            "providers",
            "settings",
        ]
    }
}

impl fmt::Display for WorkspaceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for WorkspaceKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "overview" => Ok(Self::Overview),
            "trade" => Ok(Self::Trade),
            "account" => Ok(Self::Account),
            "research" => Ok(Self::Research),
            "crypto" => Ok(Self::Crypto),
            "providers" => Ok(Self::Providers),
            "settings" => Ok(Self::Settings),
            _ => Err(format!("unknown workspace {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionMode {
    #[default]
    Normal,
    Command,
    Help,
    Inspect,
    Search,
}

impl InteractionMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Command => "command",
            Self::Help => "help",
            Self::Inspect => "inspect",
            Self::Search => "search",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Panel {
    Watchlist,
    Quote,
    OrderTicket,
    IntentReview,
    Account,
    History,
    Evidence,
    Polymarket,
    Research,
    ProviderHealth,
    TaskLog,
    Settings,
}

impl Panel {
    pub const ALL: [Self; 12] = [
        Self::Watchlist,
        Self::Quote,
        Self::OrderTicket,
        Self::IntentReview,
        Self::Account,
        Self::History,
        Self::Evidence,
        Self::Polymarket,
        Self::Research,
        Self::ProviderHealth,
        Self::TaskLog,
        Self::Settings,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Watchlist => "Watchlist",
            Self::Quote => "Quote / Sessions",
            Self::OrderTicket => "Order Ticket",
            Self::IntentReview => "Intent Review",
            Self::Account => "Account",
            Self::History => "History Chart",
            Self::Evidence => "Crypto Evidence",
            Self::Polymarket => "Polymarket",
            Self::Research => "News / Research",
            Self::ProviderHealth => "Provider Health",
            Self::TaskLog => "Task Log",
            Self::Settings => "Settings",
        }
    }

    pub const fn order(self) -> usize {
        match self {
            Self::Watchlist => 0,
            Self::Quote => 1,
            Self::OrderTicket => 2,
            Self::IntentReview => 3,
            Self::Account => 4,
            Self::History => 5,
            Self::Evidence => 6,
            Self::Polymarket => 7,
            Self::Research => 8,
            Self::ProviderHealth => 9,
            Self::TaskLog => 10,
            Self::Settings => 11,
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

    pub fn from_open_focused(open: Vec<Panel>, focused: Panel) -> Self {
        let mut panels = Self { open, focused };
        panels.normalize();
        panels
    }

    pub fn focused(&self) -> Panel {
        self.focused
    }

    pub fn open_panels(&self) -> &[Panel] {
        &self.open
    }

    pub fn into_parts(self) -> (Vec<Panel>, Panel) {
        (self.open, self.focused)
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

    fn normalize(&mut self) {
        self.open = Panel::ALL
            .into_iter()
            .filter(|panel| self.open.contains(panel))
            .collect();
        if self.open.is_empty() {
            self.open = Panel::ALL.to_vec();
        }
        if !self.open.contains(&self.focused) {
            self.focused = self.open[0];
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FloatingKind {
    CommandPalette,
    Help,
    LiveWritesConfirmation,
    StagedSubmitConfirmation,
    TradingProfile,
    ProviderDetails,
    SymbolSearch,
    WatchlistAdd,
}

impl FloatingKind {
    pub const fn persistent(self) -> bool {
        match self {
            Self::CommandPalette
            | Self::LiveWritesConfirmation
            | Self::StagedSubmitConfirmation
            | Self::TradingProfile
            | Self::SymbolSearch
            | Self::WatchlistAdd => false,
            Self::Help | Self::ProviderDetails => true,
        }
    }

    pub const fn text_input(self) -> bool {
        matches!(
            self,
            Self::CommandPalette | Self::TradingProfile | Self::SymbolSearch | Self::WatchlistAdd
        )
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::CommandPalette => "Command Palette",
            Self::Help => "Help",
            Self::LiveWritesConfirmation => "Enable Live Writes",
            Self::StagedSubmitConfirmation => "Confirm Staged Submit",
            Self::TradingProfile => "Trading Profile",
            Self::ProviderDetails => "Provider Details",
            Self::SymbolSearch => "Symbol Search",
            Self::WatchlistAdd => "Add Symbols",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct FloatingPane {
    pub kind: FloatingKind,
    pub size: FloatingSize,
}

impl FloatingPane {
    pub fn new(kind: FloatingKind) -> Self {
        Self {
            kind,
            size: FloatingSize::default_for(kind),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct FloatingSize {
    pub width_ratio: u16,
    pub height_ratio: u16,
}

impl FloatingSize {
    pub const MIN_RATIO: u16 = 20;
    pub const MAX_RATIO: u16 = 95;

    pub const fn default_for(kind: FloatingKind) -> Self {
        match kind {
            FloatingKind::CommandPalette
            | FloatingKind::TradingProfile
            | FloatingKind::SymbolSearch
            | FloatingKind::WatchlistAdd => Self {
                width_ratio: 70,
                height_ratio: 40,
            },
            FloatingKind::LiveWritesConfirmation | FloatingKind::StagedSubmitConfirmation => Self {
                width_ratio: 56,
                height_ratio: 34,
            },
            FloatingKind::Help => Self {
                width_ratio: 64,
                height_ratio: 70,
            },
            FloatingKind::ProviderDetails => Self {
                width_ratio: 58,
                height_ratio: 58,
            },
        }
    }

    pub fn resized(width_ratio: u16, height_ratio: u16) -> Self {
        Self {
            width_ratio: width_ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO),
            height_ratio: height_ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO),
        }
    }
}
