use agent_finance_i18n::{LocaleId, MessageArgs, Translator};

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
