use super::*;
use crate::command::ActionId;
use crate::confirmation_dialog::{self, ConfirmationButtonAction, ConfirmationRow};
use crate::intent_review_view::{
    IntentReviewAction, action_line, action_state_for_status, staged_change_content_row,
};
use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::{FloatingKind, Panel, WorkspaceKind};
use crate::mouse_target::{self, MousePosition, MouseTarget, PanelMouseAction};
use crate::search_floating_view::SearchFloatingLayout;
use crate::staged_gate_preview::confirmation_gate_preview;
use crate::status_bar::StatusAction;
use agent_finance_core::{Environment, Market, Provider, SignedReadRequest, SignedReadSnapshot};
use agent_finance_market::snapshot::{MarketSnapshot, QuoteSnapshot, RegularBasisSnapshot};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

#[test]
fn live_writes_confirmation_blocks_mouse_focus_behind_the_modal() {
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::ToggleLiveWrites));
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        Rect::new(0, 0, 120, 40),
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 1, 1),
    );

    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::LiveWritesConfirmation)
    );
}

#[test]
fn staged_execution_confirmation_blocks_mouse_focus_behind_the_modal() {
    let mut state = staged_execution_confirmation_state();
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        Rect::new(0, 0, 120, 40),
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 1, 1),
    );

    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
}

#[test]
fn mouse_click_confirms_live_writes_confirmation() {
    let area = Rect::new(0, 0, 120, 40);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Execute(ActionId::ToggleLiveWrites));
    let mut drag = MouseDrag::default();
    let modal = floating_rect(area, &state, FloatingKind::LiveWritesConfirmation);
    let click = confirmation_click(
        &state,
        FloatingKind::LiveWritesConfirmation,
        modal,
        ConfirmationButtonAction::Primary,
    );

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert!(state.live_writes_enabled);
    assert_ne!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::LiveWritesConfirmation)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_cancels_live_writes_confirmation() {
    let area = Rect::new(0, 0, 120, 40);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Execute(ActionId::ToggleLiveWrites));
    let mut drag = MouseDrag::default();
    let modal = floating_rect(area, &state, FloatingKind::LiveWritesConfirmation);
    let click = confirmation_click(
        &state,
        FloatingKind::LiveWritesConfirmation,
        modal,
        ConfirmationButtonAction::Cancel,
    );

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert!(!state.live_writes_enabled);
    assert_ne!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::LiveWritesConfirmation)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_confirms_staged_execution_confirmation() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_execution_confirmation_state();
    let mut drag = MouseDrag::default();
    let modal = floating_rect(area, &state, FloatingKind::StagedExecutionConfirmation);
    let click = confirmation_click(
        &state,
        FloatingKind::StagedExecutionConfirmation,
        modal,
        ConfirmationButtonAction::Primary,
    );

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_some());
    assert_ne!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_cannot_confirm_typed_staged_execution_before_phrase_matches() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.transfer_ticket.set_amount_text(Some("5".to_string()));
    state.reduce(Action::StageTransferTicket);
    state.reduce(Action::ExecuteStagedChange);
    let modal = floating_rect(area, &state, FloatingKind::StagedExecutionConfirmation);
    let attempted_primary = maybe_clickable_confirmation_button(
        &mut state,
        area,
        FloatingKind::StagedExecutionConfirmation,
        modal,
        ConfirmationButtonAction::Primary,
    );

    assert!(attempted_primary.is_none());

    assert!(state.pending_staged_confirmation().is_some());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
}

