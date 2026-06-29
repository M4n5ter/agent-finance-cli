use super::*;
use crate::account::ACCOUNT_READ_PLAN;
use crate::command::ActionId;
use crate::config::MAX_LEFT_MAIN_RATIO;
use crate::model::InteractionMode;
use crate::profile_snapshot::{ProfileValidationSnapshot, ProfileValidationState, test_profile};
use crate::settings_editor::SettingRow;
use crate::task_failure::TaskFailureSource;
use crate::task_log::TaskStatus;
use crate::theme::ThemeColor;
use agent_finance_core::intent::IntentStatus;
use agent_finance_core::submit::{SubmitIntentKind, SubmitMode};
use agent_finance_core::{
    Environment, FuturesStateChange, Market, OrderSpec, Provider, SignedReadRequest,
    SignedReadSnapshot,
};
use agent_finance_market::args::CryptoProvider;
use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::snapshot::{QuoteSnapshot, RegularBasisSnapshot};
use std::path::PathBuf;

fn toggle_panel_action(panel: Panel) -> ActionId {
    ActionId::TogglePanel(panel)
}

fn move_to_setting(state: &mut AppState, label: &str) {
    let index = SettingRow::ALL
        .iter()
        .position(|row| row.label() == label)
        .expect("setting row exists");
    for _ in 0..index {
        state.reduce(Action::MoveSettingsSelection(1));
    }
}

fn request_and_confirm_selected_staged_submit(state: &mut AppState) -> StagedSubmitRequest {
    state.reduce(Action::ExecuteStagedChange);
    assert!(state.take_pending_staged_execution().is_none());
    assert!(state.pending_staged_confirmation().is_some());
    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );

    state.reduce(Action::ConfirmStagedExecution);

    assert!(state.pending_staged_confirmation().is_none());
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::StagedExecutionConfirmation)
    );
    state
        .take_pending_staged_execution()
        .and_then(|request| match request.execution {
            StagedExecution::Submit { subject, mode } => Some(StagedSubmitRequest {
                id: request.id,
                subject,
                mode,
            }),
            StagedExecution::LocalCommit { .. } => None,
        })
        .expect("pending staged submit")
}

fn type_staged_confirmation(state: &mut AppState, text: &str) {
    for character in text.chars() {
        state.reduce(Action::EditStagedExecutionConfirmation(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
}

#[test]
fn reducer_wraps_symbol_focus_across_watchlist_boundaries() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
        ..TuiConfig::default()
    });

    state.reduce(Action::Execute(ActionId::SelectSymbolBy(-1)));

    assert_eq!(state.selected_symbol(), Some("CRDO"));

    state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));

    assert_eq!(state.selected_symbol(), Some("AAPL"));
}

#[test]
fn live_write_gate_is_session_only_and_not_exported_to_config() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::SetLiveWritesEnabled(true));
    let exported = state.export_config(&TuiConfig::default());
    let restored = AppState::from_config(exported);

    assert_eq!(state.default_submit_mode, SubmitMode::Live);
    assert!(state.live_writes_enabled);
    assert_eq!(state.effective_submit_mode(), SubmitMode::Live);
    assert_eq!(restored.default_submit_mode, SubmitMode::DryRun);
    assert!(!restored.live_writes_enabled);
    assert_eq!(restored.effective_submit_mode(), SubmitMode::DryRun);
}

#[test]
fn staged_changes_use_dry_run_until_live_writes_are_confirmed() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));

    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Protected order",
    )));

    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.mode, Some(SubmitMode::DryRun));

    state.reduce(Action::CloseStagedChange("order-1".to_string()));
    state.reduce(Action::SetLiveWritesEnabled(true));
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-2",
        SubmitIntentKind::Order,
        "Confirmed live order",
    )));

    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.mode, Some(SubmitMode::Live));
}

#[test]
fn order_ticket_staging_requires_core_valid_preview_before_review_change() {
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

    state.reduce(Action::StageOrderTicket);

    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert!(state.staged_change_views().is_empty());

    state
        .order_ticket
        .set_quantity_text(Some("0.05".to_string()));
    state.order_ticket.set_price_text(Some("204".to_string()));
    state.reduce(Action::StageOrderTicket);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_kind, StagedChangeKind::Order);
    assert_eq!(changes[0].intent_kind, Some(SubmitIntentKind::Order));
    assert_eq!(changes[0].stage, StagedChangeStage::Ready);
    assert_eq!(changes[0].mode, Some(SubmitMode::DryRun));
    assert!(changes[0].intent_id.is_none());
    assert!(changes[0].summary.contains("CRDO"));
    let StagedChangeSubject::OrderTicket(review) = &changes[0].subject else {
        panic!("staged order ticket");
    };
    assert_eq!(review.parsed_quantity.to_string(), "0.05");
    assert!(matches!(
        review.order_spec,
        OrderSpec::PostOnlyLimit { ref price } if price.to_string() == "204"
    ));
}

#[test]
fn submitting_ready_order_change_queues_submit_request() {
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

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let crate::state::StagedSubmitSubject::OrderTicket(review) = &request.subject else {
        panic!("expected order submit");
    };
    assert_eq!(review.symbol, "CRDO");
    assert_eq!(request.mode, SubmitMode::DryRun);
    assert!(state.take_pending_staged_execution().is_none());
    let change = state.staged_change_views().pop().unwrap();
    assert_eq!(change.stage, StagedChangeStage::SubmitQueued);
}

#[test]
fn cancelling_staged_execution_confirmation_returns_change_to_ready() {
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

    state.reduce(Action::ExecuteStagedChange);
    assert!(state.pending_staged_confirmation().is_some());
    assert_eq!(
        state.staged_change_views()[0].stage,
        StagedChangeStage::Ready
    );

    state.reduce(Action::CancelStagedExecutionConfirmation);

    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(
        state.staged_change_views()[0].stage,
        StagedChangeStage::Ready
    );
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::StagedExecutionConfirmation)
    );
}

#[test]
fn intent_review_submission_uses_selected_staged_change() {
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
    state.order_ticket.set_price_text(Some("198".to_string()));
    state.reduce(Action::StageOrderTicket);
    state.reduce(Action::MoveStagedChangeSelection(-1));

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let StagedSubmitSubject::OrderTicket(review) = &request.subject else {
        panic!("expected order submit");
    };
    assert_eq!(review.price.as_deref(), Some("204"));
    let changes = state.staged_change_views();
    assert!(
        changes
            .iter()
            .any(|change| change.selected && change.stage == StagedChangeStage::SubmitQueued)
    );
    assert!(
        changes
            .iter()
            .any(|change| !change.selected && change.stage == StagedChangeStage::Ready)
    );
}

#[test]
fn selected_staged_change_can_be_closed_from_review() {
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
    state.order_ticket.set_price_text(Some("198".to_string()));
    state.reduce(Action::StageOrderTicket);
    state.reduce(Action::MoveStagedChangeSelection(-1));

    state.reduce(Action::CloseSelectedStagedChange);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].selected);
    assert!(changes[0].summary.contains("198"));
}

#[test]
fn transfer_ticket_staging_creates_transfer_review_change() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::StageTransferTicket);

    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert!(state.staged_change_views().is_empty());

    state.transfer_ticket.set_amount_text(Some("5".to_string()));
    state.reduce(Action::StageTransferTicket);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_kind, StagedChangeKind::Transfer);
    assert_eq!(changes[0].intent_kind, Some(SubmitIntentKind::Transfer));
    assert_eq!(changes[0].stage, StagedChangeStage::Ready);
    assert_eq!(changes[0].mode, Some(SubmitMode::DryRun));
    assert!(changes[0].summary.contains("spot-to-usds-futures"));
    let StagedChangeSubject::Transfer(review) = &changes[0].subject else {
        panic!("staged transfer");
    };
    assert_eq!(review.profile, "mainnet");
    assert_eq!(review.asset, "USDT");
    assert_eq!(review.amount, "5");
    assert_eq!(review.parsed_amount.to_string(), "5");
}

