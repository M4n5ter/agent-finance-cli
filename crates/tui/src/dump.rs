use agent_finance_core::submit::SubmitMode;
use serde::Serialize;

use crate::account::AccountSnapshot;
use crate::hints;
use crate::model::{InteractionMode, Panel, WorkspaceKind};
use crate::order_ticket::OrderTicketPreview;
use crate::pane_status::{TuiPaneStatus, pane_health};
use crate::provider_health::{ProviderHealthReport, ProviderHealthTask};
use crate::state::{AppState, WriteSessionView};

#[derive(Debug, Clone, Serialize)]
pub struct TuiDump {
    pub workspace: WorkspaceKind,
    pub mode: InteractionMode,
    pub selected_symbol: Option<String>,
    pub partial: bool,
    pub panes: Vec<TuiPaneDump>,
    pub provider_health: ProviderHealthReport,
    pub tasks: Vec<ProviderHealthTask>,
    pub default_submit_mode: SubmitMode,
    pub live_writes_enabled: bool,
    pub effective_submit_mode: SubmitMode,
    pub trading_profile: Option<String>,
    pub account: Option<AccountSnapshot>,
    pub order_ticket: OrderTicketPreview,
    pub write_sessions: Vec<WriteSessionView>,
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
            default_submit_mode: state.default_submit_mode,
            live_writes_enabled: state.live_writes_enabled,
            effective_submit_mode: state.effective_submit_mode(),
            trading_profile: state.trading_profile.clone(),
            account: state.account_snapshot.clone(),
            order_ticket: state.order_ticket_preview(),
            write_sessions: state.write_session_views(),
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
    if let Some(account) = state.account_snapshot.as_ref() {
        errors.extend(
            account
                .errors
                .iter()
                .map(|error| format!("{} account read warning: {}", error.kind, error.error)),
        );
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::{TuiConfig, WorkspaceConfig};
    use crate::model::FloatingKind;
    use crate::state::{Action, WriteSessionEvent, WriteSessionRequest};
    use agent_finance_core::submit::{SubmitIntentKind, SubmitMode};
    use agent_finance_core::{Environment, Provider, SignedReadSnapshot};

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
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["workspace"], "providers");
        assert_eq!(value["trading_profile"], "mainnet");
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

    #[test]
    fn dump_includes_default_submit_mode_and_write_sessions() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
        state.reduce(Action::OpenWriteSession(WriteSessionRequest {
            id: "watchlist-add-crdo".to_string(),
            intent_kind: SubmitIntentKind::Order,
            summary: "Add CRDO to watchlist".to_string(),
        }));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["default_submit_mode"], "live");
        assert_eq!(value["live_writes_enabled"], false);
        assert_eq!(value["effective_submit_mode"], "dry-run");
        assert_eq!(value["write_sessions"][0]["intent_kind"], "order");
        assert_eq!(value["write_sessions"][0]["stage"], "draft");
        assert_eq!(value["write_sessions"][0]["mode"], "dry-run");
        assert!(value["write_sessions"][0]["intent_status"].is_null());
        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::ValidationStarted,
        });
        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::ValidationReady,
        });
        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::ConfirmationRequested,
        });
        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::IntentCreated {
                intent_id: "intent-1".to_string(),
            },
        });
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["write_sessions"][0]["stage"], "intent-created");
        assert_eq!(value["write_sessions"][0]["mode"], "dry-run");
        assert_eq!(value["write_sessions"][0]["intent_id"], "intent-1");
        assert!(value["write_sessions"][0]["intent_status"].is_null());

        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::NonConsumingFinished {
                intent_id: "intent-1".to_string(),
                mode: SubmitMode::DryRun,
            },
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["write_sessions"][0]["stage"], "dry-run-completed");
        assert_eq!(value["write_sessions"][0]["mode"], "dry-run");
        assert_eq!(value["write_sessions"][0]["intent_id"], "intent-1");
        assert!(value["write_sessions"][0]["intent_status"].is_null());

        state.reduce(Action::ApplyWriteSessionEvent {
            id: "watchlist-add-crdo".to_string(),
            event: WriteSessionEvent::LiveSubmitStarted {
                intent_id: "intent-1".to_string(),
            },
        });
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["write_sessions"][0]["stage"], "dry-run-completed");
        assert_eq!(value["write_sessions"][0]["mode"], "dry-run");
        assert_eq!(value["write_sessions"][0]["intent_id"], "intent-1");
        assert!(value["write_sessions"][0]["intent_status"].is_null());
    }

    #[test]
    fn dump_exposes_order_ticket_readiness_for_agents() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["order_ticket"]["symbol"], "CRDO");
        assert_eq!(value["order_ticket"]["profile"], "mainnet");
        assert_eq!(value["order_ticket"]["quantity"], "0.05");
        assert_eq!(value["order_ticket"]["price"], "204");
        assert_eq!(value["order_ticket"]["ready"], true);
        assert!(
            value["panes"]
                .as_array()
                .expect("panes")
                .iter()
                .any(|pane| pane["panel"] == "order-ticket" && pane["visible"] == true)
        );
    }

    #[test]
    fn dump_exposes_signed_account_snapshot_for_agents() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::AccountStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(Action::AccountLoaded {
            generation: 1,
            snapshot: account_snapshot("mainnet"),
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["account"]["profile"], "mainnet");
        assert_eq!(value["account"]["provider"], "binance");
        assert_eq!(value["account"]["environment"], "live");
        assert_eq!(value["account"]["reads"][0]["kind"], "api-permissions");
    }

    #[test]
    fn dump_surfaces_partial_account_read_warnings() {
        let mut state = AppState::from_config(TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("testnet".to_string()),
            },
            ..TuiConfig::default()
        });
        let mut snapshot = account_snapshot("testnet");
        snapshot.reads.pop();
        snapshot.errors.push(AccountReadError::new(
            agent_finance_core::SignedReadSnapshotKind::UsdsFuturesPositions,
            "futures account timeout",
        ));
        state.reduce(Action::AccountStarted {
            generation: 1,
            profile: "testnet".to_string(),
        });
        state.reduce(Action::AccountLoaded {
            generation: 1,
            snapshot,
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, false)).expect("serialize");

        assert!(
            value["errors"]
                .as_array()
                .expect("errors")
                .iter()
                .any(|error| error.as_str().is_some_and(|text| {
                    text.contains("usds-futures-positions account read warning")
                        && text.contains("futures account timeout")
                }))
        );
        assert!(
            value["panes"]
                .as_array()
                .expect("panes")
                .iter()
                .any(|pane| pane["panel"] == "account" && pane["status"] == "stale")
        );
    }

    use crate::AccountReadError;

    fn account_snapshot(profile: &str) -> AccountSnapshot {
        AccountSnapshot::new(
            profile.to_string(),
            Provider::Binance,
            Environment::Live,
            crate::account::ACCOUNT_READ_PLAN
                .into_iter()
                .map(|plan| {
                    SignedReadSnapshot::new(
                        profile.to_string(),
                        Provider::Binance,
                        Environment::Live,
                        plan.request(),
                        serde_json::json!({ "ok": true }),
                    )
                })
                .collect(),
            Vec::new(),
        )
    }
}
