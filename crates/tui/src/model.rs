use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceKind {
    #[default]
    Market,
    Trade,
    Account,
    Research,
    Settings,
}

impl WorkspaceKind {
    pub const ALL: [Self; 5] = [
        Self::Market,
        Self::Trade,
        Self::Account,
        Self::Research,
        Self::Settings,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Market => "Market",
            Self::Trade => "Trade",
            Self::Account => "Account",
            Self::Research => "Research",
            Self::Settings => "Settings",
        }
    }

    pub const fn panels(self) -> &'static [Panel] {
        match self {
            Self::Market => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::History,
                Panel::ProviderHealth,
                Panel::TaskLog,
            ],
            Self::Trade => &[
                Panel::Watchlist,
                Panel::OrderTicket,
                Panel::OpenOrders,
                Panel::IntentReview,
                Panel::RiskAudit,
                Panel::TaskLog,
                Panel::ProviderHealth,
            ],
            Self::Account => &[
                Panel::Account,
                Panel::TransferTicket,
                Panel::FuturesState,
                Panel::ProviderHealth,
                Panel::TaskLog,
                Panel::Watchlist,
            ],
            Self::Research => &[
                Panel::Watchlist,
                Panel::Quote,
                Panel::Polymarket,
                Panel::Research,
                Panel::Evidence,
                Panel::TaskLog,
            ],
            Self::Settings => &[
                Panel::Settings,
                Panel::ProfileRisk,
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
            Self::Market => "market",
            Self::Trade => "trade",
            Self::Account => "account",
            Self::Research => "research",
            Self::Settings => "settings",
        }
    }

    pub const fn labels() -> &'static [&'static str] {
        &["market", "trade", "account", "research", "settings"]
    }

    pub const fn command_id(self) -> &'static str {
        match self {
            Self::Market => "workspace-market",
            Self::Trade => "workspace-trade",
            Self::Account => "workspace-account",
            Self::Research => "workspace-research",
            Self::Settings => "workspace-settings",
        }
    }

    pub const fn command_title(self) -> &'static str {
        match self {
            Self::Market => "Workspace market",
            Self::Trade => "Workspace trade",
            Self::Account => "Workspace account",
            Self::Research => "Workspace research",
            Self::Settings => "Workspace settings",
        }
    }

    pub const fn command_description(self) -> &'static str {
        match self {
            Self::Market => "Show watchlist, quote sessions, history, and provider health",
            Self::Trade => "Show order tickets, staged intent review, and trading context",
            Self::Account => {
                "Show account reads, balances, open orders, transfers, and futures state"
            }
            Self::Research => "Show news, research, crypto evidence, and prediction-market context",
            Self::Settings => "Show configuration maintenance controls",
        }
    }
}

impl Serialize for WorkspaceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
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
            "market" => Ok(Self::Market),
            "trade" => Ok(Self::Trade),
            "account" => Ok(Self::Account),
            "research" => Ok(Self::Research),
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
    OpenOrders,
    IntentReview,
    RiskAudit,
    Account,
    TransferTicket,
    FuturesState,
    History,
    Evidence,
    Polymarket,
    Research,
    ProviderHealth,
    TaskLog,
    Settings,
    ProfileRisk,
}