#[test]
fn submitting_ready_transfer_change_queues_transfer_submit_request() {
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
    state.reduce(Action::StageTransferTicket);

    state.reduce(Action::ExecuteStagedChange);
    assert!(state.pending_staged_confirmation().is_some());
    state.reduce(Action::ConfirmStagedExecution);
    assert!(state.pending_staged_confirmation().is_some());
    assert!(state.take_pending_staged_execution().is_none());
    type_staged_confirmation(&mut state, " TRANSFER ");
    state.reduce(Action::ConfirmStagedExecution);
    assert!(state.pending_staged_confirmation().is_some());
    assert!(state.take_pending_staged_execution().is_none());
    for _ in 0.." TRANSFER ".chars().count() {
        state.reduce(Action::EditStagedExecutionConfirmation(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    type_staged_confirmation(&mut state, "TRANSFER");

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let StagedSubmitSubject::Transfer(review) = &request.subject else {
        panic!("expected transfer submit");
    };
    assert_eq!(review.asset, "USDT");
    assert_eq!(request.mode, SubmitMode::DryRun);
    let change = state.staged_change_views().pop().unwrap();
    assert_eq!(change.stage, StagedChangeStage::SubmitQueued);
}

#[test]
fn futures_state_ticket_staging_requires_value_then_creates_review_change() {
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

    state.reduce(Action::StageFuturesStateTicket);

    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert!(state.staged_change_views().is_empty());

    state.futures_state_ticket.set_leverage(Some(2));
    state.reduce(Action::StageFuturesStateTicket);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_kind, StagedChangeKind::FuturesState);
    assert_eq!(changes[0].intent_kind, Some(SubmitIntentKind::FuturesState));
    assert_eq!(changes[0].stage, StagedChangeStage::Ready);
    assert_eq!(changes[0].mode, Some(SubmitMode::DryRun));
    let StagedChangeSubject::FuturesState(review) = &changes[0].subject else {
        panic!("staged futures state");
    };
    assert_eq!(review.profile, "mainnet");
    assert_eq!(
        review.change,
        FuturesStateChange::Leverage {
            symbol: "ETHUSDT".to_string(),
            leverage: 2,
        }
    );
    assert!(changes[0].summary.contains("ETHUSDT 2"));
}

#[test]
fn futures_state_ticket_does_not_stage_non_futures_symbol_by_fallback() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string()],
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..TuiConfig::default()
    });
    state.futures_state_ticket.set_leverage(Some(2));

    state.reduce(Action::StageFuturesStateTicket);

    assert!(state.staged_change_views().is_empty());
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message.contains("USD-M futures symbol is required"))
    );
}

#[test]
fn futures_state_staged_reviews_include_changed_target_values() {
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
    state.reduce(Action::StageFuturesStateTicket);
    state.futures_state_ticket.set_leverage(Some(5));
    state.reduce(Action::StageFuturesStateTicket);

    let summaries = state
        .staged_change_views()
        .into_iter()
        .map(|change| change.summary)
        .collect::<Vec<_>>();
    assert_eq!(summaries.len(), 2);
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("ETHUSDT 2"))
    );
    assert!(
        summaries
            .iter()
            .any(|summary| summary.contains("ETHUSDT 5"))
    );
}

#[test]
fn submitting_ready_futures_state_change_queues_futures_state_submit_request() {
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
    state.reduce(Action::StageFuturesStateTicket);

    state.reduce(Action::ExecuteStagedChange);
    assert!(state.pending_staged_confirmation().is_some());
    state.reduce(Action::ConfirmStagedExecution);
    assert!(state.pending_staged_confirmation().is_some());
    assert!(state.take_pending_staged_execution().is_none());
    type_staged_confirmation(&mut state, "FUTURES STATE");

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let StagedSubmitSubject::FuturesState(review) = &request.subject else {
        panic!("expected futures state submit");
    };
    assert_eq!(review.change.kind().to_string(), "leverage");
    assert_eq!(request.mode, SubmitMode::DryRun);
    let change = state.staged_change_views().pop().unwrap();
    assert_eq!(change.stage, StagedChangeStage::SubmitQueued);
}

#[test]
fn selected_open_order_can_be_staged_as_cancel_request() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
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
        snapshot: account_snapshot_with_open_orders("mainnet"),
    });
    state.reduce(Action::MoveOpenOrderSelection(1));

    state.reduce(Action::StageSelectedOpenOrderCancel);

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let crate::state::StagedSubmitSubject::Cancel(review) = &request.subject else {
        panic!("expected cancel submit");
    };
    assert_eq!(review.market, Market::UsdsFutures);
    assert_eq!(review.symbol, "ETHUSDT");
    assert_eq!(
        review.target,
        agent_finance_core::OrderIdentifier::ClientOrderId {
            client_order_id: "futures-order".to_string()
        }
    );
    assert_eq!(request.mode, SubmitMode::DryRun);
}

#[test]
fn selected_open_order_cancel_preserves_exchange_order_id_target() {
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
        snapshot: account_snapshot_with_order_id_only_open_order("mainnet"),
    });

    state.reduce(Action::StageSelectedOpenOrderCancel);

    let request = request_and_confirm_selected_staged_submit(&mut state);
    let crate::state::StagedSubmitSubject::Cancel(review) = &request.subject else {
        panic!("expected cancel submit");
    };
    assert_eq!(
        review.target,
        agent_finance_core::OrderIdentifier::OrderId {
            order_id: "3001".to_string()
        }
    );
}

#[test]
fn trading_profile_change_invalidates_account_snapshot_before_cancel() {
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
        snapshot: account_snapshot_with_open_orders("mainnet"),
    });
    assert!(state.account_snapshot.is_some());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);
    state.reduce(Action::StageSelectedOpenOrderCancel);

    assert_eq!(state.trading_profile.as_deref(), Some("hedge"));
    assert!(state.account_snapshot.is_none());
    assert!(state.staged_change_views().is_empty());
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message.contains("no open order selected"))
    );
}

#[test]
fn cancel_stage_id_separates_dry_run_and_live_for_same_open_order() {
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
        snapshot: account_snapshot_with_open_orders("mainnet"),
    });
    state.reduce(Action::StageSelectedOpenOrderCancel);
    state.reduce(Action::ExecuteStagedChange);
    state.reduce(Action::ConfirmStagedExecution);
    let dry_run_id = state.staged_change_views()[0].id.clone();
    for event in [
        StagedChangeEvent::IntentCreated {
            intent_id: "dry-run-intent".to_string(),
        },
        StagedChangeEvent::NonConsumingFinished {
            intent_id: "dry-run-intent".to_string(),
            mode: SubmitMode::DryRun,
        },
    ] {
        state.reduce(Action::ApplyStagedChangeEvent {
            id: dry_run_id.clone(),
            event,
        });
    }

    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::SetLiveWritesEnabled(true));
    state.reduce(Action::StageSelectedOpenOrderCancel);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .any(|change| change.mode == Some(SubmitMode::DryRun))
    );
    assert!(
        changes
            .iter()
            .any(|change| change.mode == Some(SubmitMode::Live))
    );
}

#[test]
fn order_ticket_staging_keeps_risk_semantics_in_frozen_changes() {
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
    state.order_ticket.set_reduce_only(true);
    state.reduce(Action::StageOrderTicket);

    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .any(|change| !change.summary.contains("reduce-only"))
    );
    assert!(
        changes
            .iter()
            .any(|change| change.summary.contains("reduce-only"))
    );
}

#[test]
fn reducer_tracks_staged_change_workflow_without_accepting_unsafe_jumps() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::SetLiveWritesEnabled(true));
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Buy BTCUSDT",
    )));

    state.reduce(Action::ApplyStagedChangeEvent {
        id: "order-1".to_string(),
        event: StagedChangeEvent::LiveSubmitSucceeded {
            intent_id: "intent-1".to_string(),
        },
    });
    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::Draft);

    for event in [
        StagedChangeEvent::ValidationStarted,
        StagedChangeEvent::ValidationReady,
        StagedChangeEvent::SubmitQueued,
        StagedChangeEvent::IntentCreated {
            intent_id: "intent-1".to_string(),
        },
        StagedChangeEvent::LiveIntentClaimed {
            intent_id: "intent-1".to_string(),
        },
        StagedChangeEvent::LiveSubmitSucceeded {
            intent_id: "intent-1".to_string(),
        },
    ] {
        state.reduce(Action::ApplyStagedChangeEvent {
            id: "order-1".to_string(),
            event,
        });
    }

    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::LiveSubmitted);
    assert_eq!(view.intent_id.as_deref(), Some("intent-1"));
    assert_eq!(view.intent_status, Some(IntentStatus::Submitted));

    state.reduce(Action::CloseStagedChange("order-1".to_string()));
    assert_eq!(state.staged_change_views().len(), 0);
}