#[test]
fn mouse_confirms_typed_staged_execution_after_phrase_matches() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.transfer_ticket.set_amount_text(Some("5".to_string()));
    state.reduce(Action::StageTransferTicket);
    state.reduce(Action::ExecuteStagedChange);
    for character in "TRANSFER".chars() {
        state.reduce(Action::EditStagedExecutionConfirmation(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    assert!(
        state
            .pending_staged_confirmation_gate()
            .is_some_and(|gate| gate.matched)
    );
    let mut drag = MouseDrag::default();
    let modal = floating_rect(area, &state, FloatingKind::StagedExecutionConfirmation);
    let click = clickable_confirmation_button(
        &mut state,
        area,
        FloatingKind::StagedExecutionConfirmation,
        modal,
        ConfirmationButtonAction::Primary,
    );

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_some());
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_cancels_staged_execution_confirmation() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_execution_confirmation_state();
    let mut drag = MouseDrag::default();
    let modal = floating_rect(area, &state, FloatingKind::StagedExecutionConfirmation);
    let click = confirmation_click(
        &state,
        FloatingKind::StagedExecutionConfirmation,
        modal,
        ConfirmationButtonAction::Cancel,
    );

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_ne!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_down_focuses_panel_and_drag_resizes_columns() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let mut drag = MouseDrag::default();
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 2),
    );
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    assert_eq!(drag, MouseDrag::default());

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            layout.panel_rect(Panel::Watchlist).unwrap().right(),
            2,
        ),
    );
    assert_eq!(
        drag,
        MouseDrag {
            target: Some(MouseDragTarget::DockedSplit(DockedColumnSplit::LeftMain)),
        }
    );

    let previous_left_ratio = state.layout.left_ratio;
    let drag_column = layout
        .panel_rect(Panel::Watchlist)
        .unwrap()
        .right()
        .saturating_add(24)
        .min(area.right().saturating_sub(2));
    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Drag(MouseButton::Left), drag_column, 2),
    );
    assert!(state.layout.left_ratio > previous_left_ratio);

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Up(MouseButton::Left), drag_column, 2),
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_status_workspace_tab_switches_workspace() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 31),
    );

    assert_eq!(state.workspace, WorkspaceKind::Account);
    assert_eq!(state.panels.focused(), Panel::Account);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_watchlist_row_selects_symbol() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string(), "LITE".to_string()],
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Watchlist)
    .expect("watchlist panel is visible");

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            panel.x + 2,
            panel.y + 2,
        ),
    );

    assert_eq!(state.selected_symbol(), Some("CRDO"));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_intent_review_row_selects_staged_change() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state
        .order_ticket
        .set_quantity_text(Some("0.05".to_string()));
    state.order_ticket.set_price_text(Some("204".to_string()));
    state.reduce(Action::StageOrderTicket);
    state.order_ticket.set_price_text(Some("198".to_string()));
    state.reduce(Action::StageOrderTicket);
    assert!(
        state.staged_change_views()[1].selected,
        "new staged change starts selected"
    );

    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::IntentReview)
    .expect("intent review panel is visible");

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            panel.x + 2,
            panel.y + 1 + staged_change_content_row(0) as u16,
        ),
    );

    let changes = state.staged_change_views();
    assert!(changes[0].selected);
    assert!(!changes[1].selected);
    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_movement_tracks_intent_review_summary_action_hover() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_review_state();
    let mut drag = MouseDrag::default();
    let (column, row) =
        intent_review_action_cell(area, &state, IntentReviewAction::ExecuteSelected);

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, column, row),
    );

    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::PanelAction {
            panel: Panel::IntentReview,
            action: PanelMouseAction::IntentReviewAction {
                action: IntentReviewAction::ExecuteSelected,
            },
        })
    );
}

#[test]
fn mouse_click_on_intent_review_execute_action_opens_confirmation() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_review_state();
    let mut drag = MouseDrag::default();
    let (column, row) =
        intent_review_action_cell(area, &state, IntentReviewAction::ExecuteSelected);

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), column, row),
    );

    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn running_intent_review_change_does_not_expose_mouse_actions() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_review_state();
    let (execute_column, action_row) =
        intent_review_action_cell(area, &state, IntentReviewAction::ExecuteSelected);
    let (close_column, _) =
        intent_review_action_cell(area, &state, IntentReviewAction::CloseSelected);
    state.reduce(Action::ExecuteStagedChange);
    state.reduce(Action::ConfirmStagedExecution);
    let mut drag = MouseDrag::default();

    for column in [execute_column, close_column] {
        handle_mouse_event(
            area,
            &mut state,
            &mut drag,
            mouse_event(MouseEventKind::Moved, column, action_row),
        );

        assert_eq!(state.floating.last().map(|pane| pane.kind), None);
        assert!(!matches!(
            current_mouse_target(area, &state),
            Some(MouseTarget::PanelAction {
                panel: Panel::IntentReview,
                ..
            })
        ));

        handle_mouse_event(
            area,
            &mut state,
            &mut drag,
            mouse_event(MouseEventKind::Down(MouseButton::Left), column, action_row),
        );

        assert_eq!(state.floating.last().map(|pane| pane.kind), None);
        assert!(state.pending_staged_confirmation().is_none());
    }
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_intent_review_close_action_closes_selected_change() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_review_state();
    let mut drag = MouseDrag::default();
    assert_eq!(state.staged_change_count(), 2);
    assert_eq!(
        state
            .staged_change_views()
            .iter()
            .filter(|change| change.selected)
            .count(),
        1
    );
    let (column, row) = intent_review_action_cell(area, &state, IntentReviewAction::CloseSelected);

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Down(MouseButton::Left), column, row),
    );

    assert_eq!(state.staged_change_count(), 1);
    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn narrow_intent_review_does_not_click_hidden_summary_action() {
    let area = Rect::new(0, 0, 72, 30);
    let mut state = staged_review_state();
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::IntentReview)
    .expect("intent review panel is visible");

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            panel.right().saturating_sub(2),
            panel.y + 2,
        ),
    );

    assert_ne!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::StagedExecutionConfirmation)
    );
    assert_eq!(state.staged_change_count(), 2);
}

#[test]
fn intent_review_row_selection_works_when_click_column_is_outside_inner_content() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = staged_review_state();
    let mut drag = MouseDrag::default();
    assert!(state.staged_change_views()[1].selected);
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::IntentReview)
    .expect("intent review panel is visible");

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            panel.right().saturating_sub(1),
            panel.y + 1 + staged_change_content_row(0) as u16,
        ),
    );

    assert!(state.staged_change_views()[0].selected);
}

