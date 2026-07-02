use agent_finance_i18n::{LocaleId, MessageArgs, Translator};

use crate::command::CommandSpec;
use crate::model::{Panel, WorkspaceKind};
use crate::pane_status::TuiPaneStatus;
use crate::settings_editor::SettingRow;

pub(crate) struct TuiText {
    translator: Translator,
}

impl TuiText {
    pub(crate) fn new(locale: LocaleId) -> Self {
        Self {
            translator: Translator::new(locale).expect("built-in TUI catalogs must be valid"),
        }
    }

    pub(crate) fn t(&self, key: &str) -> String {
        self.translator.text(key)
    }

    pub(crate) fn f(&self, key: &str, args: MessageArgs<'_>) -> String {
        self.translator.text_with_args(key, args)
    }

    pub(crate) fn f_or(&self, key: &str, args: MessageArgs<'_>, fallback: &str) -> String {
        let value = self.f(key, args);
        if value == missing_marker(key) {
            fallback.to_owned()
        } else {
            value
        }
    }

    pub(crate) fn panel_title(&self, panel: Panel) -> String {
        self.t(panel_title_key(panel))
    }

    pub(crate) fn workspace_title(&self, workspace: WorkspaceKind) -> String {
        self.t(workspace_title_key(workspace))
    }

    pub(crate) fn pane_status(&self, status: TuiPaneStatus) -> String {
        self.t(pane_status_key(status))
    }

    pub(crate) fn setting_label(&self, row: SettingRow) -> String {
        self.t(row.label_key())
    }

    pub(crate) fn floating_title(&self, kind: crate::model::FloatingKind) -> String {
        self.t(floating_title_key(kind))
    }

    pub(crate) fn command_title(&self, command: &CommandSpec) -> String {
        self.command_message(command, "title", command.title.as_ref())
    }

    pub(crate) fn command_description(&self, command: &CommandSpec) -> String {
        self.command_message(command, "description", command.description.as_ref())
    }

    fn command_message(&self, command: &CommandSpec, suffix: &str, fallback: &str) -> String {
        if self.translator.locale() == agent_finance_i18n::LocaleId::DEFAULT {
            return fallback.to_owned();
        }
        let key = command_message_key(command.id.as_ref(), suffix);
        let label = command_label(command.id.as_ref()).unwrap_or(fallback);
        let value = self.f(&key, &[("label", label)]);
        if value != missing_marker(&key) {
            return value;
        }
        if let Some(generic_key) = generic_command_message_key(command.id.as_ref(), suffix) {
            return self.f_or(generic_key, &[("label", label)], fallback);
        }
        fallback.to_owned()
    }
}

fn missing_marker(key: &str) -> String {
    format!("⟦{key}⟧")
}

fn command_message_key(id: &str, suffix: &str) -> String {
    format!("tui-command-{id}-{suffix}")
}

fn command_label(id: &str) -> Option<&str> {
    id.strip_prefix("chart-preset-")
        .or_else(|| id.strip_prefix("chart-interval-"))
        .or_else(|| id.strip_prefix("chart-glyph-"))
        .or_else(|| id.strip_prefix("focus-"))
        .or_else(|| id.strip_prefix("toggle-"))
}

fn generic_command_message_key(id: &str, suffix: &str) -> Option<&'static str> {
    match (id, suffix) {
        (id, "title") if id.starts_with("chart-preset-") => Some("tui-command-chart-preset-title"),
        (id, "description") if id.starts_with("chart-preset-") => {
            Some("tui-command-chart-preset-description")
        }
        (id, "title") if id.starts_with("chart-interval-") => {
            Some("tui-command-chart-interval-title")
        }
        (id, "description") if id.starts_with("chart-interval-") => {
            Some("tui-command-chart-interval-description")
        }
        (id, "title") if id.starts_with("chart-glyph-") => Some("tui-command-chart-glyph-title"),
        (id, "description") if id.starts_with("chart-glyph-") => {
            Some("tui-command-chart-glyph-description")
        }
        (id, "title") if id.starts_with("focus-") => Some("tui-command-focus-panel-title"),
        (id, "description") if id.starts_with("focus-") => {
            Some("tui-command-focus-panel-description")
        }
        (id, "title") if id.starts_with("toggle-") => Some("tui-command-toggle-panel-title"),
        (id, "description") if id.starts_with("toggle-") => {
            Some("tui-command-toggle-panel-description")
        }
        _ => None,
    }
}

fn floating_title_key(kind: crate::model::FloatingKind) -> &'static str {
    match kind {
        crate::model::FloatingKind::CommandPalette => "tui-floating-command-palette",
        crate::model::FloatingKind::Help => "tui-floating-help",
        crate::model::FloatingKind::LiveWritesConfirmation => {
            "tui-floating-live-writes-confirmation"
        }
        crate::model::FloatingKind::StagedExecutionConfirmation => {
            "tui-floating-staged-execution-confirmation"
        }
        crate::model::FloatingKind::TradingProfile => "tui-floating-trading-profile",
        crate::model::FloatingKind::ProviderDetails => "tui-floating-provider-details",
        crate::model::FloatingKind::SymbolSearch => "tui-floating-symbol-search",
        crate::model::FloatingKind::WatchlistAdd => "tui-floating-watchlist-add",
        crate::model::FloatingKind::TicketTextInput => "tui-floating-ticket-text-input",
    }
}

fn panel_title_key(panel: Panel) -> &'static str {
    match panel {
        Panel::Watchlist => "tui-panel-watchlist",
        Panel::Quote => "tui-panel-quote",
        Panel::OrderTicket => "tui-panel-order-ticket",
        Panel::OpenOrders => "tui-panel-open-orders",
        Panel::IntentReview => "tui-panel-intent-review",
        Panel::RiskAudit => "tui-panel-risk-audit",
        Panel::Account => "tui-panel-account",
        Panel::TransferTicket => "tui-panel-transfer-ticket",
        Panel::FuturesState => "tui-panel-futures-state",
        Panel::History => "tui-panel-history",
        Panel::Evidence => "tui-panel-evidence",
        Panel::Polymarket => "tui-panel-polymarket",
        Panel::Research => "tui-panel-research",
        Panel::ProviderHealth => "tui-panel-provider-health",
        Panel::TaskLog => "tui-panel-task-log",
        Panel::Settings => "tui-panel-settings",
        Panel::ProfileRisk => "tui-panel-profile-risk",
    }
}

fn workspace_title_key(workspace: WorkspaceKind) -> &'static str {
    match workspace {
        WorkspaceKind::Market => "tui-workspace-market",
        WorkspaceKind::Trade => "tui-workspace-trade",
        WorkspaceKind::Account => "tui-workspace-account",
        WorkspaceKind::Research => "tui-workspace-research",
        WorkspaceKind::Settings => "tui-workspace-settings",
    }
}

fn pane_status_key(status: TuiPaneStatus) -> &'static str {
    match status {
        TuiPaneStatus::Fresh => "tui-pane-status-fresh",
        TuiPaneStatus::Loading => "tui-pane-status-loading",
        TuiPaneStatus::Partial => "tui-pane-status-partial",
        TuiPaneStatus::Empty => "tui-pane-status-empty",
        TuiPaneStatus::Error => "tui-pane-status-error",
        TuiPaneStatus::Stale => "tui-pane-status-stale",
    }
}