#[test]
fn reducer_keeps_live_claimed_staged_change_until_terminal_event() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::SetLiveWritesEnabled(true));
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Buy BTCUSDT",
    )));
    for event in [
        StagedChangeEvent::ValidationStarted,
        StagedChangeEvent::ValidationReady,
        StagedChangeEvent::SubmitQueued,
        StagedChangeEvent::IntentCreated {
            intent_id: "intent-1".to_string(),
        },
        StagedChangeEvent::LiveIntentClaimed {
            intent_id: "intent-1".to_string(),
        },
    ] {
        state.reduce(Action::ApplyStagedChangeEvent {
            id: "order-1".to_string(),
            event,
        });
    }

    state.reduce(Action::CloseStagedChange("order-1".to_string()));
    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::LiveIntentClaimed);

    state.reduce(Action::ApplyStagedChangeEvent {
        id: "order-1".to_string(),
        event: StagedChangeEvent::LiveSubmitSucceeded {
            intent_id: "intent-1".to_string(),
        },
    });
    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::LiveSubmitted);
}

#[test]
fn reducer_keeps_submit_queued_staged_change_until_worker_progress() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Buy BTCUSDT",
    )));
    for event in [
        StagedChangeEvent::ValidationStarted,
        StagedChangeEvent::ValidationReady,
        StagedChangeEvent::SubmitQueued,
    ] {
        state.reduce(Action::ApplyStagedChangeEvent {
            id: "order-1".to_string(),
            event,
        });
    }

    state.reduce(Action::CloseStagedChange("order-1".to_string()));
    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::SubmitQueued);

    state.reduce(Action::ApplyStagedChangeEvent {
        id: "order-1".to_string(),
        event: StagedChangeEvent::FailedBeforeIntent,
    });
    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::FailedBeforeIntent);
}

#[test]
fn reducer_does_not_replace_active_staged_change_with_new_submit_mode() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Dry run order",
    )));
    state.reduce(Action::ApplyStagedChangeEvent {
        id: "order-1".to_string(),
        event: StagedChangeEvent::ValidationStarted,
    });
    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Live order",
    )));

    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.mode, Some(SubmitMode::DryRun));
    assert_eq!(view.stage, StagedChangeStage::Validating);
    assert_eq!(view.summary, "Dry run order");
}

#[test]
fn live_write_command_requires_confirmation_before_enabling() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::ToggleLiveWrites));

    assert!(!state.live_writes_enabled);
    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::LiveWritesConfirmation)
    );

    state.reduce(Action::SetLiveWritesEnabled(true));
    assert!(state.live_writes_enabled);
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::LiveWritesConfirmation)
    );

    state.reduce(Action::Execute(ActionId::ToggleLiveWrites));
    assert!(!state.live_writes_enabled);
}

#[test]
fn disabling_live_writes_abandons_pending_live_changes() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
    state.reduce(Action::SetLiveWritesEnabled(true));
    state.reduce(Action::OpenStagedChange(StagedChangeRequest::text(
        "order-1",
        SubmitIntentKind::Order,
        "Pending live order",
    )));

    state.reduce(Action::SetLiveWritesEnabled(false));

    let view = state.staged_change_views().pop().unwrap();
    assert_eq!(view.stage, StagedChangeStage::Abandoned);
    assert_eq!(view.mode, Some(SubmitMode::DryRun));
    assert_eq!(state.effective_submit_mode(), SubmitMode::DryRun);
}

#[test]
fn reducer_ignores_stale_account_snapshots_after_new_profile_request() {
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
    state.trading_profile = Some("hedge".to_string());
    state.reduce(Action::AccountStarted {
        generation: 2,
        profile: "hedge".to_string(),
    });
    state.reduce(Action::AccountLoaded {
        generation: 1,
        snapshot: account_snapshot("mainnet"),
    });

    assert!(state.account_loading());
    assert!(state.account_snapshot.is_none());

    state.reduce(Action::AccountLoaded {
        generation: 2,
        snapshot: account_snapshot("hedge"),
    });

    assert!(!state.account_loading());
    assert_eq!(state.account_snapshot.as_ref().unwrap().profile, "hedge");
}

#[test]
fn reducer_ignores_account_snapshot_for_previous_trading_profile() {
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

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);
    state.reduce(Action::AccountLoaded {
        generation: 1,
        snapshot: account_snapshot("mainnet"),
    });

    assert!(!state.account_loading());
    assert!(state.account_snapshot.is_none());
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message.contains("ignored stale account generation"))
    );
}

#[test]
fn reducer_keeps_profile_validation_for_current_profile_only() {
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
    state.trading_profile = Some("hedge".to_string());
    state.reduce(Action::ProfileValidationStarted {
        generation: 2,
        profile: "hedge".to_string(),
    });
    state.reduce(Action::ProfileValidationLoaded {
        generation: 1,
        snapshot: profile_validation_snapshot("mainnet", true),
    });

    assert!(state.profile_validation_loading());
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Loading { .. }
    ));

    state.reduce(Action::ProfileValidationLoaded {
        generation: 2,
        snapshot: profile_validation_snapshot("hedge", true),
    });

    assert!(!state.profile_validation_loading());
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Ready { profile, .. } if profile == "hedge"
    ));
    assert!(state.has_current_profile_validation());
}

#[test]
fn reducer_rejects_profile_validation_for_previous_trading_profile() {
    let mut state = AppState::from_config(TuiConfig {
        trading: crate::config::TradingConfig {
            default_profile: Some("hedge".to_string()),
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::ProfileValidationStarted {
        generation: 1,
        profile: "mainnet".to_string(),
    });
    state.reduce(Action::ProfileValidationLoaded {
        generation: 1,
        snapshot: profile_validation_snapshot("mainnet", true),
    });

    assert!(!state.profile_validation_loading());
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Idle
    ));
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message.contains("ignored stale profile validation"))
    );
}

#[test]
fn reducer_keeps_profile_validation_failure_as_current_terminal_state() {
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

    assert!(!state.profile_validation_loading());
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Failed { profile, error }
            if profile == "missing" && error == "profile not found"
    ));
    assert!(state.has_current_profile_validation());
}

#[test]
fn scheduler_failure_terminates_active_profile_validation() {
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
    state.reduce(Action::SchedulerFailed(
        "scheduler worker stopped".to_string(),
    ));

    assert!(!state.profile_validation_loading());
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Failed { profile, error }
            if profile == "mainnet" && error.contains("scheduler failed")
    ));
}

#[test]
fn trading_profile_change_invalidates_profile_validation_snapshot() {
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
        snapshot: profile_validation_snapshot("mainnet", true),
    });
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Ready { .. }
    ));

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);

    assert_eq!(state.trading_profile.as_deref(), Some("hedge"));
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Idle
    ));
    assert!(!state.has_current_profile_validation());
}

#[test]
fn trading_profile_change_invalidates_profile_validation_failure() {
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
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Failed { .. }
    ));

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."missing".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);

    assert_eq!(state.trading_profile.as_deref(), Some("hedge"));
    assert!(matches!(
        &state.profile_validation,
        ProfileValidationState::Idle
    ));
    assert!(!state.has_current_profile_validation());
}