#[test]
fn mouse_click_on_open_order_row_selects_order() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OpenOrders)
    .expect("open orders panel is visible");

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            panel.x + 2,
            panel.y + 4,
        ),
    );

    assert_eq!(state.selected_open_order, 1);
    assert_eq!(state.panels.focused(), Panel::OpenOrders);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_open_order_cancel_action_stages_cancel() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OpenOrders)
    .expect("open orders panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::OpenOrders,
        ActionId::StageSelectedOpenOrderCancel,
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.staged_change_count(), 1);
    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn narrow_open_order_action_line_does_not_create_mouse_action() {
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));
    let panel = Rect::new(0, 0, 20, 10);
    let open_orders = state.account_snapshot.as_ref().unwrap().open_orders();
    let action_row =
        crate::open_order_view::open_order_rows(&open_orders, state.selected_open_order).len()
            as u16;

    let action = crate::panel_mouse::click_action(
        &state,
        Panel::OpenOrders,
        panel,
        panel.right().saturating_sub(2),
        panel.y + action_row + 1,
    );

    assert_eq!(action, None);
}

#[test]
fn mouse_click_on_account_open_order_row_selects_order() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));
    state.selected_open_order = 1;
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Account)
    .expect("account panel is visible");

    let click = clickable_panel_row(&mut state, area, panel, Panel::Account, 0);
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.selected_open_order, 0);
    assert_eq!(state.panels.focused(), Panel::Account);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_open_order_cancel_action_stages_cancel() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Account)
    .expect("account panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::Account,
        ActionId::StageSelectedOpenOrderCancel,
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.staged_change_count(), 1);
    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_transfer_action_focuses_transfer_ticket() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Account)
    .expect("account panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::Account,
        ActionId::FocusPanel(Panel::TransferTicket),
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.panels.focused(), Panel::TransferTicket);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_holding_prefills_transfer_ticket() {
    let area = Rect::new(0, 0, 200, 80);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_transferable_holdings("mainnet"));
    state.reduce(Action::Focus(Panel::Account));
    state.reduce(Action::ToggleFocusedZoom);
    let mut drag = MouseDrag::default();
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let panel = layout
        .panel_rect(Panel::Account)
        .expect("zoomed account panel is visible");
    let click = clickable_account_row_action(&state, panel, "USDT wallet:7.25");

    handle_mouse_event(area, &mut state, &mut drag, click);

    let preview = state.transfer_ticket_preview();
    assert_eq!(state.panels.focused(), Panel::TransferTicket);
    assert_eq!(
        preview.direction,
        agent_finance_core::TransferDirection::UsdsFuturesToSpot
    );
    assert_eq!(preview.asset, "USDT");
    assert_eq!(preview.amount.as_deref(), Some("4.5"));
    assert!(preview.ready);
    assert_eq!(state.staged_change_count(), 0);
    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_position_prefills_futures_state_ticket() {
    let area = Rect::new(0, 0, 200, 80);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_transferable_holdings("mainnet"));
    state.reduce(Action::Focus(Panel::Account));
    state.reduce(Action::ToggleFocusedZoom);
    let mut drag = MouseDrag::default();
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let panel = layout
        .panel_rect(Panel::Account)
        .expect("zoomed account panel is visible");
    let click = clickable_account_row_action(&state, panel, "ETHUSDT LONG amt:0.25");

    handle_mouse_event(area, &mut state, &mut drag, click);

    let preview = state.futures_state_ticket_preview();
    assert_eq!(state.panels.focused(), Panel::FuturesState);
    assert_eq!(
        preview.kind,
        agent_finance_core::FuturesStateChangeKind::Leverage
    );
    assert_eq!(preview.symbol.as_deref(), Some("ETHUSDT"));
    assert!(!preview.ready);
    assert_eq!(preview.blockers, vec!["leverage is required"]);
    assert_eq!(state.staged_change_count(), 0);
    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_holding_text_does_not_prefill_transfer_ticket() {
    let area = Rect::new(0, 0, 200, 80);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.account_snapshot = Some(account_snapshot_with_transferable_holdings("mainnet"));
    state.reduce(Action::Focus(Panel::Account));
    state.reduce(Action::ToggleFocusedZoom);
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Account)
    .expect("zoomed account panel is visible");
    let click = account_row_text_click(&state, panel, "USDT wallet:7.25");

    handle_mouse_event(area, &mut state, &mut drag, click);

    let preview = state.transfer_ticket_preview();
    assert_eq!(state.panels.focused(), Panel::Account);
    assert_eq!(preview.asset, "USDT");
    assert_eq!(preview.amount, None);
    assert!(!preview.ready);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_account_futures_state_action_focuses_futures_state() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Account)
    .expect("account panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::Account,
        ActionId::FocusPanel(Panel::FuturesState),
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.panels.focused(), Panel::FuturesState);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_settings_row_selects_setting() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Settings)
    .expect("settings panel is visible");

    handle_mouse_event(area, &mut state, &mut drag, panel_click(panel, 14));

    assert_eq!(state.settings_editor.selected().label(), "theme accent");
    assert_eq!(state.panels.focused(), Panel::Settings);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_settings_adjust_action_updates_setting() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..crate::config::TuiConfig::default()
    });
    let original_provider = state.providers.equity;
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Settings)
    .expect("settings panel is visible");

    let click = clickable_panel_target(
        &mut state,
        area,
        panel,
        |target| target.panel_setting_adjust_hovered(Panel::Settings, 0, 1),
        "settings next action was not found",
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_ne!(state.providers.equity, original_provider);
    assert_eq!(state.settings_editor.selected().label(), "equity provider");
    assert_eq!(state.config_changes, vec!["providers"]);
    assert_eq!(state.panels.focused(), Panel::Settings);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_profile_risk_action_executes_action() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Settings,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::ProfileRisk)
    .expect("profile risk panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::ProfileRisk,
        ActionId::OpenFloating(FloatingKind::TradingProfile),
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::TradingProfile)
    );
    assert_eq!(state.panels.focused(), Panel::ProfileRisk);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_order_ticket_field_selects_that_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");

    let click = clickable_panel_field(&mut state, area, panel, Panel::OrderTicket, 4);
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.order_ticket.selected_field_label(), "price");
    assert_eq!(state.panels.focused(), Panel::OrderTicket);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_order_ticket_field_adjusts_that_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");

    let click = clickable_panel_field_adjust(&mut state, area, panel, Panel::OrderTicket, 1, 1);
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.order_ticket.selected_field_label(), "side");
    assert_eq!(state.order_ticket_preview().side.to_string(), "sell");
    assert_eq!(state.panels.focused(), Panel::OrderTicket);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_order_ticket_capture_price_action_fixes_quote_price() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.market_snapshot = Some(market_snapshot_with_price("CRDO", 250.0));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::OrderTicket,
        ActionId::CaptureOrderReferencePrice,
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.panels.focused(), Panel::OrderTicket);
    assert_eq!(state.order_ticket.selected_field_label(), "price");
    assert_eq!(
        state.order_ticket_preview().price.as_deref(),
        Some("250.00")
    );
    assert_eq!(state.staged_change_count(), 0);
    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_order_ticket_capture_price_blank_space_does_not_fix_price() {
    let area = Rect::new(0, 0, 240, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state.market_snapshot = Some(market_snapshot_with_price("CRDO", 250.0));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");
    let action_click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::OrderTicket,
        ActionId::CaptureOrderReferencePrice,
    );
    let blank_column = panel.x
        + 1
        + crate::order_ticket_controls::ORDER_TICKET_ACTIONS[0]
            .label
            .len() as u16
        + 4;
    assert!(blank_column < panel.right().saturating_sub(1));
    let blank_click = mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        blank_column,
        action_click.row,
    );

    handle_mouse_event(area, &mut state, &mut drag, blank_click);

    assert_eq!(state.panels.focused(), Panel::OrderTicket);
    assert_eq!(state.order_ticket.selected_field_label(), "market");
    assert_eq!(state.staged_change_count(), 0);
    assert!(state.pending_staged_confirmation().is_none());
    assert!(state.take_pending_staged_execution().is_none());
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_order_ticket_edit_field_action_opens_input() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        ..crate::config::TuiConfig::default()
    });
    state.reduce(Action::SelectOrderTicketField(
        crate::order_ticket::OrderTicketField::Quantity.index(),
    ));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");

    let click = clickable_panel_action(
        &mut state,
        area,
        panel,
        Panel::OrderTicket,
        ActionId::OpenTicketTextInput,
    );
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.panels.focused(), Panel::OrderTicket);
    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::TicketTextInput)
    );
    assert_eq!(state.ticket_text_input.target().field_label(), "quantity");
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_ready_order_ticket_stages_review() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state
        .order_ticket
        .set_quantity_text(Some("0.05".to_string()));
    state.order_ticket.set_price_text(Some("204".to_string()));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::OrderTicket)
    .expect("order ticket panel is visible");

    let click = clickable_panel_ready_action(&mut state, area, panel, Panel::OrderTicket);
    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.staged_change_views().len(), 1);
    assert_eq!(state.panels.focused(), Panel::IntentReview);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_transfer_ticket_field_selects_that_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::TransferTicket)
    .expect("transfer ticket panel is visible");

    handle_mouse_event(area, &mut state, &mut drag, panel_click(panel, 3));

    assert_eq!(state.transfer_ticket.selected_field_label(), "amount");
    assert_eq!(state.panels.focused(), Panel::TransferTicket);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_transfer_ticket_field_adjusts_that_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::TransferTicket)
    .expect("transfer ticket panel is visible");

    let click = clickable_panel_field_adjust(&mut state, area, panel, Panel::TransferTicket, 2, 1);
    handle_mouse_event(area, &mut state, &mut drag, click);

    let preview = state.transfer_ticket_preview();
    assert_eq!(state.transfer_ticket.selected_field_label(), "amount");
    assert_eq!(preview.amount.as_deref(), Some("1"));
    assert_eq!(state.panels.focused(), Panel::TransferTicket);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_futures_state_field_adjusts_that_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["BTCUSDT".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::FuturesState)
    .expect("futures state panel is visible");

    let click = clickable_panel_field_adjust(&mut state, area, panel, Panel::FuturesState, 2, 1);
    handle_mouse_event(area, &mut state, &mut drag, click);

    let preview = state.futures_state_ticket_preview();
    assert_eq!(state.futures_state_ticket.selected_field_label(), "value");
    assert_eq!(preview.leverage, Some(1));
    assert_eq!(state.panels.focused(), Panel::FuturesState);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_click_on_inactive_futures_scope_field_does_not_select_value() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    state
        .futures_state_ticket
        .adjust_selected_field(-1, Some("BTCUSDT"));
    let mut drag = MouseDrag::default();
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::FuturesState)
    .expect("futures state panel is visible");

    handle_mouse_event(area, &mut state, &mut drag, panel_click(panel, 2));

    assert_eq!(state.futures_state_ticket.selected_field_label(), "kind");
    assert_eq!(state.panels.focused(), Panel::FuturesState);
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_cannot_adjust_inactive_futures_scope_field() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Account,
        },
        ..crate::config::TuiConfig::default()
    });
    state
        .futures_state_ticket
        .adjust_selected_field(-1, Some("BTCUSDT"));
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::FuturesState)
    .expect("futures state panel is visible");

    let click =
        maybe_clickable_panel_field_adjust(&mut state, area, panel, Panel::FuturesState, 1, 1);

    assert!(click.is_none());
    assert_eq!(
        state.futures_state_ticket_preview().kind,
        agent_finance_core::FuturesStateChangeKind::PositionMode
    );
    assert_eq!(state.futures_state_ticket.selected_field_label(), "kind");
}

