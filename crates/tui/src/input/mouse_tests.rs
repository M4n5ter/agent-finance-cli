use super::*;
use crate::command::ActionId;
use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::{FloatingKind, Panel, WorkspaceKind};
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
fn mouse_click_on_account_panel_does_not_guess_open_order_rows() {
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
    assert_eq!(state.panels.focused(), Panel::Account);
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
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state
        .order_ticket
        .set_quantity_text(Some("0.05".to_string()));
    state.order_ticket.set_price_text(Some("204".to_string()));
    state.reduce(Action::StageOrderTicket);
    state.reduce(Action::ExecuteStagedChange);
    state
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