#[test]
fn reducer_marks_same_generation_profile_mismatch_as_terminal_warning() {
    let mut state = AppState::from_config(TuiConfig {
        trading: crate::config::TradingConfig {
            default_profile: Some("hedge".to_string()),
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

    assert!(!state.account_loading());
    assert!(state.account_snapshot.is_none());
    let account_entries = state
        .task_log
        .iter()
        .filter(|entry| {
            matches!(
                entry.key,
                Some(crate::task_log::TaskKey::Account { generation: 1, .. })
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(account_entries.len(), 1);
    assert_eq!(account_entries[0].status, TaskStatus::Warning);
    assert!(
        account_entries[0]
            .message
            .contains("ignored stale account snapshot")
    );
}

#[test]
fn trading_profile_change_cancels_active_account_load_and_ignores_old_failure() {
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

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);
    state.reduce(Action::AccountFailed {
        generation: 1,
        profile: "mainnet".to_string(),
        error: "old profile failed".to_string(),
    });

    assert_eq!(state.trading_profile.as_deref(), Some("hedge"));
    assert!(!state.account_loading());
    assert!(!state.task_failures.has_source(TaskFailureSource::Account));
}

#[test]
fn floating_panes_use_vec_order_as_top_order() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));

    assert_eq!(state.floating[0].kind, FloatingKind::Help);
    assert_eq!(state.floating[1].kind, FloatingKind::CommandPalette);

    state.reduce(Action::CloseFocusedFloating);

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn floating_panes_can_be_focused_and_resized() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::ProviderDetails,
    )));
    state.reduce(Action::FocusFloating(FloatingKind::Help));

    assert_eq!(state.floating.last().unwrap().kind, FloatingKind::Help);

    let size = FloatingSize::resized(82, 63);
    state.reduce(Action::ResizeFloating {
        kind: FloatingKind::Help,
        size,
    });

    let help = state
        .floating
        .iter()
        .find(|pane| pane.kind == FloatingKind::Help)
        .unwrap();
    assert_eq!(help.size, size);
}

#[test]
fn interaction_mode_follows_top_floating_pane() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.interaction_mode(), InteractionMode::Normal);

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    assert_eq!(state.interaction_mode(), InteractionMode::Help);

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    assert_eq!(state.interaction_mode(), InteractionMode::Command);

    state.reduce(Action::CloseFocusedFloating);
    assert_eq!(state.interaction_mode(), InteractionMode::Help);
}

#[test]
fn workspace_switching_keeps_focus_visible() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::History));

    assert_eq!(state.panels.focused(), Panel::History);

    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.visible_panels().contains(&state.panels.focused()));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
}

#[test]
fn pane_focus_navigation_wraps_visible_workspace_panels() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Research,
        },
        ..TuiConfig::default()
    });

    assert_eq!(state.panels.focused(), Panel::Watchlist);
    state.reduce(Action::FocusPanelBy(1));
    assert_eq!(state.panels.focused(), Panel::Quote);
    state.reduce(Action::FocusPanelBy(-1));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    state.reduce(Action::FocusPanelBy(-1));
    assert_eq!(state.panels.focused(), Panel::TaskLog);
}

#[test]
fn pane_focus_navigation_uses_workspace_declared_order() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Market,
        },
        ..TuiConfig::default()
    });

    assert_eq!(
        state.workspace_panels(),
        vec![
            Panel::Watchlist,
            Panel::Quote,
            Panel::History,
            Panel::ProviderHealth,
            Panel::TaskLog,
        ]
    );
    state.reduce(Action::FocusPanelBy(1));
    assert_eq!(state.panels.focused(), Panel::Quote);
}

#[test]
fn pane_zoom_limits_visible_panels_without_trapping_focus_navigation() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Focus(Panel::History));
    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    assert_eq!(state.visible_panels(), vec![Panel::History]);

    state.reduce(Action::FocusPanelBy(1));
    assert!(state.zoomed);
    assert_eq!(state.panels.focused(), Panel::ProviderHealth);
    assert_eq!(state.visible_panels(), vec![Panel::ProviderHealth]);

    state.reduce(Action::ToggleFocusedZoom);
    assert!(!state.zoomed);
    assert!(state.visible_panels().len() > 1);
}

#[test]
fn zoom_does_not_turn_hidden_open_panels_into_focus_actions() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Focus(Panel::History));
    state.reduce(Action::ToggleFocusedZoom);
    assert_eq!(state.visible_panels(), vec![Panel::History]);
    assert!(state.is_open_in_workspace(Panel::Quote));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Quote)));

    assert!(!state.panels.contains(Panel::Quote));
    assert!(!state.zoomed);
    assert_eq!(state.panels.focused(), Panel::History);
}

#[test]
fn workspace_and_layout_restore_leave_zoom_mode() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));
    assert!(!state.zoomed);

    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    state.reduce(Action::RestorePanels);
    assert!(!state.zoomed);
}

#[test]
fn inconsistent_persisted_workspace_config_is_normalized_on_load() {
    let state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Research,
        },
        panels: PanelConfig {
            open: vec![Panel::History],
            focused: Panel::History,
        },
        ..TuiConfig::default()
    });

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.panels.contains(Panel::History));
    assert!(state.panels.contains(Panel::Watchlist));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    assert_eq!(state.visible_panels(), vec![Panel::Watchlist]);
}

#[test]
fn closing_every_visible_workspace_panel_reopens_workspace_default() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));

    for panel in WorkspaceKind::Research.panels() {
        state.reduce(Action::Focus(*panel));
        state.reduce(Action::CloseFocusedPanel);
    }

    assert!(!state.visible_panels().is_empty());
    assert_eq!(
        state.panels.focused(),
        WorkspaceKind::Research.default_panel()
    );
    assert!(
        state
            .visible_panels()
            .contains(&WorkspaceKind::Research.default_panel())
    );
}

#[test]
fn focusing_hidden_panel_moves_to_a_workspace_that_can_show_it() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.workspace, WorkspaceKind::Market);

    state.reduce(Action::Focus(Panel::Polymarket));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert_eq!(state.panels.focused(), Panel::Polymarket);
    assert!(state.visible_panels().contains(&Panel::Polymarket));
}

#[test]
fn entering_settings_workspace_focuses_its_primary_panel() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.panels.focused(), Panel::Watchlist);

    state.reduce(Action::SetWorkspace(WorkspaceKind::Settings));

    assert_eq!(state.workspace, WorkspaceKind::Settings);
    assert_eq!(state.panels.focused(), Panel::Settings);
    assert!(state.visible_panels().contains(&Panel::Settings));
}

#[test]
fn settings_workspace_initialization_opens_its_primary_panel_for_old_custom_layouts() {
    let state = AppState::from_config(TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        panels: crate::config::PanelConfig {
            open: vec![Panel::Watchlist, Panel::ProviderHealth, Panel::TaskLog],
            focused: Panel::Watchlist,
        },
        ..TuiConfig::default()
    });

    assert_eq!(state.workspace, WorkspaceKind::Settings);
    assert_eq!(state.panels.focused(), Panel::Settings);
    assert!(state.panels.contains(Panel::Settings));
    assert!(state.visible_panels().contains(&Panel::Settings));
}

#[test]
fn command_palette_show_panel_routes_to_visible_workspace() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));
    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Polymarket)));
    assert!(!state.panels.contains(Panel::Polymarket));

    state.reduce(Action::SetWorkspace(WorkspaceKind::Market));
    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Polymarket)));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert_eq!(state.panels.focused(), Panel::Polymarket);
    assert!(state.visible_panels().contains(&Panel::Polymarket));
}

#[test]
fn command_palette_toggle_hidden_open_panel_routes_to_visible_workspace() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.workspace, WorkspaceKind::Market);
    assert!(state.panels.contains(Panel::Research));
    assert!(!state.visible_panels().contains(&Panel::Research));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Research)));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.panels.contains(Panel::Research));
    assert_eq!(state.panels.focused(), Panel::Research);
    assert!(state.visible_panels().contains(&Panel::Research));
}

#[test]
fn command_palette_executes_workspace_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::SetWorkspace(
        WorkspaceKind::Account,
    )));

    assert_eq!(state.workspace, WorkspaceKind::Account);
    assert!(state.floating.is_empty());
    assert!(state.visible_panels().contains(&state.panels.focused()));
    assert!(state.visible_panels().contains(&Panel::Account));
}