#[test]
fn mouse_wheel_moves_focused_panel_and_search_selection() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string(), "BTCUSDT".to_string()],
        ..crate::config::TuiConfig::default()
    });
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::ScrollDown, 2, 2),
    );

    assert_eq!(state.selected_symbol(), Some("BTCUSDT"));

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    assert_eq!(state.command_palette.selected(), 0);

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::ScrollDown, 2, 2),
    );

    assert_eq!(state.command_palette.selected(), 1);
}

#[test]
fn mouse_wheel_uses_hovered_panel_before_focused_panel() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string(), "BTCUSDT".to_string()],
        ..crate::config::TuiConfig::default()
    });
    state.reduce(Action::Focus(Panel::Quote));
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Watchlist)
    .expect("watchlist panel is visible");
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::ScrollDown, panel.x + 2, panel.y + 2),
    );

    assert_eq!(state.selected_symbol(), Some("BTCUSDT"));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
}

#[test]
fn mouse_wheel_over_read_only_panel_does_not_fall_back_to_focused_panel() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string(), "BTCUSDT".to_string()],
        ..crate::config::TuiConfig::default()
    });
    state.reduce(Action::Focus(Panel::Watchlist));
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Quote)
    .expect("quote panel is visible");
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::ScrollDown, panel.x + 2, panel.y + 2),
    );

    assert_eq!(state.selected_symbol(), Some("CRDO"));
    assert_eq!(state.panels.focused(), Panel::Quote);
}

