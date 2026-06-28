use std::path::PathBuf;

use agent_finance_core::{DiagnosticCheck, submit::SubmitMode};
use serde::Serialize;

use crate::account::AccountSnapshot;
use crate::config::ProviderConfig;
use crate::futures_state_ticket::FuturesStateTicketPreview;
use crate::hints;
use crate::model::{InteractionMode, Panel, WorkspaceKind};
use crate::order_ticket::OrderTicketPreview;
use crate::pane_status::{TuiPaneStatus, pane_health};
use crate::profile_snapshot::{ProfileValidationState, TradingProfileSnapshot};
use crate::provider_health::{ProviderHealthReport, ProviderHealthTask};
use crate::state::{AppState, StagedChangeView, StagedExecutionRequest};
use crate::theme::ThemeConfig;
use crate::transfer_ticket::TransferTicketPreview;

const TUI_DUMP_SCHEMA_VERSION: u32 = 25;

#[derive(Debug, Clone, Serialize)]
pub struct TuiDump {
    pub schema_version: u32,
    pub workspace: WorkspaceKind,
    pub mode: InteractionMode,
    pub selected_symbol: Option<String>,
    pub config_changes: Vec<String>,
    pub config_undo_available: bool,
    pub watchlist_add_query: String,
    pub partial: bool,
    pub panes: Vec<TuiPaneDump>,
    pub provider_health: ProviderHealthReport,
    pub provider_preferences: ProviderConfig,
    pub theme_preferences: ThemeConfig,
    pub tasks: Vec<ProviderHealthTask>,
    pub default_submit_mode: SubmitMode,
    pub live_writes_enabled: bool,
    pub effective_submit_mode: SubmitMode,
    pub trading_profile: Option<String>,
    pub profile_validation: ProfileValidationDump,
    pub account: Option<AccountSnapshot>,
    pub order_ticket: OrderTicketPreview,
    pub transfer_ticket: TransferTicketPreview,
    pub futures_state_ticket: FuturesStateTicketPreview,
    pub staged_changes: Vec<StagedChangeView>,
    pub pending_staged_confirmation: Option<StagedExecutionRequest>,
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
            config_undo_available: state.config_undo_available(),
            watchlist_add_query: state.watchlist_add.query().to_string(),
            partial,
            panes: Panel::ALL
                .into_iter()
                .map(|panel| pane_dump(state, panel))
                .collect(),
            provider_preferences: state.providers.clone(),
            theme_preferences: state.theme.clone(),
            tasks: provider_health.tasks.clone(),
            default_submit_mode: state.default_submit_mode,
            live_writes_enabled: state.live_writes_enabled,
            effective_submit_mode: state.effective_submit_mode(),
            trading_profile: state.trading_profile.clone(),
            profile_validation: ProfileValidationDump::from_state(&state.profile_validation),
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
    if let ProfileValidationState::Failed { profile, error } = &state.profile_validation {
        errors.push(format!("{profile} profile validation failed: {error}"));
    }
    if let ProfileValidationState::Ready {
        profile, checks, ..
    } = &state.profile_validation
    {
        let required_failure_count = required_failures(checks).len();
        if required_failure_count > 0 {
            errors.push(format!(
                "{profile} profile validation has {required_failure_count} required failure(s)"
            ));
        }
    }
    errors
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileValidationDump {
    pub status: ProfileValidationDumpStatus,
    pub profile: Option<String>,
    pub path: Option<PathBuf>,
    pub checks: Vec<DiagnosticCheck>,
    pub required_failure_count: usize,
    pub required_failures: Vec<DiagnosticCheck>,
    pub profile_snapshot: Option<TradingProfileSnapshot>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileValidationDumpStatus {
    Idle,
    Loading,
    Ready,
    Failed,
}

impl ProfileValidationDump {
    fn from_state(state: &ProfileValidationState) -> Self {
        match state {
            ProfileValidationState::Idle => Self {
                status: ProfileValidationDumpStatus::Idle,
                profile: None,
                path: None,
                checks: Vec::new(),
                required_failure_count: 0,
                required_failures: Vec::new(),
                profile_snapshot: None,
                error: None,
            },
            ProfileValidationState::Loading { profile } => Self {
                status: ProfileValidationDumpStatus::Loading,
                profile: Some(profile.clone()),
                path: None,
                checks: Vec::new(),
                required_failure_count: 0,
                required_failures: Vec::new(),
                profile_snapshot: None,
                error: None,
            },
            ProfileValidationState::Ready {
                profile,
                path,
                profile_config,
                checks,
                ..
            } => {
                let required_failures = required_failures(checks);
                Self {
                    status: ProfileValidationDumpStatus::Ready,
                    profile: Some(profile.clone()),
                    path: Some(path.clone()),
                    checks: checks.clone(),
                    required_failure_count: required_failures.len(),
                    required_failures,
                    profile_snapshot: Some(TradingProfileSnapshot::from(profile_config.as_ref())),
                    error: None,
                }
            }
            ProfileValidationState::Failed { profile, error } => Self {
                status: ProfileValidationDumpStatus::Failed,
                profile: Some(profile.clone()),
                path: None,
                checks: Vec::new(),
                required_failure_count: 0,
                required_failures: Vec::new(),
                profile_snapshot: None,
                error: Some(error.clone()),
            },
        }
    }
}

fn required_failures(checks: &[DiagnosticCheck]) -> Vec<DiagnosticCheck> {
    checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::{EquityProvider, ProviderConfig, TuiConfig, WorkspaceConfig};
    use crate::model::FloatingKind;
    use crate::profile_snapshot::{ProfileValidationSnapshot, test_profile};
    use crate::state::{Action, StagedChangeEvent};
    use crate::theme::{ThemeColor, ThemeConfig};
    use agent_finance_core::{Environment, Provider, SignedReadSnapshot};
    use std::path::PathBuf;

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
            Panel::ProfileRisk,
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
            theme: ThemeConfig {
                accent: ThemeColor::Blue,
                selection_background: ThemeColor::Magenta,
                selection_foreground: ThemeColor::White,
                ..ThemeConfig::default()
            },
            ..TuiConfig::default()
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");

        assert_eq!(value["workspace"], "market");
        assert_eq!(value["trading_profile"], "mainnet");
        assert_eq!(value["provider_preferences"]["equity"], "robinhood");
        assert_eq!(value["provider_preferences"]["crypto"], "okx");
        assert_eq!(value["theme_preferences"]["accent"], "blue");
        assert_eq!(
            value["theme_preferences"]["selection_background"],
            "magenta"
        );
        assert_eq!(value["theme_preferences"]["selection_foreground"], "white");
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
    fn dump_exposes_profile_validation_snapshot() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        let mut profile = test_profile("mainnet");
        profile.permissions.spot_trading = false;
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(
                &profile,
                PathBuf::from("mainnet.toml"),
            ),
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, false)).expect("serialize");