#[test]
fn command_palette_wraps_selection_and_executes_overlay_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::MoveCommandSelection(-1));
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::CloseCommandPalette)
    );

    for character in "open help".chars() {
        state.reduce(Action::EditCommandQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::OpenFloating(FloatingKind::Help))
    );

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn command_palette_query_filters_executable_actions() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    for character in "market".chars() {
        state.reduce(Action::EditCommandQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }

    assert_eq!(state.command_palette.query(), "market");
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::SetWorkspace(WorkspaceKind::Market))
    );
}

#[test]
fn command_palette_query_resets_after_close() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::EditCommandQuery(
        tui_input::InputRequest::InsertChar('z'),
    ));
    state.reduce(Action::CloseFocusedFloating);

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));

    assert_eq!(state.command_palette.query(), "");
    assert_eq!(
        state.command_palette.len(),
        crate::command::ACTION_REGISTRY
            .iter()
            .filter(|action| action.command().is_some())
            .count()
    );
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::SelectSymbolBy(1))
    );
}

#[test]
fn symbol_search_selects_watchlist_symbols_and_resets_on_close() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec![
            "AAPL".to_string(),
            "CRDO".to_string(),
            "BTCUSDT".to_string(),
        ],
        ..TuiConfig::default()
    });
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::SymbolSearch,
    )));

    for character in "btc".chars() {
        state.reduce(Action::EditSymbolSearchQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptSymbolSearch);

    assert_eq!(state.selected_symbol(), Some("BTCUSDT"));
    assert_eq!(state.interaction_mode(), InteractionMode::Normal);
    assert_eq!(state.symbol_search.query(), "");

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::SymbolSearch,
    )));
    state.reduce(Action::EditSymbolSearchQuery(
        tui_input::InputRequest::InsertChar('c'),
    ));
    state.reduce(Action::CloseFocusedFloating);

    assert_eq!(state.symbol_search.query(), "");
}

#[test]
fn command_palette_executes_panel_focus_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Research)));

    assert_eq!(state.panels.focused(), Panel::Research);
    assert!(state.floating.is_empty());
}

#[test]
fn command_palette_close_preserves_underlying_overlay() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::CloseCommandPalette));

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn panel_lifecycle_closes_focused_panel_and_restores_all_panels() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::Research));

    state.reduce(Action::CloseFocusedPanel);

    assert!(!state.panels.contains(Panel::Research));
    assert_ne!(state.panels.focused(), Panel::Research);
    assert!(state.panels.contains(state.panels.focused()));

    state.reduce(Action::RestorePanels);

    assert!(
        Panel::ALL
            .into_iter()
            .all(|panel| state.panels.contains(panel))
    );
    assert_eq!(state.panels.open_panels().len(), Panel::ALL.len());
}

#[test]
fn panel_lifecycle_toggles_panels_without_closing_the_last_one() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert!(!state.panels.contains(Panel::History));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert!(state.panels.contains(Panel::History));
    assert_eq!(state.panels.focused(), Panel::History);

    for panel in [
        Panel::Watchlist,
        Panel::Quote,
        Panel::ProviderHealth,
        Panel::TaskLog,
    ] {
        state.reduce(Action::Execute(toggle_panel_action(panel)));
    }
    assert_eq!(state.visible_panels(), vec![Panel::History]);

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert_eq!(state.visible_panels(), vec![Panel::Watchlist]);
}

#[test]
fn state_exports_user_layout_preferences_to_config() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::Research));
    state.reduce(Action::CloseFocusedPanel);
    state.reduce(Action::ResizeDockedColumns {
        left_ratio: 31,
        main_ratio: 42,
    });
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::ResizeFloating {
        kind: FloatingKind::Help,
        size: FloatingSize::resized(82, 63),
    });

    let config = state.export_config(&TuiConfig::default());

    assert_eq!(config.layout.left_ratio, 31);
    assert_eq!(config.layout.main_ratio, 42);
    assert!(!config.panels.open.contains(&Panel::Research));
    assert!(config.panels.open.contains(&Panel::Watchlist));
    assert_ne!(config.panels.focused, Panel::Research);
    assert_eq!(config.floating.panes.len(), 1);
    assert_eq!(config.floating.panes[0].kind, FloatingKind::Help);
    assert_eq!(config.floating.panes[0].size, FloatingSize::resized(82, 63));
    assert_eq!(state.config_changes, ["layout"]);
}

#[test]
fn config_changes_track_layout_without_treating_navigation_as_config() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Focus(Panel::Quote));
    state.reduce(Action::ShiftWorkspace(1));
    state.reduce(Action::FocusPanelBy(1));
    state.reduce(Action::ToggleFocusedZoom);
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::ResizeFloating {
        kind: FloatingKind::CommandPalette,
        size: FloatingSize::resized(80, 60),
    });

    assert!(state.config_changes.is_empty());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    assert_eq!(state.config_changes, ["layout"]);
}

#[test]
fn config_changes_track_only_persistent_floating_layout_changes() {
    let mut temporary = AppState::from_config(TuiConfig::default());
    temporary.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    temporary.reduce(Action::CloseFocusedFloating);
    assert!(temporary.config_changes.is_empty());

    let mut persistent = AppState::from_config(TuiConfig {
        floating: FloatingConfig {
            panes: vec![FloatingPane::new(FloatingKind::Help)],
        },
        ..TuiConfig::default()
    });
    persistent.reduce(Action::CloseFocusedFloating);
    assert_eq!(persistent.config_changes, ["layout"]);

    let mut idempotent = AppState::from_config(TuiConfig {
        floating: FloatingConfig {
            panes: vec![FloatingPane::new(FloatingKind::Help)],
        },
        ..TuiConfig::default()
    });
    idempotent.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    assert!(idempotent.config_changes.is_empty());

    let mut focus_only = AppState::from_config(TuiConfig {
        floating: FloatingConfig {
            panes: vec![
                FloatingPane::new(FloatingKind::Help),
                FloatingPane::new(FloatingKind::ProviderDetails),
            ],
        },
        ..TuiConfig::default()
    });
    focus_only.reduce(Action::FocusFloating(FloatingKind::Help));
    assert!(focus_only.config_changes.is_empty());
}

#[test]
fn layout_undo_restores_persistent_floatings_without_rewinding_navigation() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::SetWorkspace(
        WorkspaceKind::Trade,
    )));
    state.reduce(Action::Execute(ActionId::FocusPanel(Panel::OpenOrders)));
    state.reduce(Action::ToggleFocusedZoom);

    assert_eq!(state.config_changes, ["layout"]);
    assert!(
        state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::Help)
    );

    state.reduce(Action::UndoConfigChange);

    assert!(state.config_changes.is_empty());
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::Help)
    );
    assert_eq!(state.workspace, WorkspaceKind::Trade);
    assert_eq!(state.panels.focused(), Panel::OpenOrders);
    assert!(state.zoomed);
}

#[test]
fn config_save_request_lifecycle_clears_changes_only_after_success() {
    let mut clean = AppState::from_config(TuiConfig::default());
    clean.reduce(Action::RequestConfigSave);
    assert!(!clean.take_pending_config_save());
    assert!(clean.config_changes.is_empty());

    let mut dirty = AppState::from_config(TuiConfig::default());
    dirty.reduce(Action::ResizeDockedColumns {
        left_ratio: 31,
        main_ratio: 42,
    });
    dirty.reduce(Action::RequestConfigSave);
    assert!(dirty.take_pending_config_save());
    assert_eq!(dirty.config_changes, ["layout"]);

    dirty.reduce(Action::ConfigSaveFailed("disk full".to_string()));
    assert_eq!(dirty.config_changes, ["layout"]);
    assert!(!dirty.take_pending_config_save());

    dirty.reduce(Action::RequestConfigSave);
    assert!(dirty.take_pending_config_save());
    dirty.reduce(Action::ConfigSaved);
    assert!(dirty.config_changes.is_empty());
    assert!(!dirty.take_pending_config_save());
}

#[test]
fn command_palette_save_config_routes_to_pending_save_request() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::ResizeDockedColumns {
        left_ratio: 31,
        main_ratio: 42,
    });

    state.reduce(Action::Execute(ActionId::SaveConfig));

    assert!(state.take_pending_config_save());
    assert_eq!(state.config_changes, ["layout"]);
}