#[test]
fn mouse_movement_tracks_workspace_tab_hover() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, 0, area.bottom() - 1),
    );

    assert_eq!(
        state.mouse_position,
        Some(MousePosition::new(0, area.bottom() - 1))
    );
    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::WorkspaceTab(WorkspaceKind::Market))
    );
}

#[test]
fn mouse_movement_tracks_visible_status_action_hover() {
    let area = Rect::new(0, 0, 180, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let mut drag = MouseDrag::default();
    let column = visible_status_action_column(
        area,
        &state,
        ActionId::OpenFloating(FloatingKind::CommandPalette),
    );

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, column, area.bottom() - 1),
    );

    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::StatusAction(StatusAction {
            label: "open command palette",
            action: ActionId::OpenFloating(FloatingKind::CommandPalette),
        }))
    );
}

#[test]
fn mouse_click_on_visible_status_action_executes_it() {
    let area = Rect::new(0, 0, 180, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let mut drag = MouseDrag::default();
    let column = visible_status_action_column(
        area,
        &state,
        ActionId::OpenFloating(FloatingKind::CommandPalette),
    );

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            column,
            area.bottom() - 1,
        ),
    );

    assert_eq!(
        state.floating.last().map(|pane| pane.kind),
        Some(FloatingKind::CommandPalette)
    );
    assert_eq!(drag, MouseDrag::default());
}

#[test]
fn mouse_movement_tracks_read_only_panel_row_hover() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Quote)
    .expect("quote panel is visible");
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, panel.x + 2, panel.y + 1),
    );

    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::PanelAction {
            panel: Panel::Quote,
            action: PanelMouseAction::InspectRow { index: 0 },
        })
    );
}

#[test]
fn mouse_movement_over_history_chart_does_not_report_info_row_hover() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Focus(Panel::History));
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::History)
    .expect("history panel is visible");
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, panel.x + 2, panel.y + 8),
    );

    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::Panel(Panel::History))
    );
}

#[test]
fn mouse_movement_tracks_clickable_panel_row_hover() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::Watchlist)
    .expect("watchlist is visible");
    let mut drag = MouseDrag::default();

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(MouseEventKind::Moved, panel.x + 2, panel.y + 1),
    );

    assert_eq!(
        state.mouse_position,
        Some(MousePosition::new(panel.x + 2, panel.y + 1))
    );
    assert_eq!(
        current_mouse_target(area, &state),
        Some(MouseTarget::PanelAction {
            panel: Panel::Watchlist,
            action: PanelMouseAction::SelectRow { index: 0 },
        })
    );
}

