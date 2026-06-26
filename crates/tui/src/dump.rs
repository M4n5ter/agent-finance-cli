use serde::Serialize;

use crate::command::ACTION_REGISTRY;
use crate::model::{InteractionMode, Panel, WorkspaceKind};
use crate::provider_health::{ProviderHealthReport, ProviderHealthTask};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct TuiDump {
    pub workspace: WorkspaceKind,
    pub mode: InteractionMode,
    pub selected_symbol: Option<String>,
    pub partial: bool,
    pub panes: Vec<TuiPaneDump>,
    pub provider_health: ProviderHealthReport,
    pub tasks: Vec<ProviderHealthTask>,
    pub errors: Vec<String>,
    pub key_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TuiPaneDump {
    pub panel: Panel,
    pub title: &'static str,
    pub visible: bool,
    pub focused: bool,
    pub loading: bool,
    pub has_data: bool,
    pub status: TuiPaneStatus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TuiPaneStatus {
    Fresh,
    Loading,
    Partial,
    Empty,
    Error,
}

impl TuiDump {
    pub fn from_state(state: &AppState, partial: bool) -> Self {
        let provider_health = ProviderHealthReport::from_state(state);
        Self {
            workspace: state.workspace,
            mode: state.interaction_mode(),
            selected_symbol: state.selected_symbol().map(ToString::to_string),
            partial,
            panes: Panel::ALL
                .into_iter()
                .map(|panel| pane_dump(state, panel))
                .collect(),
            tasks: provider_health.tasks.clone(),
            errors: dump_errors(state),
            provider_health,
            key_hints: ACTION_REGISTRY
                .iter()
                .filter_map(|action| action.command())
                .map(|command| command.title.to_string())
                .collect(),
        }
    }
}

fn pane_dump(state: &AppState, panel: Panel) -> TuiPaneDump {
    let visible = state.visible_panels().contains(&panel);
    let focused = state.panels.focused() == panel;
    let (loading, has_data, has_error) = pane_data_state(state, panel);
    let status = if has_error {
        TuiPaneStatus::Error
    } else if loading {
        TuiPaneStatus::Loading
    } else if has_data {
        TuiPaneStatus::Fresh
    } else if panel == Panel::Evidence && !selected_symbol_is_crypto(state) {
        TuiPaneStatus::Empty
    } else {
        TuiPaneStatus::Partial
    };

    TuiPaneDump {
        panel,
        title: panel.title(),
        visible,
        focused,
        loading,
        has_data,
        status,
    }
}

fn pane_data_state(state: &AppState, panel: Panel) -> (bool, bool, bool) {
    let selected = state.selected_symbol().unwrap_or_default();
    match panel {
        Panel::Watchlist => (false, !state.watchlist.is_empty(), false),
        Panel::Quote => (
            state.refresh.loading,
            state.market_snapshot.is_some(),
            state
                .task_failures
                .has_source(crate::task_failure::TaskFailureSource::Quotes),
        ),
        Panel::History => (
            state.history.loading(),
            state.history.selected_snapshot(selected).is_some(),
            state
                .task_failures
                .has_source(crate::task_failure::TaskFailureSource::History),
        ),
        Panel::Evidence => (
            state.evidence.loading(),
            state.evidence.selected_snapshot(selected).is_some(),
            state
                .task_failures
                .has_source(crate::task_failure::TaskFailureSource::CryptoEvidence),
        ),
        Panel::Polymarket | Panel::Research => (
            state.research.loading(),
            state.research.selected_snapshot(selected).is_some(),
            false,
        ),
        Panel::ProviderHealth => {
            let report = ProviderHealthReport::from_state(state);
            (state.refresh.loading, !report.is_empty(), false)
        }
        Panel::TaskLog => (false, !state.task_log.is_empty(), false),
    }
}

fn dump_errors(state: &AppState) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(error) = &state.scheduler_error {
        errors.push(error.clone());
    }
    errors.extend(
        state
            .task_failures
            .iter()
            .map(|failure| failure.error.clone()),
    );
    errors
}

fn selected_symbol_is_crypto(state: &AppState) -> bool {
    state
        .selected_symbol()
        .is_some_and(agent_finance_market::is_likely_crypto_pair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TuiConfig, WorkspaceConfig};

    #[test]
    fn dump_marks_only_workspace_panels_visible() {
        let state = AppState::from_config(TuiConfig {
            watchlist: vec!["AAPL".to_string(), "BTCUSDT".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Crypto,
            },
            ..TuiConfig::default()
        });

        let dump = TuiDump::from_state(&state, true);

        assert_eq!(dump.workspace, WorkspaceKind::Crypto);
        assert_eq!(dump.selected_symbol.as_deref(), Some("AAPL"));
        assert!(dump.partial);
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::History && pane.visible)
        );
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Evidence && pane.visible)
        );
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Research && !pane.visible)
        );
    }

    #[test]
    fn dump_serializes_agent_facing_names() {
        let state = AppState::from_config(TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Providers,
            },
            ..TuiConfig::default()
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["workspace"], "providers");
        assert!(
            value["panes"]
                .as_array()
                .expect("panes")
                .iter()
                .any(|pane| pane["panel"] == "provider-health")
        );
    }
}