#[test]
fn trading_profile_editor_updates_exported_config_and_can_clear_profile() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for character in " mainnet ".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }

    state.reduce(Action::AcceptTradingProfile);

    assert_eq!(state.trading_profile.as_deref(), Some("mainnet"));
    assert_eq!(state.config_changes, ["trading"]);
    let config = state.export_config(&TuiConfig::default());
    assert_eq!(config.trading.default_profile.as_deref(), Some("mainnet"));

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    state.reduce(Action::AcceptTradingProfile);

    assert_eq!(state.trading_profile, None);
    let config = state.export_config(&TuiConfig::default());
    assert_eq!(config.trading.default_profile, None);
}

#[test]
fn trading_profile_undo_restores_profile_and_rejects_cancelled_account_loads() {
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

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::TradingProfile,
    )));
    for _ in 0.."mainnet".len() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::DeletePrevChar,
        ));
    }
    for character in "hedge".chars() {
        state.reduce(Action::EditTradingProfileQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptTradingProfile);
    state.reduce(Action::AccountStarted {
        generation: 2,
        profile: "hedge".to_string(),
    });

    assert_eq!(state.trading_profile.as_deref(), Some("hedge"));
    assert!(state.account_loading());

    state.reduce(Action::UndoConfigChange);
    state.reduce(Action::AccountLoaded {
        generation: 2,
        snapshot: account_snapshot("hedge"),
    });

    assert_eq!(state.trading_profile.as_deref(), Some("mainnet"));
    assert!(state.config_changes.is_empty());
    assert!(!state.trading_profile_edited);
    assert!(!state.account_loading());
    assert!(state.account_snapshot.is_none());

    state.reduce(Action::AccountStarted {
        generation: 3,
        profile: "mainnet".to_string(),
    });
    state.reduce(Action::AccountLoaded {
        generation: 3,
        snapshot: account_snapshot("mainnet"),
    });

    assert_eq!(state.account_snapshot.as_ref().unwrap().profile, "mainnet");
}

#[test]
fn trading_profile_revalidation_clears_terminal_validation_without_persisted_config_change() {
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
            PathBuf::from("/tmp/mainnet.toml"),
        ),
    });
    assert!(state.has_current_profile_validation());

    state.reduce(Action::Execute(ActionId::RevalidateTradingProfile));

    assert!(!state.has_current_profile_validation());
    assert!(matches!(
        state.profile_validation,
        ProfileValidationState::Idle
    ));
    assert!(state.config_changes.is_empty());
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message == "mainnet profile validation queued")
    );
}

#[test]
fn trading_profile_revalidation_requires_selected_profile() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::RevalidateTradingProfile));

    assert!(matches!(
        state.profile_validation,
        ProfileValidationState::Idle
    ));
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message == "no trading profile selected for validation")
    );
}

#[test]
fn profile_live_toggle_stages_validated_profile_risk_review_for_local_commit_confirmation() {
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
            PathBuf::from("/tmp/mainnet.toml"),
        ),
    });

    state.reduce(Action::Execute(ActionId::StageProfileLiveToggle));

    assert_eq!(state.panels.focused(), Panel::IntentReview);
    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_kind, StagedChangeKind::ProfileRisk);
    assert_eq!(changes[0].intent_kind, None);
    assert_eq!(changes[0].stage, StagedChangeStage::Ready);
    let StagedChangeSubject::ProfileRisk(review) = &changes[0].subject else {
        panic!("expected profile risk review");
    };
    assert_eq!(review.profile, "mainnet");
    assert_eq!(
        review.change,
        ProfileRiskChange::AllowLive {
            before: true,
            after: false
        }
    );
    assert_eq!(review.diff, vec!["risk.allow_live: true -> false"]);
    assert_eq!(review.required_failure_count, 0);

    state.reduce(Action::ExecuteStagedChange);

    assert!(state.take_pending_staged_execution().is_none());
    assert!(matches!(
        state
            .pending_staged_confirmation()
            .map(|request| &request.execution),
        Some(StagedExecution::LocalCommit { .. })
    ));
    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
}

#[test]
fn profile_risk_shortcut_stages_profile_risk_review() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..TuiConfig::default()
    });
    state.reduce(Action::Execute(ActionId::FocusPanel(Panel::ProfileRisk)));
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
    let action = crate::input::key_action(
        &state,
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('t')),
    )
    .expect("settings risk shortcut action");

    state.reduce(action);

    assert_eq!(state.panels.focused(), Panel::IntentReview);
    let changes = state.staged_change_views();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].change_kind, StagedChangeKind::ProfileRisk);
}

#[test]
fn profile_live_toggle_requires_current_profile_validation() {
    let mut state = AppState::from_config(TuiConfig {
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::Execute(ActionId::StageProfileLiveToggle));

    assert!(state.staged_change_views().is_empty());
    assert!(state.task_log.iter().any(|entry| {
        entry
            .message
            .contains("profile must be validated before staging a risk change")
    }));
}

#[test]
fn settings_provider_preferences_edit_export_and_request_runtime_update() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::AdjustSelectedSetting(1));

    assert_eq!(state.providers.equity, crate::config::EquityProvider::Yahoo);
    assert_eq!(state.config_changes, ["providers"]);
    let pending = state
        .take_pending_provider_preferences_update()
        .expect("provider preference update");
    assert_eq!(pending.equity, crate::config::EquityProvider::Yahoo);
    assert!(state.take_pending_provider_preferences_update().is_none());

    state.reduce(Action::MoveSettingsSelection(1));
    state.reduce(Action::AdjustSelectedSetting(1));

    assert_eq!(state.providers.crypto, CryptoProvider::Binance);
    let pending = state
        .take_pending_provider_preferences_update()
        .expect("crypto provider preference update");
    assert_eq!(pending.crypto, CryptoProvider::Binance);
    let config = state.export_config(&TuiConfig::default());
    assert_eq!(
        config.providers.equity,
        crate::config::EquityProvider::Yahoo
    );
    assert_eq!(config.providers.crypto, CryptoProvider::Binance);
}

#[test]
fn settings_theme_edit_exports_without_provider_runtime_update() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::MoveSettingsSelection(2));
    state.reduce(Action::AdjustSelectedSetting(1));

    assert_eq!(state.theme.accent, ThemeColor::Gray);
    assert_eq!(state.config_changes, ["theme"]);
    assert!(state.take_pending_provider_preferences_update().is_none());
    let config = state.export_config(&TuiConfig::default());
    assert_eq!(config.theme.accent, ThemeColor::Gray);
}

#[test]
fn settings_provider_edit_can_be_undone_and_reloads_runtime_preferences() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });
    let initial_equity = state.providers.equity;

    state.reduce(Action::AdjustSelectedSetting(1));
    assert_eq!(state.providers.equity, crate::config::EquityProvider::Yahoo);
    assert_eq!(state.config_changes, ["providers"]);
    assert!(state.config_undo_available());
    assert!(state.take_pending_provider_preferences_update().is_some());

    state.reduce(Action::UndoConfigChange);

    assert_eq!(state.providers.equity, initial_equity);
    assert!(state.config_changes.is_empty());
    assert!(!state.config_undo_available());
    let pending = state
        .take_pending_provider_preferences_update()
        .expect("undo should refresh runtime providers");
    assert_eq!(pending.equity, initial_equity);
}

#[test]
fn settings_theme_edit_undo_restores_clean_config_without_provider_reload() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });
    let initial_accent = state.theme.accent;

    state.reduce(Action::MoveSettingsSelection(2));
    state.reduce(Action::AdjustSelectedSetting(1));
    assert_eq!(state.theme.accent, ThemeColor::Gray);

    state.reduce(Action::UndoConfigChange);

    assert_eq!(state.theme.accent, initial_accent);
    assert!(state.config_changes.is_empty());
    assert!(state.take_pending_provider_preferences_update().is_none());
}