#[test]
fn mouse_clicking_command_palette_result_executes_that_command() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    for character in "workspace account".chars() {
        state.reduce(Action::EditCommandQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::SetWorkspace(WorkspaceKind::Account))
    );
    let floating = floating_rect(area, &state, FloatingKind::CommandPalette);
    let click = floating_search_entry_click(floating, state.command_palette.len(), 0, 0);
    let mut drag = MouseDrag::default();

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.workspace, WorkspaceKind::Account);
}

#[test]
fn mouse_clicking_symbol_search_result_selects_that_symbol() {
    let area = Rect::new(0, 0, 120, 32);
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["AAOI".to_string(), "LITE".to_string(), "CRDO".to_string()],
        ..crate::config::TuiConfig::default()
    });
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::SymbolSearch,
    )));
    let floating = floating_rect(area, &state, FloatingKind::SymbolSearch);
    let click = floating_search_entry_click(floating, state.symbol_search.len(), 0, 2);
    let mut drag = MouseDrag::default();

    handle_mouse_event(area, &mut state, &mut drag, click);

    assert_eq!(state.selected_symbol(), Some("CRDO"));
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::SymbolSearch)
    );
}

#[test]
fn mouse_focuses_and_resizes_floating_panes() {
    let area = Rect::new(0, 0, 160, 48);
    let mut state = AppState::from_config(crate::config::TuiConfig::default());
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::ProviderDetails,
    )));
    let mut drag = MouseDrag::default();

    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let help = layout
        .floating
        .iter()
        .find(|pane| pane.kind == FloatingKind::Help)
        .unwrap();
    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            help.rect.x + 2,
            help.rect.y + 2,
        ),
    );
    assert_eq!(state.floating.last().unwrap().kind, FloatingKind::Help);
    assert_eq!(drag, MouseDrag::default());

    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let help = layout
        .floating
        .iter()
        .find(|pane| pane.kind == FloatingKind::Help)
        .unwrap();
    let previous_rect = help.rect;
    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            help.rect.right() - 1,
            help.rect.bottom() - 1,
        ),
    );

    handle_mouse_event(
        area,
        &mut state,
        &mut drag,
        mouse_event(
            MouseEventKind::Drag(MouseButton::Left),
            help.rect.right().saturating_add(8),
            help.rect.bottom().saturating_add(4),
        ),
    );
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let resized = layout
        .floating
        .iter()
        .find(|pane| pane.kind == FloatingKind::Help)
        .unwrap();
    assert!(resized.rect.width > previous_rect.width);
    assert!(resized.rect.height > previous_rect.height);
    assert_eq!(
        layout.hit_test(resized.rect.x + 1, resized.rect.y + 1),
        Some(LayoutHit::Floating(FloatingKind::Help))
    );

    state.reduce(Action::CloseFocusedFloating);
    assert!(
        !state
            .floating
            .iter()
            .any(|pane| pane.kind == FloatingKind::Help)
    );
}

fn account_snapshot_with_open_orders(profile: &str) -> crate::AccountSnapshot {
    crate::AccountSnapshot::new(
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
                    "orderId": 1001,
                    "clientOrderId": "spot-order",
                    "side": "BUY",
                    "type": "LIMIT",
                    "origQty": "0.10",
                    "executedQty": "0",
                    "price": "64000"
                },
                {
                    "symbol": "ETHUSDT",
                    "orderId": 1002,
                    "clientOrderId": "eth-order",
                    "side": "SELL",
                    "type": "LIMIT",
                    "origQty": "0.20",
                    "executedQty": "0.05",
                    "price": "3200"
                }
            ]),
        )],
        Vec::new(),
    )
}

fn account_snapshot_with_transferable_holdings(profile: &str) -> crate::AccountSnapshot {
    crate::AccountSnapshot::new(
        profile.to_string(),
        Provider::Binance,
        Environment::Live,
        crate::profile_snapshot::test_trading_profile_snapshot(),
        vec![
            SignedReadSnapshot::new(
                profile.to_string(),
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::SpotBalances,
                serde_json::json!({
                    "balances": [
                        { "asset": "USDT", "free": "12.5", "locked": "0" },
                        { "asset": "ETH", "free": "0", "locked": "0" }
                    ]
                }),
            ),
            SignedReadSnapshot::new(
                profile.to_string(),
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::UsdsFuturesPositions,
                serde_json::json!({
                    "assets": [
                        {
                            "asset": "USDT",
                            "walletBalance": "7.25",
                            "availableBalance": "5",
                            "marginBalance": "6.75",
                            "maxWithdrawAmount": "4.5",
                            "unrealizedProfit": "-0.5"
                        }
                    ],
                    "positions": [
                        {
                            "symbol": "ETHUSDT",
                            "positionSide": "LONG",
                            "positionAmt": "0.25",
                            "notional": "1000",
                            "isolatedMargin": "0",
                            "isolatedWallet": "0",
                            "unrealizedProfit": "12.5"
                        }
                    ]
                }),
            ),
        ],
        Vec::new(),
    )
}

fn market_snapshot_with_price(symbol: &str, price: f64) -> MarketSnapshot {
    MarketSnapshot {
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        quotes: vec![QuoteSnapshot {
            symbol: symbol.to_string(),
            price: Some(price),
            currency: Some("USD".to_string()),
            provider: "test".to_string(),
            session: Some("regular".to_string()),
            market_time_local: None,
            change_pct: None,
            aliases: Vec::new(),
            regular_basis: RegularBasisSnapshot {
                previous_close: None,
                open: None,
                high: None,
                low: None,
                volume: None,
            },
        }],
        errors: Vec::new(),
    }
}

