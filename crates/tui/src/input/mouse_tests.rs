use super::*;
use crate::command::ActionId;
use crate::confirmation_dialog::{self, ConfirmationButtonAction, ConfirmationRow};
use crate::intent_review_view::{IntentReviewAction, action_line};
use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::{FloatingKind, Panel, WorkspaceKind};
use crate::mouse_target::{self, MousePosition, MouseTarget, PanelMouseAction};
use crate::search_floating_view::SearchFloatingLayout;
use crate::status_bar::StatusAction;
use agent_finance_core::{Environment, Market, Provider, SignedReadRequest, SignedReadSnapshot};
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
            panel.y + 4,
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
            panel.y + 4,
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

    handle_mouse_event(area, &mut state, &mut drag, panel_click(panel, 6));

    assert_eq!(state.order_ticket.selected_field_label(), "price");
    assert_eq!(state.panels.focused(), Panel::OrderTicket);
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

    handle_mouse_event(area, &mut state, &mut drag, panel_click(panel, 9));

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
    let line = action_line(hidden, panel.width.saturating_sub(2));
    let span = line
        .actions
        .into_iter()
        .find(|span| span.action == action)
        .expect("intent review action is visible");
    (panel.x + 1 + span.start, panel.y + 2)
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

fn clickable_panel_action(
    state: &mut AppState,
    area: Rect,
    panel: Rect,
    target_panel: Panel,
    target_action: ActionId,
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
            .is_some_and(|target| target.panel_action_hovered(target_panel, target_action))
        {
            return mouse_event(MouseEventKind::Down(MouseButton::Left), column, row);
        }
    }
    panic!("clickable panel action was not found");
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
    let rows = confirmation_dialog::rows_for(
        kind,
        state.pending_staged_confirmation(),
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
        .find(|segment| segment.action == Some(action))
        .expect("button action is present");
    let column = button.start.saturating_add(1) as u16;
    floating_click(floating, column, row)
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