#[test]
fn settings_keymap_edit_exports_runtime_binding_and_can_be_undone() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });

    move_to_setting(&mut state, "key command palette");
    state.reduce(Action::AdjustSelectedSetting(1));

    assert_eq!(state.config_changes, ["keymap"]);
    assert_eq!(
        state.keymap.normal_action(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::CONTROL,
        )),
        Some(ActionId::OpenFloating(FloatingKind::CommandPalette))
    );
    assert_eq!(state.keymap.overrides.len(), 1);
    assert!(state.take_pending_provider_preferences_update().is_none());
    let config = state.export_config(&TuiConfig::default());
    assert_eq!(config.keymap.overrides.len(), 1);

    state.reduce(Action::AdjustSelectedSetting(-1));
    let config = state.export_config(&TuiConfig::default());
    assert!(config.keymap.overrides.is_empty());
    assert_eq!(
        state.keymap.normal_action(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(':'),
            crossterm::event::KeyModifiers::SHIFT,
        )),
        Some(ActionId::OpenFloating(FloatingKind::CommandPalette))
    );

    state.reduce(Action::UndoConfigChange);

    assert_eq!(state.config_changes, ["keymap"]);
    assert_eq!(
        state.keymap.normal_action(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::CONTROL,
        )),
        Some(ActionId::OpenFloating(FloatingKind::CommandPalette))
    );

    state.reduce(Action::UndoConfigChange);

    assert!(state.config_changes.is_empty());
    assert!(state.keymap.overrides.is_empty());
}

#[test]
fn watchlist_edits_normalize_reorder_delete_and_export_config() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
        ..TuiConfig::default()
    });
    for character in " lite,crdo ".chars() {
        state.reduce(Action::EditWatchlistAddQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }

    state.reduce(Action::AcceptWatchlistAdd);
    assert_eq!(state.watchlist, ["AAPL", "CRDO", "LITE"]);
    assert_eq!(state.selected_symbol(), Some("LITE"));
    assert_eq!(state.config_changes, ["watchlist"]);

    state.reduce(Action::MoveSelectedWatchlistSymbol(-1));
    assert_eq!(state.watchlist, ["AAPL", "LITE", "CRDO"]);
    assert_eq!(state.selected_symbol(), Some("LITE"));

    state.reduce(Action::DeleteSelectedWatchlistSymbol);
    assert_eq!(state.watchlist, ["AAPL", "CRDO"]);
    assert_eq!(state.selected_symbol(), Some("CRDO"));

    let config = state.export_config(&TuiConfig::default());
    assert_eq!(config.watchlist, ["AAPL", "CRDO"]);
}

#[test]
fn watchlist_config_edits_can_be_undone_in_reverse_order() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
        ..TuiConfig::default()
    });
    for character in "lite".chars() {
        state.reduce(Action::EditWatchlistAddQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptWatchlistAdd);
    state.reduce(Action::MoveSelectedWatchlistSymbol(-1));
    state.reduce(Action::DeleteSelectedWatchlistSymbol);

    assert_eq!(state.watchlist, ["AAPL", "CRDO"]);
    assert_eq!(state.selected_symbol(), Some("CRDO"));
    assert!(state.config_undo_available());

    state.reduce(Action::UndoConfigChange);
    assert_eq!(state.watchlist, ["AAPL", "LITE", "CRDO"]);
    assert_eq!(state.selected_symbol(), Some("CRDO"));

    state.reduce(Action::UndoConfigChange);
    assert_eq!(state.watchlist, ["AAPL", "CRDO", "LITE"]);
    assert_eq!(state.selected_symbol(), Some("CRDO"));

    state.reduce(Action::UndoConfigChange);
    assert_eq!(state.watchlist, ["AAPL", "CRDO"]);
    assert_eq!(state.selected_symbol(), Some("CRDO"));
    assert!(state.config_changes.is_empty());
    assert!(!state.config_undo_available());
    assert_eq!(
        state.export_config(&TuiConfig::default()).watchlist,
        ["AAPL", "CRDO"]
    );
}

#[test]
fn config_undo_does_not_restore_later_runtime_navigation() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string(), "LITE".to_string()],
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..TuiConfig::default()
    });

    state.reduce(Action::MoveSettingsSelection(2));
    state.reduce(Action::AdjustSelectedSetting(1));
    state.reduce(Action::Execute(ActionId::SetWorkspace(
        WorkspaceKind::Market,
    )));
    state.reduce(Action::Execute(ActionId::SelectSymbolBy(2)));
    state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Quote)));
    state.reduce(Action::ToggleFocusedZoom);

    assert_eq!(state.workspace, WorkspaceKind::Market);
    assert_eq!(state.selected_symbol(), Some("LITE"));
    assert_eq!(state.panels.focused(), Panel::Quote);
    assert!(state.zoomed);

    state.reduce(Action::UndoConfigChange);

    assert_eq!(state.workspace, WorkspaceKind::Market);
    assert_eq!(state.selected_symbol(), Some("LITE"));
    assert_eq!(state.panels.focused(), Panel::Quote);
    assert!(state.zoomed);
    assert!(state.config_changes.is_empty());
}

#[test]
fn watchlist_delete_keeps_one_symbol() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string()],
        ..TuiConfig::default()
    });

    state.reduce(Action::DeleteSelectedWatchlistSymbol);

    assert_eq!(state.watchlist, ["AAPL"]);
    assert!(state.config_changes.is_empty());
    assert!(
        state
            .task_log
            .iter()
            .any(|entry| entry.message.contains("at least one symbol"))
    );
}

#[test]
fn watchlist_reorder_stops_at_edges() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
        ..TuiConfig::default()
    });

    state.reduce(Action::MoveSelectedWatchlistSymbol(-1));
    assert_eq!(state.watchlist, ["AAPL", "CRDO"]);
    assert!(state.config_changes.is_empty());

    state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
    state.reduce(Action::MoveSelectedWatchlistSymbol(1));
    assert_eq!(state.watchlist, ["AAPL", "CRDO"]);
    assert!(state.config_changes.is_empty());
}

#[test]
fn reducer_resizes_and_resets_docked_layout() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ResizeDockedColumns {
        left_ratio: 8,
        main_ratio: 80,
    });
    assert_eq!(state.layout.left_ratio, 15);
    assert_eq!(state.layout.main_ratio, 60);
    assert!(state.layout.left_ratio + state.layout.main_ratio <= MAX_LEFT_MAIN_RATIO);

    state.reduce(Action::ResetLayout);
    assert_eq!(state.layout, LayoutConfig::default());
}

#[test]
fn reducer_accepts_current_snapshot_and_ignores_stale_snapshot() {
    let mut state = AppState::from_config(TuiConfig::default());
    let current = snapshot(2, "CRDO");
    let stale = snapshot(1, "AAPL");

    state.reduce(Action::RefreshStarted(2));
    state.reduce(Action::SnapshotLoaded {
        generation: 1,
        snapshot: stale,
    });
    assert!(state.market_snapshot.is_none());
    assert!(state.refresh_loading());

    state.reduce(Action::SnapshotLoaded {
        generation: 2,
        snapshot: current,
    });
    assert_eq!(
        state
            .market_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.quote_for("CRDO"))
            .and_then(|quote| quote.price),
        Some(250.0)
    );
    assert!(!state.refresh_loading());
}