fn staged_execution_confirmation_state() -> AppState {
    let mut state = staged_review_state();
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::ExecuteStagedChange);
    state
}

fn staged_review_state() -> AppState {
    let mut state = AppState::from_config(crate::config::TuiConfig {
        watchlist: vec!["CRDO".to_string()],
        workspace: crate::config::WorkspaceConfig {
            current: WorkspaceKind::Trade,
        },
        trading: crate::config::TradingConfig {
            default_profile: Some("mainnet".to_string()),
        },
        ..crate::config::TuiConfig::default()
    });
    state
        .order_ticket
        .set_quantity_text(Some("0.05".to_string()));
    state.order_ticket.set_price_text(Some("204".to_string()));
    state.reduce(Action::StageOrderTicket);
    state.order_ticket.set_price_text(Some("198".to_string()));
    state.reduce(Action::StageOrderTicket);
    state
}

fn intent_review_action_cell(
    area: Rect,
    state: &AppState,
    action: IntentReviewAction,
) -> (u16, u16) {
    let panel = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .panel_rect(Panel::IntentReview)
    .expect("intent review panel is visible");
    let hidden = state
        .staged_change_count()
        .saturating_sub(crate::state::VISIBLE_REVIEW_LIMIT);
    let action_state = action_state_for_status(
        state
            .staged_change_review_views()
            .iter()
            .find(|change| change.selected)
            .map(|change| change.stage.queue_status()),
    );
    let line = action_line(hidden, panel.width.saturating_sub(2), action_state);
    let span = line
        .actions
        .into_iter()
        .find(|span| span.action == action)
        .expect("intent review action is visible");
    (
        panel.x + 1 + span.start,
        panel.y + 1 + crate::intent_review_view::INTENT_REVIEW_ACTION_ROW as u16,
    )
}

fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn panel_click(panel: Rect, content_row: u16) -> MouseEvent {
    mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        panel.x + 2,
        panel.y + content_row + 1,
    )
}

fn clickable_panel_row(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_index: usize,
) -> MouseEvent {
    let mut drag = MouseDrag::default();
    for content_row in 0..panel.height.saturating_sub(2) {
        let column = panel.x + 2;
        let row = panel.y + content_row + 1;
        handle_mouse_event(
            area,
            state,
            &mut drag,
            mouse_event(MouseEventKind::Moved, column, row),
        );
        if current_mouse_target(area, state)
            == Some(MouseTarget::PanelAction {
                panel: target_panel,
                action: PanelMouseAction::SelectRow {
                    index: target_index,
                },
            })
        {
            return mouse_event(MouseEventKind::Down(MouseButton::Left), column, row);
        }
    }
    panic!("clickable panel row was not found");
}

fn account_row_text_click(state: &AppState, panel: Rect, needle: &str) -> MouseEvent {
    let content_width = panel.width.saturating_sub(2);
    let content_row = crate::account_panel_view::rows_for_width(state, None, content_width)
        .iter()
        .position(|row| line_text(&row.line).contains(needle))
        .expect("clickable account row text is visible");
    mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        panel.x + 2,
        panel.y + content_row as u16 + 1,
    )
}

fn clickable_account_row_action(state: &AppState, panel: Rect, needle: &str) -> MouseEvent {
    let content_width = panel.width.saturating_sub(2);
    let (content_row, action_start) =
        crate::account_panel_view::rows_for_width(state, None, content_width)
            .iter()
            .enumerate()
            .find_map(|(content_row, row)| {
                if !line_text(&row.line).contains(needle) {
                    return None;
                }
                Some((content_row, row.preset_actions.first()?.start))
            })
            .expect("account row action is visible");
    mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        panel.x + 1 + action_start,
        panel.y + content_row as u16 + 1,
    )
}

fn line_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn clickable_panel_field(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_index: usize,
) -> MouseEvent {
    clickable_panel_target(
        state,
        area,
        panel,
        |target| {
            target
                == MouseTarget::PanelAction {
                    panel: target_panel,
                    action: PanelMouseAction::SelectField {
                        index: target_index,
                    },
                }
        },
        "clickable panel field was not found",
    )
}

fn clickable_panel_field_adjust(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_index: usize,
    direction: isize,
) -> MouseEvent {
    maybe_clickable_panel_field_adjust(state, area, panel, target_panel, target_index, direction)
        .expect("clickable panel field adjust action was not found")
}

fn maybe_clickable_panel_field_adjust(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_index: usize,
    direction: isize,
) -> Option<MouseEvent> {
    maybe_clickable_panel_target(state, area, panel, |target| {
        target.panel_field_adjust_hovered(target_panel, target_index, direction)
    })
}

fn clickable_panel_ready_action(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
) -> MouseEvent {
    clickable_panel_target(
        state,
        area,
        panel,
        |target| {
            target
                == MouseTarget::PanelAction {
                    panel: target_panel,
                    action: PanelMouseAction::StageReadyChange,
                }
        },
        "clickable panel ready action was not found",
    )
}

