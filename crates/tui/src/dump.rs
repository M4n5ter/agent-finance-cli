use serde::Serialize;

use crate::hints;
use crate::model::{InteractionMode, Panel, WorkspaceKind};
use crate::pane_status::{TuiPaneStatus, pane_health};
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
            key_hints: hints::mode_key_hints(state),
        }
    }
}

fn pane_dump(state: &AppState, panel: Panel) -> TuiPaneDump {
    let visible = state.visible_panels().contains(&panel);
    let focused = state.panels.focused() == panel;
    let health = pane_health(state, panel);

    TuiPaneDump {
        panel,
        title: panel.title(),
        visible,
        focused,
        loading: health.loading,
        has_data: health.has_data,
        status: health.status,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::{TuiConfig, WorkspaceConfig};
    use crate::model::FloatingKind;
    use crate::state::Action;

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
        assert!(
            value["key_hints"]
                .as_array()
                .expect("key_hints")
                .iter()
                .any(|hint| hint == "q quit")
        );
    }

    #[test]
    fn dump_key_hints_follow_current_interaction_mode() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::SymbolSearch,
        )));
        let search = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert!(
            search["key_hints"]
                .as_array()
                .expect("key_hints")
                .iter()
                .any(|hint| hint == "enter select")
        );

        state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
        let help = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert!(
            help["key_hints"]
                .as_array()
                .expect("key_hints")
                .iter()
                .any(|hint| hint == "q quit")
        );
    }
}