impl Panel {
    pub const ALL: [Self; 17] = [
        Self::Watchlist,
        Self::Quote,
        Self::OrderTicket,
        Self::OpenOrders,
        Self::IntentReview,
        Self::RiskAudit,
        Self::Account,
        Self::TransferTicket,
        Self::FuturesState,
        Self::History,
        Self::Evidence,
        Self::Polymarket,
        Self::Research,
        Self::ProviderHealth,
        Self::TaskLog,
        Self::Settings,
        Self::ProfileRisk,
    ];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Watchlist => "Watchlist",
            Self::Quote => "Quote / Sessions",
            Self::OrderTicket => "Order Ticket",
            Self::OpenOrders => "Open Orders",
            Self::IntentReview => "Intent Review",
            Self::RiskAudit => "Risk / Audit",
            Self::Account => "Account",
            Self::TransferTicket => "Transfer Ticket",
            Self::FuturesState => "Futures State",
            Self::History => "History Chart",
            Self::Evidence => "Crypto Evidence",
            Self::Polymarket => "Polymarket",
            Self::Research => "News / Research",
            Self::ProviderHealth => "Provider Health",
            Self::TaskLog => "Task Log",
            Self::Settings => "Settings",
            Self::ProfileRisk => "Profile / Risk",
        }
    }

    pub const fn command_slug(self) -> &'static str {
        match self {
            Self::Watchlist => "watchlist",
            Self::Quote => "quote",
            Self::OrderTicket => "order-ticket",
            Self::OpenOrders => "open-orders",
            Self::IntentReview => "intent-review",
            Self::RiskAudit => "risk-audit",
            Self::Account => "account",
            Self::TransferTicket => "transfer-ticket",
            Self::FuturesState => "futures-state",
            Self::History => "history",
            Self::Evidence => "crypto-evidence",
            Self::Polymarket => "polymarket",
            Self::Research => "research",
            Self::ProviderHealth => "provider-health",
            Self::TaskLog => "task-log",
            Self::Settings => "settings",
            Self::ProfileRisk => "profile-risk",
        }
    }

    pub const fn focus_command_title(self) -> &'static str {
        match self {
            Self::Watchlist => "Focus watchlist",
            Self::Quote => "Focus quote",
            Self::OrderTicket => "Focus order ticket",
            Self::OpenOrders => "Focus open orders",
            Self::IntentReview => "Focus intent review",
            Self::RiskAudit => "Focus risk audit",
            Self::Account => "Focus account",
            Self::TransferTicket => "Focus transfer ticket",
            Self::FuturesState => "Focus futures state",
            Self::History => "Focus history",
            Self::Evidence => "Focus crypto evidence",
            Self::Polymarket => "Focus Polymarket",
            Self::Research => "Focus research",
            Self::ProviderHealth => "Focus provider health",
            Self::TaskLog => "Focus task log",
            Self::Settings => "Focus settings",
            Self::ProfileRisk => "Focus profile risk",
        }
    }

    pub const fn focus_command_description(self) -> &'static str {
        match self {
            Self::Watchlist => "Move keyboard focus to the symbol list",
            Self::Quote => "Move keyboard focus to quote and session summary",
            Self::OrderTicket => "Move keyboard focus to the staged order ticket",
            Self::OpenOrders => "Move keyboard focus to active exchange orders",
            Self::IntentReview => "Move keyboard focus to staged changes",
            Self::RiskAudit => "Move keyboard focus to trading risk and audit summary",
            Self::Account => "Move keyboard focus to signed account state",
            Self::TransferTicket => "Move keyboard focus to transfer staging",
            Self::FuturesState => "Move keyboard focus to USD-M futures state staging",
            Self::History => "Move keyboard focus to historical price chart",
            Self::Evidence => "Move keyboard focus to crypto provider evidence",
            Self::Polymarket => "Move keyboard focus to prediction market signals",
            Self::Research => "Move keyboard focus to news and research highlights",
            Self::ProviderHealth => "Move keyboard focus to provider health",
            Self::TaskLog => "Move keyboard focus to runtime task log",
            Self::Settings => "Move keyboard focus to configuration maintenance",
            Self::ProfileRisk => "Move keyboard focus to profile validation and risk policy",
        }
    }

    pub const fn toggle_command_title(self) -> &'static str {
        match self {
            Self::Watchlist => "Toggle watchlist",
            Self::Quote => "Toggle quote",
            Self::OrderTicket => "Toggle order ticket",
            Self::OpenOrders => "Toggle open orders",
            Self::IntentReview => "Toggle intent review",
            Self::RiskAudit => "Toggle risk audit",
            Self::Account => "Toggle account",
            Self::TransferTicket => "Toggle transfer ticket",
            Self::FuturesState => "Toggle futures state",
            Self::History => "Toggle history",
            Self::Evidence => "Toggle crypto evidence",
            Self::Polymarket => "Toggle Polymarket",
            Self::Research => "Toggle research",
            Self::ProviderHealth => "Toggle provider health",
            Self::TaskLog => "Toggle task log",
            Self::Settings => "Toggle settings",
            Self::ProfileRisk => "Toggle profile risk",
        }
    }

    pub const fn toggle_command_description(self) -> &'static str {
        match self {
            Self::Watchlist => "Show or hide the symbol list panel",
            Self::Quote => "Show or hide quote and session summary",
            Self::OrderTicket => "Show or hide the staged order ticket",
            Self::OpenOrders => "Show or hide active exchange orders",
            Self::IntentReview => "Show or hide staged changes",
            Self::RiskAudit => "Show or hide trading risk and audit summary",
            Self::Account => "Show or hide signed account state",
            Self::TransferTicket => "Show or hide transfer staging",
            Self::FuturesState => "Show or hide USD-M futures state staging",
            Self::History => "Show or hide the historical price chart",
            Self::Evidence => "Show or hide crypto provider evidence",
            Self::Polymarket => "Show or hide prediction market signals",
            Self::Research => "Show or hide news and research highlights",
            Self::ProviderHealth => "Show or hide provider capability coverage",
            Self::TaskLog => "Show or hide the runtime task log",
            Self::Settings => "Show or hide configuration maintenance",
            Self::ProfileRisk => "Show or hide profile validation and risk policy",
        }
    }

    pub const fn order(self) -> usize {
        match self {
            Self::Watchlist => 0,
            Self::Quote => 1,
            Self::OrderTicket => 2,
            Self::OpenOrders => 3,
            Self::IntentReview => 4,
            Self::RiskAudit => 5,
            Self::Account => 6,
            Self::TransferTicket => 7,
            Self::FuturesState => 8,
            Self::History => 9,
            Self::Evidence => 10,
            Self::Polymarket => 11,
            Self::Research => 12,
            Self::ProviderHealth => 13,
            Self::TaskLog => 14,
            Self::Settings => 15,
            Self::ProfileRisk => 16,
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
    StagedExecutionConfirmation,
    TradingProfile,
    ProviderDetails,
    SymbolSearch,
    WatchlistAdd,
    TicketTextInput,
}

impl FloatingKind {
    pub const ALL: [Self; 9] = [
        Self::CommandPalette,
        Self::Help,
        Self::LiveWritesConfirmation,
        Self::StagedExecutionConfirmation,
        Self::TradingProfile,
        Self::ProviderDetails,
        Self::SymbolSearch,
        Self::WatchlistAdd,
        Self::TicketTextInput,
    ];

    pub const fn persistent(self) -> bool {
        match self {
            Self::CommandPalette
            | Self::LiveWritesConfirmation
            | Self::StagedExecutionConfirmation
            | Self::TradingProfile
            | Self::SymbolSearch
            | Self::WatchlistAdd
            | Self::TicketTextInput => false,
            Self::Help | Self::ProviderDetails => true,
        }
    }

    pub const fn text_input(self) -> bool {
        matches!(
            self,
            Self::CommandPalette
                | Self::TradingProfile
                | Self::SymbolSearch
                | Self::WatchlistAdd
                | Self::TicketTextInput
        )
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::CommandPalette => "Command Palette",
            Self::Help => "Help",
            Self::LiveWritesConfirmation => "Enable Live Writes",
            Self::StagedExecutionConfirmation => "Confirm Staged Execution",
            Self::TradingProfile => "Trading Profile",
            Self::ProviderDetails => "Provider Details",
            Self::SymbolSearch => "Symbol Search",
            Self::WatchlistAdd => "Add Symbols",
            Self::TicketTextInput => "Ticket Text Input",
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
            | FloatingKind::WatchlistAdd
            | FloatingKind::TicketTextInput => Self {
                width_ratio: 70,
                height_ratio: 40,
            },
            FloatingKind::LiveWritesConfirmation => Self {
                width_ratio: 56,
                height_ratio: 34,
            },
            FloatingKind::StagedExecutionConfirmation => Self {
                width_ratio: 56,
                height_ratio: 60,
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
