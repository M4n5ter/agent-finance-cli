use agent_finance_core::submit::SubmitMode;
use serde::Serialize;

use crate::account::AccountSnapshot;
use crate::config::ProviderConfig;
use crate::futures_state_ticket::FuturesStateTicketPreview;
use crate::hints;
use crate::model::{InteractionMode, Panel, WorkspaceKind};
use crate::order_ticket::OrderTicketPreview;
use crate::pane_status::{TuiPaneStatus, pane_health};
use crate::provider_health::{ProviderHealthReport, ProviderHealthTask};
use crate::state::{AppState, StagedChangeView, StagedSubmitRequest};
use crate::transfer_ticket::TransferTicketPreview;

const TUI_DUMP_SCHEMA_VERSION: u32 = 13;

#[derive(Debug, Clone, Serialize)]
pub struct TuiDump {
    pub schema_version: u32,
    pub workspace: WorkspaceKind,
    pub mode: InteractionMode,
    pub selected_symbol: Option<String>,
    pub config_changes: Vec<String>,
    pub watchlist_add_query: String,
    pub partial: bool,
    pub panes: Vec<TuiPaneDump>,
    pub provider_health: ProviderHealthReport,
    pub provider_preferences: ProviderConfig,
    pub tasks: Vec<ProviderHealthTask>,
    pub default_submit_mode: SubmitMode,
    pub live_writes_enabled: bool,
    pub effective_submit_mode: SubmitMode,
    pub trading_profile: Option<String>,
    pub account: Option<AccountSnapshot>,
    pub order_ticket: OrderTicketPreview,
    pub transfer_ticket: TransferTicketPreview,
    pub futures_state_ticket: FuturesStateTicketPreview,
    pub staged_changes: Vec<StagedChangeView>,
    pub pending_staged_confirmation: Option<StagedSubmitRequest>,
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
            schema_version: TUI_DUMP_SCHEMA_VERSION,
            workspace: state.workspace,
            mode: state.interaction_mode(),
            selected_symbol: state.selected_symbol().map(ToString::to_string),
            config_changes: state.config_changes.clone(),
            watchlist_add_query: state.watchlist_add.query().to_string(),
            partial,
            panes: Panel::ALL
                .into_iter()
                .map(|panel| pane_dump(state, panel))
                .collect(),
            provider_preferences: state.providers.clone(),
            tasks: provider_health.tasks.clone(),
            default_submit_mode: state.default_submit_mode,
            live_writes_enabled: state.live_writes_enabled,
            effective_submit_mode: state.effective_submit_mode(),
            trading_profile: state.trading_profile.clone(),
            account: state.account_snapshot.clone(),
            order_ticket: state.order_ticket_preview(),
            transfer_ticket: state.transfer_ticket_preview(),
            futures_state_ticket: state.futures_state_ticket_preview(),
            staged_changes: state.staged_change_views(),
            pending_staged_confirmation: state.pending_staged_confirmation().cloned(),
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
                .map(|error| format!("{} account read warning: {}", error.label, error.error)),
        );
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::{EquityProvider, ProviderConfig, TuiConfig, WorkspaceConfig};
    use crate::model::FloatingKind;
    use crate::state::{Action, StagedChangeEvent};
    use agent_finance_core::submit::SubmitMode;
    use agent_finance_core::{Environment, Provider, SignedReadSnapshot};

    #[test]
    fn dump_marks_only_workspace_panels_visible() {
        let state = AppState::from_config(TuiConfig {
            watchlist: vec!["AAPL".to_string(), "BTCUSDT".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Research,
            },
            ..TuiConfig::default()
        });

        let dump = TuiDump::from_state(&state, true);

        assert_eq!(dump.workspace, WorkspaceKind::Research);
        assert_eq!(dump.selected_symbol.as_deref(), Some("AAPL"));
        assert!(dump.partial);
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Evidence && pane.visible)
        );
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Research && pane.visible)
        );
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::History && !pane.visible)
        );
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Settings && !pane.visible)
        );
    }

    #[test]
    fn dump_exposes_settings_workspace_panels() {
        let state = AppState::from_config(TuiConfig {
            watchlist: vec!["AAPL".to_string(), "BTCUSDT".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Settings,
            },
            ..TuiConfig::default()
        });

        let dump = TuiDump::from_state(&state, false);

        assert_eq!(dump.workspace, WorkspaceKind::Settings);
        for panel in [
            Panel::Settings,
            Panel::Watchlist,
            Panel::ProviderHealth,
            Panel::TaskLog,
        ] {
            assert!(
                dump.panes
                    .iter()
                    .any(|pane| pane.panel == panel && pane.visible),
                "{panel:?} should be visible in settings workspace"
            );
        }
        assert!(
            dump.panes
                .iter()
                .any(|pane| pane.panel == Panel::Quote && !pane.visible)
        );
    }

    #[test]
    fn dump_serializes_agent_facing_names() {
        let state = AppState::from_config(TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Market,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            providers: ProviderConfig {
                equity: EquityProvider::Robinhood,
                crypto: agent_finance_market::args::CryptoProvider::Okx,
            },
            ..TuiConfig::default()
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["workspace"], "market");
        assert_eq!(value["trading_profile"], "mainnet");
        assert_eq!(value["provider_preferences"]["equity"], "robinhood");
        assert_eq!(value["provider_preferences"]["crypto"], "okx");
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
    fn dump_includes_default_submit_mode_and_staged_changes() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.watchlist = vec!["CRDO".to_string()];
        state.trading_profile = Some("mainnet".to_string());
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));
        state.reduce(Action::StageOrderTicket);
        let staged_change_id = state.staged_change_views()[0].id.clone();
        state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["schema_version"], TUI_DUMP_SCHEMA_VERSION);
        assert_eq!(value["default_submit_mode"], "live");
        assert_eq!(value["live_writes_enabled"], false);
        assert_eq!(value["effective_submit_mode"], "dry-run");
        assert_eq!(value["staged_changes"][0]["intent_kind"], "order");
        assert_eq!(value["staged_changes"][0]["selected"], true);
        assert!(value["pending_staged_confirmation"].is_null());
        assert_eq!(value["staged_changes"][0]["stage"], "ready");
        assert_eq!(value["staged_changes"][0]["mode"], "dry-run");
        assert!(value["staged_changes"][0]["intent_status"].is_null());
        state.reduce(Action::ApplyStagedChangeEvent {
            id: staged_change_id.clone(),
            event: StagedChangeEvent::SubmitQueued,
        });
        state.reduce(Action::ApplyStagedChangeEvent {
            id: staged_change_id.clone(),
            event: StagedChangeEvent::IntentCreated {
                intent_id: "intent-1".to_string(),
            },
        });
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["staged_changes"][0]["stage"], "intent-created");
        assert_eq!(value["staged_changes"][0]["mode"], "dry-run");
        assert_eq!(value["staged_changes"][0]["intent_id"], "intent-1");
        assert!(value["staged_changes"][0]["intent_status"].is_null());

        state.reduce(Action::ApplyStagedChangeEvent {
            id: staged_change_id.clone(),
            event: StagedChangeEvent::NonConsumingFinished {
                intent_id: "intent-1".to_string(),
                mode: SubmitMode::DryRun,
            },
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["staged_changes"][0]["stage"], "dry-run-completed");
        assert_eq!(value["staged_changes"][0]["mode"], "dry-run");
        assert_eq!(value["staged_changes"][0]["intent_id"], "intent-1");
        assert!(value["staged_changes"][0]["intent_status"].is_null());

        state.reduce(Action::ApplyStagedChangeEvent {
            id: staged_change_id,
            event: StagedChangeEvent::LiveIntentClaimed {
                intent_id: "intent-1".to_string(),
            },
        });
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["staged_changes"][0]["stage"], "dry-run-completed");
        assert_eq!(value["staged_changes"][0]["mode"], "dry-run");
        assert_eq!(value["staged_changes"][0]["intent_id"], "intent-1");
        assert!(value["staged_changes"][0]["intent_status"].is_null());
    }

    #[test]
    fn dump_exposes_pending_staged_submit_confirmation() {
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
        state.reduce(Action::StageOrderTicket);
        let staged_change_id = state.staged_change_views()[0].id.clone();

        state.reduce(Action::SubmitStagedChange);

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["schema_version"], TUI_DUMP_SCHEMA_VERSION);
        assert_eq!(
            value["pending_staged_confirmation"]["id"],
            staged_change_id.as_str()
        );
        assert_eq!(value["pending_staged_confirmation"]["mode"], "dry-run");
        assert_eq!(value["staged_changes"][0]["stage"], "ready");
        assert_eq!(
            value["pending_staged_confirmation"]["subject"]["type"],
            "order-ticket"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["subject"]["symbol"],
            "CRDO"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["subject"]["quantity"],
            "0.05"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["subject"]["price"],
            "204"
        );
    }

    #[test]
    fn dump_exposes_watchlist_edit_state_for_agents() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::WatchlistAdd,
        )));
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["watchlist_add_query"], "");
        assert_eq!(
            value["key_hints"],
            serde_json::json!(["type symbols", "enter add", "esc close"])
        );

        for character in "lite".chars() {
            state.reduce(Action::EditWatchlistAddQuery(
                tui_input::InputRequest::InsertChar(character),
            ));
        }
        state.reduce(Action::AcceptWatchlistAdd);

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["config_changes"][0], "watchlist");
        assert_eq!(value["selected_symbol"], "LITE");
        assert_eq!(value["watchlist_add_query"], "");
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
    fn dump_exposes_transfer_ticket_readiness_for_agents() {
        let mut state = AppState::from_config(TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.transfer_ticket.set_amount_text(Some("5".to_string()));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(
            value["transfer_ticket"]["direction"],
            "spot-to-usds-futures"
        );
        assert_eq!(value["transfer_ticket"]["asset"], "USDT");
        assert_eq!(value["transfer_ticket"]["amount"], "5");
        assert_eq!(value["transfer_ticket"]["ready"], true);
    }

    #[test]
    fn dump_exposes_futures_state_ticket_readiness_for_agents() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["ETHUSDT".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.futures_state_ticket.set_leverage(Some(2));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["futures_state_ticket"]["kind"], "leverage");
        assert_eq!(value["futures_state_ticket"]["symbol"], "ETHUSDT");
        assert_eq!(value["futures_state_ticket"]["leverage"], 2);
        assert_eq!(value["futures_state_ticket"]["ready"], true);
    }

    #[test]
    fn dump_exposes_position_mode_as_account_wide_for_agents() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["ETHUSDT".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::AdjustFuturesStateTicketField(-1));
        state.reduce(Action::MoveFuturesStateTicketField(1));
        state.reduce(Action::AdjustFuturesStateTicketField(1));

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["futures_state_ticket"]["kind"], "position-mode");
        assert_eq!(
            value["futures_state_ticket"]["symbol"],
            serde_json::Value::Null
        );
        assert_eq!(value["futures_state_ticket"]["position_mode"], "hedge");
        assert_eq!(value["futures_state_ticket"]["ready"], true);
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
        assert_eq!(
            value["account"]["profile_config"]["risk"]["allow_live"],
            true
        );
        assert_eq!(
            value["account"]["profile_config"]["risk"]["allowed_symbols"]["btcusdt"]["max_order_notional_usdt"],
            "50"
        );
        assert_eq!(
            value["account"]["profile_config"]["missing_permissions"]
                .as_array()
                .expect("missing permissions")
                .len(),
            0
        );
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
        let failed_plan = crate::account::ACCOUNT_READ_PLAN
            .into_iter()
            .find(|plan| plan.label() == "USD-M open orders")
            .expect("USD-M open orders plan");
        snapshot
            .reads
            .retain(|read| read.request != failed_plan.request());
        snapshot.errors.push(AccountReadError::from_plan(
            &failed_plan,
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
                    text.contains("USD-M open orders account read warning")
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
            crate::profile_snapshot::test_trading_profile_snapshot(),
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