fn clickable_panel_action(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_action: ActionId,
) -> MouseEvent {
    clickable_panel_target(
        state,
        area,
        panel,
        |target| target.panel_action_hovered(target_panel, target_action),
        "clickable panel action was not found",
    )
}

fn clickable_panel_target(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    matches_target: impl Fn(MouseTarget) -> bool,
    missing: &str,
) -> MouseEvent {
    maybe_clickable_panel_target(state, area, panel, matches_target).unwrap_or_else(|| {
        panic!("{missing}");
    })
}

fn maybe_clickable_panel_target(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    matches_target: impl Fn(MouseTarget) -> bool,
) -> Option<MouseEvent> {
    let mut drag = MouseDrag::default();
    for content_row in 0..panel.height.saturating_sub(2) {
        for content_column in 0..panel.width.saturating_sub(2) {
            let column = panel.x + content_column + 1;
            let row = panel.y + content_row + 1;
            handle_mouse_event(
                area,
                state,
                &mut drag,
                mouse_event(MouseEventKind::Moved, column, row),
            );
            if current_mouse_target(area, state).is_some_and(&matches_target) {
                return Some(mouse_event(
                    MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                ));
            }
        }
    }
    None
}

fn floating_click(floating: Rect, content_column: u16, content_row: u16) -> MouseEvent {
    mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        floating.x + content_column + 1,
        floating.y + content_row + 1,
    )
}

fn floating_search_entry_click(
    floating: Rect,
    total: usize,
    selected: usize,
    entry_index: usize,
) -> MouseEvent {
    let layout = SearchFloatingLayout::new(floating, total, selected);
    let result_offset = entry_index.saturating_sub(layout.window().start());
    let result_row = layout.list_area.y + 1 + result_offset as u16;
    mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        layout.list_area.x + 1,
        result_row,
    )
}

fn confirmation_click(
    state: &AppState,
    kind: FloatingKind,
    floating: Rect,
    action: ConfirmationButtonAction,
) -> MouseEvent {
    maybe_confirmation_click(state, kind, floating, action).expect("button action is present")
}

fn maybe_confirmation_click(
    state: &AppState,
    kind: FloatingKind,
    floating: Rect,
    action: ConfirmationButtonAction,
) -> Option<MouseEvent> {
    let rows = confirmation_dialog::rows_for(
        kind,
        state.pending_staged_confirmation_view(),
        confirmation_gate_preview(kind, state, state.pending_staged_confirmation_view()),
        floating.width.saturating_sub(2) as usize,
    );
    let (row, buttons) = rows
        .iter()
        .enumerate()
        .find_map(|(row, item)| match item {
            ConfirmationRow::Buttons(buttons) => Some((row as u16, buttons)),
            _ => None,
        })
        .expect("confirmation button row is present");
    let button = confirmation_dialog::button_segments(buttons)
        .into_iter()
        .find(|segment| segment.action == Some(action))?;
    let column = button.start.saturating_add(1) as u16;
    Some(floating_click(floating, column, row))
}

fn clickable_confirmation_button(
    state: &mut AppState,
    area: Rect,
    kind: FloatingKind,
    floating: Rect,
    action: ConfirmationButtonAction,
) -> MouseEvent {
    maybe_clickable_confirmation_button(state, area, kind, floating, action)
        .expect("clickable confirmation button was not found")
}

fn maybe_clickable_confirmation_button(
    state: &mut AppState,
    area: Rect,
    kind: FloatingKind,
    floating: Rect,
    action: ConfirmationButtonAction,
) -> Option<MouseEvent> {
    let mut drag = MouseDrag::default();
    for content_row in 0..floating.height.saturating_sub(2) {
        for content_column in 0..floating.width.saturating_sub(2) {
            let column = floating.x + content_column + 1;
            let row = floating.y + content_row + 1;
            handle_mouse_event(
                area,
                state,
                &mut drag,
                mouse_event(MouseEventKind::Moved, column, row),
            );
            if current_mouse_target(area, state)
                .and_then(|target| target.confirmation_button_hovered(kind))
                == Some(action)
            {
                return Some(mouse_event(
                    MouseEventKind::Down(MouseButton::Left),
                    column,
                    row,
                ));
            }
        }
    }
    None
}

fn floating_rect(area: Rect, state: &AppState, kind: FloatingKind) -> Rect {
    layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    )
    .floating
    .iter()
    .find(|pane| pane.kind == kind)
    .expect("floating is visible")
    .rect
}

fn current_mouse_target(area: Rect, state: &AppState) -> Option<MouseTarget> {
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    state
        .mouse_position
        .and_then(|position| mouse_target::target_at(state, &layout, position))
}

fn visible_status_action_column(area: Rect, state: &AppState, action: ActionId) -> u16 {
    let layout = layout::build(
        area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let detail = crate::status_bar::areas(layout.status).detail;
    (detail.x..detail.right())
        .find(|column| {
            mouse_target::target_at(state, &layout, MousePosition::new(*column, detail.y))
                .is_some_and(|target| {
                    matches!(
                        target,
                        MouseTarget::StatusAction(StatusAction {
                            action: target_action,
                            ..
                        }) if target_action == action
                    )
                })
        })
        .expect("status action is visible")
}