        assert_eq!(value["schema_version"], TUI_DUMP_SCHEMA_VERSION);
        assert_eq!(value["profile_validation"]["status"], "ready");
        assert_eq!(value["profile_validation"]["profile"], "mainnet");
        assert_eq!(value["profile_validation"]["required_failure_count"], 1);
        assert!(
            !value["profile_validation"]
                .as_object()
                .expect("profile_validation")
                .contains_key("profile_config")
        );
        assert_eq!(
            value["profile_validation"]["profile_snapshot"]["risk"]["allow_live"],
            true
        );
        assert_eq!(
            value["profile_validation"]["profile_snapshot"]["required_permissions"][0],
            "spot-trading"
        );
        assert_eq!(
            value["profile_validation"]["profile_snapshot"]["missing_permissions"][0],
            "spot-trading"
        );
        assert_eq!(
            value["profile_validation"]["profile_snapshot"]["risk"]["allowed_symbols"]["btcusdt"]["markets"]
                [0],
            "spot"
        );
        assert_eq!(
            value["profile_validation"]["checks"][0]["name"],
            "profile-parse"
        );
        assert_eq!(
            value["profile_validation"]["checks"][1]["message"],
            "permission is required by risk policy but not declared; risk.allowed_symbols includes spot markets"
        );
        assert_eq!(
            value["profile_validation"]["required_failures"][0]["name"],
            "profile-permission-spot-trading"
        );
        assert!(
            value["errors"]
                .as_array()
                .expect("errors")
                .iter()
                .any(|error| error == "mainnet profile validation has 1 required failure(s)")
        );
    }

    #[test]
    fn dump_exposes_profile_risk_review_without_profile_config_leak() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(
                &test_profile("mainnet"),
                PathBuf::from("mainnet.toml"),
            ),
        });

        state.reduce(Action::Execute(ActionId::StageProfileLiveToggle));

        let value = serde_json::to_value(TuiDump::from_state(&state, false)).expect("serialize");
        let staged = &value["staged_changes"][0];

        assert_eq!(staged["change_kind"], "profile-risk");
        assert!(staged["intent_kind"].is_null());
        assert!(staged["mode"].is_null());
        assert_eq!(staged["subject"]["type"], "profile-risk");
        assert_eq!(staged["subject"]["profile"], "mainnet");
        assert_eq!(staged["subject"]["change"]["field"], "allow-live");
        assert_eq!(staged["subject"]["change"]["before"], true);
        assert_eq!(staged["subject"]["change"]["after"], false);
        assert_eq!(
            staged["subject"]["diff"][0],
            "risk.allow_live: true -> false"
        );
        state.reduce(Action::ExecuteStagedChange);
        let value = serde_json::to_value(TuiDump::from_state(&state, false)).expect("serialize");
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["type"],
            "local-commit"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["type"],
            "profile-risk"
        );
        assert!(
            !value["profile_validation"]
                .as_object()
                .expect("profile_validation")
                .contains_key("profile_config")
        );
        assert!(
            value["profile_validation"]
                .as_object()
                .expect("profile_validation")
                .contains_key("profile_snapshot")
        );
    }

    #[test]
    fn dump_exposes_profile_validation_failure_in_errors() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("missing".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "missing".to_string(),
        });
        state.reduce(Action::ProfileValidationFailed {
            generation: 1,
            profile: "missing".to_string(),
            error: "profile not found".to_string(),
        });

        let value = serde_json::to_value(TuiDump::from_state(&state, false)).expect("serialize");

        assert_eq!(value["schema_version"], TUI_DUMP_SCHEMA_VERSION);
        assert_eq!(value["profile_validation"]["status"], "failed");
        assert_eq!(value["profile_validation"]["profile"], "missing");
        assert_eq!(value["profile_validation"]["required_failure_count"], 0);
        assert_eq!(value["profile_validation"]["error"], "profile not found");
        assert!(
            value["errors"]
                .as_array()
                .expect("errors")
                .iter()
                .any(|error| error == "missing profile validation failed: profile not found")
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
        assert_eq!(value["staged_changes"][0]["change_kind"], "order");
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
    fn dump_exposes_pending_staged_execution_confirmation() {
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

        state.reduce(Action::ExecuteStagedChange);

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["schema_version"], TUI_DUMP_SCHEMA_VERSION);
        assert_eq!(
            value["pending_staged_confirmation"]["id"],
            staged_change_id.as_str()
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["type"],
            "submit"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["mode"],
            "dry-run"
        );
        assert_eq!(value["staged_changes"][0]["stage"], "ready");
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["type"],
            "order-ticket"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["symbol"],
            "CRDO"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["quantity"],
            "0.05"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["price"],
            "204"
        );
    }

    #[test]
    fn dump_hides_profile_risk_local_commit_payload() {
        let mut state = AppState::from_config(TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Settings,
            },
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(
                &test_profile("mainnet"),
                PathBuf::from("/tmp/mainnet.toml"),
            ),
        });
        state.reduce(Action::Execute(ActionId::StageProfileLiveToggle));
        state.reduce(Action::ExecuteStagedChange);

        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["type"],
            "local-commit"
        );
        assert_eq!(
            value["pending_staged_confirmation"]["execution"]["subject"]["type"],
            "profile-risk"
        );
        assert!(value["staged_changes"][0]["mode"].is_null());
        assert!(
            value["staged_changes"][0]["subject"]["next_profile"].is_null(),
            "staged profile risk view must not expose the hidden write payload"
        );
        assert!(
            value["pending_staged_confirmation"]["execution"]["subject"]["next_profile"].is_null(),
            "pending local commit confirmation must not expose the hidden write payload"
        );

        let profile_risk_views = [
            &value["staged_changes"][0]["subject"],
            &value["pending_staged_confirmation"]["execution"]["subject"],
        ];
        for hidden in [
            "next_profile",
            "expected_content_hash",
            "api_key_env",
            "api_secret_env",
            "BINANCE_API_KEY",
            "BINANCE_PRIVATE_KEY",
            "provider",
        ] {
            for profile_risk_view in profile_risk_views {
                let serialized =
                    serde_json::to_string(profile_risk_view).expect("serialize profile risk view");
                assert!(
                    !serialized.contains(hidden),
                    "profile risk dump leaked hidden profile detail: {hidden}"
                );
            }
        }
    }

    #[test]
    fn dump_exposes_watchlist_edit_state_for_agents() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::WatchlistAdd,
        )));
        let value = serde_json::to_value(TuiDump::from_state(&state, true)).expect("serialize");
        assert_eq!(value["watchlist_add_query"], "");
        assert_eq!(value["config_undo_available"], false);
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
        assert_eq!(value["config_undo_available"], true);
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