#[test]
fn reducer_clears_in_flight_refresh_on_scheduler_fatal_failure() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::RefreshStarted(1));
    state.reduce(Action::HistoryStarted {
        generation: 1,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::EvidenceStarted {
        generation: 1,
        symbol: "BTCUSDT".to_string(),
    });
    state.reduce(Action::ResearchStarted {
        generation: 1,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::SchedulerFailed(
        "scheduler runtime failed".to_string(),
    ));

    assert!(!state.refresh_loading());
    assert!(!state.history.loading());
    assert!(!state.evidence.loading());
    assert!(!state.research.loading());
    assert_eq!(
        state.scheduler_error.as_deref(),
        Some("scheduler runtime failed")
    );

    state.reduce(Action::SnapshotLoaded {
        generation: 1,
        snapshot: snapshot(1, "CRDO"),
    });
    state.reduce(Action::HistoryLoaded {
        generation: 1,
        snapshot: history_snapshot("CRDO", 250.0),
    });
    state.reduce(Action::EvidenceLoaded {
        generation: 1,
        snapshot: evidence_snapshot("BTCUSDT", 2, 3),
    });
    state.reduce(Action::ResearchLoaded {
        generation: 1,
        snapshot: research_snapshot("CRDO", 1, 1),
    });

    assert!(state.market_snapshot.is_none());
    assert!(state.history.selected_snapshot("CRDO").is_none());
    assert!(state.evidence.selected_snapshot("BTCUSDT").is_none());
    assert!(state.research.selected_snapshot("CRDO").is_none());
    assert!(state.task_log.iter().any(|entry| {
        entry.status == TaskStatus::Failed
            && entry.message == "CRDO history loading cancelled: scheduler runtime failed"
    }));
}

#[test]
fn reducer_accepts_current_history_and_ignores_stale_history() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::HistoryStarted {
        generation: 2,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::HistoryLoaded {
        generation: 1,
        snapshot: history_snapshot("AAPL", 100.0),
    });
    assert!(state.history.selected_snapshot("AAPL").is_none());
    assert!(state.history.loading());

    state.reduce(Action::HistoryLoaded {
        generation: 2,
        snapshot: history_snapshot("CRDO", 250.0),
    });
    assert_eq!(
        state
            .history
            .selected_snapshot("CRDO")
            .and_then(|snapshot| snapshot.latest_close),
        Some(250.0)
    );
    assert!(!state.history.loading());
}

#[test]
fn reducer_accepts_current_evidence_and_ignores_stale_evidence() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::EvidenceStarted {
        generation: 2,
        symbol: "BTCUSDT".to_string(),
    });
    state.reduce(Action::EvidenceLoaded {
        generation: 1,
        snapshot: evidence_snapshot("ETHUSDT", 1, 2),
    });
    assert!(state.evidence.selected_snapshot("ETHUSDT").is_none());
    assert!(state.evidence.loading());

    state.reduce(Action::EvidenceLoaded {
        generation: 2,
        snapshot: evidence_snapshot("BTCUSDT", 2, 3),
    });
    assert_eq!(
        state
            .evidence
            .selected_snapshot("BTCUSDT")
            .map(|snapshot| (snapshot.ok_providers, snapshot.total_providers)),
        Some((2, 3))
    );
    assert!(!state.evidence.loading());
}

#[test]
fn reducer_accepts_current_research_and_ignores_stale_research() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ResearchStarted {
        generation: 2,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::ResearchLoaded {
        generation: 1,
        snapshot: research_snapshot("AAPL", 1, 1),
    });
    assert!(state.research.selected_snapshot("AAPL").is_none());
    assert!(state.research.loading());

    state.reduce(Action::ResearchLoaded {
        generation: 2,
        snapshot: research_snapshot("CRDO", 2, 3),
    });
    assert_eq!(
        state
            .research
            .selected_snapshot("CRDO")
            .map(|snapshot| (snapshot.news.len(), snapshot.prediction_markets.len())),
        Some((2, 3))
    );
    assert!(!state.research.loading());
}

fn snapshot(_generation: u64, symbol: &str) -> MarketSnapshot {
    MarketSnapshot {
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        quotes: vec![QuoteSnapshot {
            symbol: symbol.to_string(),
            price: Some(250.0),
            currency: Some("USD".to_string()),
            provider: "test".to_string(),
            session: Some("regular".to_string()),
            market_time_local: None,
            change_pct: Some(1.0),
            aliases: Vec::new(),
            regular_basis: RegularBasisSnapshot {
                previous_close: Some(247.0),
                open: None,
                high: None,
                low: None,
                volume: None,
            },
        }],
        errors: Vec::new(),
    }
}

fn history_snapshot(symbol: &str, latest_close: f64) -> HistorySnapshot {
    HistorySnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        provider: "test".to_string(),
        interval: "1d".to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        latest_close: Some(latest_close),
        latest_time: Some("2026-06-25".to_string()),
        return_pct: Some(1.0),
        volume: Some(10_000.0),
        bars: Vec::new(),
        errors: Vec::new(),
    }
}

fn evidence_snapshot(
    symbol: &str,
    ok_providers: usize,
    total_providers: usize,
) -> CryptoQuoteEvidenceSnapshot {
    CryptoQuoteEvidenceSnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        instrument: "spot".to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        ok_providers,
        total_providers,
        providers: Vec::new(),
        errors: Vec::new(),
    }
}

fn research_snapshot(
    symbol: &str,
    news_count: usize,
    prediction_count: usize,
) -> ResearchContextSnapshot {
    ResearchContextSnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        news: (0..news_count)
            .map(
                |index| agent_finance_market::research_snapshot::ResearchNewsSnapshot {
                    title: format!("headline {index}"),
                    provider: "test".to_string(),
                    module: "news".to_string(),
                },
            )
            .collect(),
        prediction_markets: (0..prediction_count)
            .map(
                |index| agent_finance_market::research_snapshot::PredictionMarketSnapshot {
                    title: format!("market {index}"),
                    probability: Some(0.5),
                    volume: Some(1000.0),
                    liquidity: None,
                    market_url: None,
                },
            )
            .collect(),
        errors: Vec::new(),
    }
}

fn account_snapshot(profile: &str) -> crate::account::AccountSnapshot {
    crate::account::AccountSnapshot::new(
        profile.to_string(),
        Provider::Binance,
        Environment::Live,
        crate::profile_snapshot::test_trading_profile_snapshot(),
        account_reads(profile),
        Vec::new(),
    )
}

fn profile_validation_snapshot(profile: &str, passed: bool) -> ProfileValidationSnapshot {
    let mut profile_config = test_profile(profile);
    profile_config.permissions.spot_trading = passed;
    ProfileValidationSnapshot::from_profile(
        &profile_config,
        PathBuf::from(format!("{profile}.toml")),
    )
}

fn account_snapshot_with_open_orders(profile: &str) -> crate::account::AccountSnapshot {
    crate::account::AccountSnapshot::new(
        profile.to_string(),
        Provider::Binance,
        Environment::Live,
        crate::profile_snapshot::test_trading_profile_snapshot(),
        vec![
            SignedReadSnapshot::new(
                profile.to_string(),
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::OpenOrders {
                    market: Market::Spot,
                    symbol: None,
                },
                serde_json::json!([
                    {
                        "symbol": "BTCUSDT",
                        "orderId": 1001,
                        "clientOrderId": "spot-order",
                        "side": "BUY",
                        "type": "LIMIT",
                        "origQty": "0.10",
                        "executedQty": "0",
                        "price": "64000"
                    }
                ]),
            ),
            SignedReadSnapshot::new(
                profile.to_string(),
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::OpenOrders {
                    market: Market::UsdsFutures,
                    symbol: None,
                },
                serde_json::json!([
                    {
                        "symbol": "ETHUSDT",
                        "orderId": 2001,
                        "clientOrderId": "futures-order",
                        "side": "SELL",
                        "type": "LIMIT",
                        "origQty": "0.20",
                        "executedQty": "0.05",
                        "price": "3200"
                    }
                ]),
            ),
        ],
        Vec::new(),
    )
}

fn account_snapshot_with_order_id_only_open_order(
    profile: &str,
) -> crate::account::AccountSnapshot {
    crate::account::AccountSnapshot::new(
        profile.to_string(),
        Provider::Binance,
        Environment::Live,
        crate::profile_snapshot::test_trading_profile_snapshot(),
        vec![SignedReadSnapshot::new(
            profile.to_string(),
            Provider::Binance,
            Environment::Live,
            SignedReadRequest::OpenOrders {
                market: Market::Spot,
                symbol: None,
            },
            serde_json::json!([
                {
                    "symbol": "BTCUSDT",
                    "orderId": 3001,
                    "side": "BUY",
                    "type": "LIMIT",
                    "origQty": "0.10",
                    "executedQty": "0",
                    "price": "64000"
                }
            ]),
        )],
        Vec::new(),
    )
}

fn account_reads(profile: &str) -> Vec<SignedReadSnapshot> {
    ACCOUNT_READ_PLAN
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
        .collect()
}
