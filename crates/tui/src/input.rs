use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::FloatingKind;
use crate::mouse_target::MousePosition;
use crate::state::{Action, AppState};
use crate::workspace_tabs::workspace_tab_at;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct MouseDrag {
    target: Option<MouseDragTarget>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MouseDragTarget {
    DockedSplit(DockedColumnSplit),
    FloatingResize(FloatingKind),
}

pub fn key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if let Some(action) = crate::floating_input::key_route(state, key).captured_action() {
        return action;
    }
    if let Some(action) = crate::panel_input::key_action(state, key) {
        return Some(action);
    }

    state.keymap.normal_action(key).map(Action::Execute)
}

pub fn should_quit(state: &AppState, key: KeyEvent) -> bool {
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    if crate::floating_input::live_writes_confirmation_is_top(state)
        || crate::floating_input::staged_execution_confirmation_is_top(state)
    {
        return false;
    }
    matches!(key.code, KeyCode::Char('q'))
        && !crate::floating_input::text_input_floating_is_top(state)
}

pub fn handle_mouse_event(
    terminal_area: Rect,
    state: &mut AppState,
    drag: &mut MouseDrag,
    mouse: MouseEvent,
) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let layout = layout::build(
                terminal_area,
                &state.layout,
                &state.floating,
                &state.visible_panels(),
            );
            state.reduce(Action::TrackMousePosition(Some(MousePosition::new(
                mouse.column,
                mouse.row,
            ))));
            drag.target = None;
            if let Some(action) = modal_mouse_action(state, &layout, mouse.column, mouse.row) {
                state.reduce(action);
                return;
            }
            if mouse_is_blocked_by_modal(state) {
                return;
            }
            match layout.hit_test(mouse.column, mouse.row) {
                Some(LayoutHit::Panel(panel)) => {
                    state.reduce(Action::Focus(panel));
                    if let Some(area) = layout.panel_rect(panel)
                        && let Some(action) = crate::panel_mouse::click_action(
                            state,
                            panel,
                            area,
                            mouse.column,
                            mouse.row,
                        )
                    {
                        state.reduce(action);
                    }
                }
                Some(LayoutHit::DockedSplit(split)) => {
                    drag.target = Some(MouseDragTarget::DockedSplit(split));
                }
                Some(LayoutHit::FloatingResize(kind)) => {
                    state.reduce(Action::FocusFloating(kind));
                    drag.target = Some(MouseDragTarget::FloatingResize(kind));
                }
                Some(LayoutHit::Floating(kind)) => {
                    if let Some(area) = layout.floating_rect(kind)
                        && let Some(action) = crate::floating_input::mouse_action(
                            state,
                            kind,
                            area,
                            mouse.column,
                            mouse.row,
                        )
                    {
                        state.reduce(action);
                    } else {
                        state.reduce(Action::FocusFloating(kind));
                    }
                }
                Some(LayoutHit::Status) => {
                    if let Some(workspace) = workspace_tab_at(terminal_area, mouse.column) {
                        state.reduce(Action::Execute(crate::command::ActionId::SetWorkspace(
                            workspace,
                        )));
                        state.reduce(Action::Focus(workspace.default_panel()));
                    } else if let Some(action) = crate::status_bar::visible_action_at(
                        state,
                        crate::status_bar::areas(layout.status).detail,
                        mouse.column,
                    ) {
                        state.reduce(Action::Execute(action.action));
                    }
                }
                None => {}
            }
        }
        MouseEventKind::ScrollUp => {
            if !mouse_is_blocked_by_modal(state) {
                state.reduce(Action::TrackMousePosition(Some(MousePosition::new(
                    mouse.column,
                    mouse.row,
                ))));
                route_mouse_wheel(terminal_area, state, mouse.column, mouse.row, -1);
            }
        }
        MouseEventKind::ScrollDown => {
            if !mouse_is_blocked_by_modal(state) {
                state.reduce(Action::TrackMousePosition(Some(MousePosition::new(
                    mouse.column,
                    mouse.row,
                ))));
                route_mouse_wheel(terminal_area, state, mouse.column, mouse.row, 1);
            }
        }
        MouseEventKind::Moved => {
            state.reduce(Action::TrackMousePosition(Some(MousePosition::new(
                mouse.column,
                mouse.row,
            ))));
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            state.reduce(Action::TrackMousePosition(Some(MousePosition::new(
                mouse.column,
                mouse.row,
            ))));
            match drag.target {
                Some(MouseDragTarget::DockedSplit(split)) => {
                    let next = layout::resize_docked_columns(
                        terminal_area,
                        split,
                        mouse.column,
                        &state.layout,
                        &state.visible_panels(),
                    );
                    state.reduce(Action::ResizeDockedColumns {
                        left_ratio: next.left_ratio,
                        main_ratio: next.main_ratio,
                    });
                }
                Some(MouseDragTarget::FloatingResize(kind)) => {
                    let size = layout::resize_floating(terminal_area, mouse.column, mouse.row);
                    state.reduce(Action::ResizeFloating { kind, size });
                }
                None => {}
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            drag.target = None;
        }
        _ => {}
    }
}

fn mouse_is_blocked_by_modal(state: &AppState) -> bool {
    crate::floating_input::live_writes_confirmation_is_top(state)
        || crate::floating_input::staged_execution_confirmation_is_top(state)
}

fn modal_mouse_action(
    state: &AppState,
    layout: &layout::CockpitLayout,
    column: u16,
    row: u16,
) -> Option<Action> {
    let kind = state.floating.last()?.kind;
    if !matches!(
        kind,
        FloatingKind::LiveWritesConfirmation | FloatingKind::StagedExecutionConfirmation
    ) {
        return None;
    }
    let floating = layout
        .floating
        .iter()
        .rev()
        .find(|floating| floating.kind == kind)?;
    crate::floating_input::mouse_action(state, kind, floating.rect, column, row)
}

fn route_mouse_wheel(
    terminal_area: Rect,
    state: &mut AppState,
    column: u16,
    row: u16,
    direction: isize,
) {
    if let Some(action) = crate::floating_input::wheel_route(state, direction) {
        if let Some(action) = action {
            state.reduce(action);
        }
        return;
    }

    let layout = layout::build(
        terminal_area,
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    if let Some(LayoutHit::Panel(panel)) = layout.hit_test(column, row) {
        let action = crate::panel_input::wheel_action_for_panel(state, panel, direction);
        state.reduce(Action::Focus(panel));
        if let Some(action) = action {
            state.reduce(action);
        }
        return;
    }

    if let Some(action) = crate::panel_input::wheel_action(state, direction) {
        state.reduce(action);
    }
}

#[cfg(test)]
mod mouse_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::keymap::{KeyBinding, KeymapConfig};
    use crate::model::{Panel, WorkspaceKind};
    use crate::settings_editor::SettingRow;
    use crossterm::event::KeyEvent;

    fn move_to_setting(state: &mut AppState, label: &str) {
        let index = SettingRow::ALL
            .iter()
            .position(|row| row.label() == label)
            .expect("setting row exists");
        for _ in 0..index {
            state.reduce(Action::MoveSettingsSelection(1));
        }
    }

    #[test]
    fn normal_mode_routes_navigation_and_overlays_to_actions() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::Execute(ActionId::SelectSymbolBy(1)))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char(':'))),
            Some(Action::Execute(ActionId::OpenFloating(
                FloatingKind::CommandPalette
            )))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('/'))),
            Some(Action::Execute(ActionId::OpenFloating(
                FloatingKind::SymbolSearch
            )))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::Execute(ActionId::CloseFocusedFloating))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('x'))),
            Some(Action::Execute(ActionId::CloseFocusedPanel))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('0'))),
            Some(Action::Execute(ActionId::RestorePanels))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Tab)),
            Some(Action::Execute(ActionId::FocusPanelBy(1)))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::BackTab)),
            Some(Action::Execute(ActionId::FocusPanelBy(-1)))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('z'))),
            Some(Action::Execute(ActionId::ToggleFocusedZoom))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('5'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::Polymarket)))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('7'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::Settings)))
        );
    }

    #[test]
    fn command_palette_mode_routes_selection_and_execution() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::CommandPalette,
        )));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveCommandSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            state.command_palette.selected_action().map(Action::Execute)
        );
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('p'))),
            Some(Action::EditCommandQuery(_))
        ));
    }

    #[test]
    fn symbol_search_mode_routes_selection_and_query_editing() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::SymbolSearch,
        )));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveSymbolSearchSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::AcceptSymbolSearch)
        );
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('a'))),
            Some(Action::EditSymbolSearchQuery(_))
        ));
    }

    #[test]
    fn order_ticket_focus_routes_field_navigation_before_global_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            keymap: KeymapConfig::from_overrides(vec![KeyBinding {
                key: "ctrl-s".parse().expect("key"),
                action: ActionId::ToggleLiveWrites,
            }]),
            ..crate::config::TuiConfig::default()
        });
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::OrderTicket)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveOrderTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Right)),
            Some(Action::AdjustOrderTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('s'))),
            Some(Action::StageOrderTicket)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('e'))),
            Some(Action::OpenTicketTextInput)
        );
        assert_eq!(
            key_action(
                &state,
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
            ),
            Some(Action::Execute(ActionId::ToggleLiveWrites))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::Execute(ActionId::SelectSymbolBy(1)))
        );
    }

    #[test]
    fn intent_review_focus_routes_review_controls_before_global_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::IntentReview)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::ExecuteStagedChange)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('k'))),
            Some(Action::MoveStagedChangeSelection(-1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::MoveStagedChangeSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('d'))),
            Some(Action::CloseSelectedStagedChange)
        );
    }

    #[test]
    fn watchlist_focus_routes_edit_keys_before_global_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Watchlist)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('a'))),
            Some(Action::Execute(ActionId::OpenFloating(
                FloatingKind::WatchlistAdd
            )))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('d'))),
            Some(Action::DeleteSelectedWatchlistSymbol)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('u'))),
            Some(Action::UndoConfigChange)
        );
        assert_eq!(
            key_action(
                &state,
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)
            ),
            None
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Right)),
            Some(Action::MoveSelectedWatchlistSymbol(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::Execute(ActionId::SelectSymbolBy(1)))
        );
    }

    #[test]
    fn watchlist_add_mode_routes_text_input_and_acceptance() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::WatchlistAdd,
        )));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::AcceptWatchlistAdd)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CloseFocusedFloating)
        );
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('l'))),
            Some(Action::EditWatchlistAddQuery(_))
        ));
    }

    #[test]
    fn ticket_text_input_mode_routes_text_input_and_acceptance() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::OrderTicket)));
        state.reduce(Action::SelectOrderTicketField(
            crate::order_ticket::OrderTicketField::Quantity.index(),
        ));
        state.reduce(Action::OpenTicketTextInput);

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::AcceptTicketTextInput)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CloseFocusedFloating)
        );
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('1'))),
            Some(Action::EditTicketTextInput(_))
        ));
    }

    #[test]
    fn account_focus_routes_open_order_selection_and_cancel_staging() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Account)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveOpenOrderSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('c'))),
            Some(Action::StageSelectedOpenOrderCancel)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char(']'))),
            Some(Action::Execute(ActionId::ShiftWorkspace(1)))
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Right)), None);
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('t'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::TransferTicket)))
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('u'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('i'))), None);
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('f'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::FuturesState)))
        );
    }

    #[test]
    fn transfer_ticket_focus_routes_transfer_staging_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::TransferTicket)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char(']'))),
            Some(Action::MoveTransferTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Right)),
            Some(Action::AdjustTransferTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('t'))),
            Some(Action::StageTransferTicket)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('c'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('f'))), None);
    }

    #[test]
    fn futures_state_focus_routes_futures_state_staging_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::FuturesState)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('u'))),
            Some(Action::MoveFuturesStateTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('i'))),
            Some(Action::AdjustFuturesStateTicketField(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('f'))),
            Some(Action::StageFuturesStateTicket)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('c'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('t'))), None);
    }

    #[test]
    fn open_orders_focus_routes_only_cancel_surface_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::OpenOrders)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveOpenOrderSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('c'))),
            Some(Action::StageSelectedOpenOrderCancel)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('t'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('f'))), None);
    }

    #[test]
    fn profile_risk_focus_routes_profile_risk_actions() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Settings,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::ProfileRisk)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('e'))),
            Some(Action::Execute(ActionId::OpenFloating(
                FloatingKind::TradingProfile
            )))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('v'))),
            Some(Action::Execute(ActionId::RevalidateTradingProfile))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('t'))),
            Some(Action::Execute(ActionId::StageProfileLiveToggle))
        );
    }

    #[test]
    fn account_local_keys_do_not_shadow_modified_global_keymap() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            keymap: KeymapConfig::from_overrides(vec![KeyBinding {
                key: "ctrl-t".parse().expect("key"),
                action: ActionId::ToggleLiveWrites,
            }]),
            ..crate::config::TuiConfig::default()
        });
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Account)));

        assert_eq!(
            key_action(
                &state,
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)
            ),
            Some(Action::Execute(ActionId::ToggleLiveWrites))
        );
    }

    #[test]
    fn account_focus_routes_operation_keys_before_global_keys() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Account)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('r'))),
            Some(Action::Execute(ActionId::RefreshAccountSnapshot))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('v'))),
            Some(Action::Execute(ActionId::RevalidateTradingProfile))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('l'))),
            Some(Action::Execute(ActionId::ToggleLiveWrites))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('t'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::TransferTicket)))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('f'))),
            Some(Action::Execute(ActionId::FocusPanel(Panel::FuturesState)))
        );
    }

    #[test]
    fn live_writes_confirmation_blocks_normal_keys_until_confirmed_or_closed() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::ToggleLiveWrites));

        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(should_quit(
            &state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::SetLiveWritesEnabled(true))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CloseFocusedFloating)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('j'))), None);
    }

    #[test]
    fn staged_execution_confirmation_blocks_normal_keys_until_confirmed_or_cancelled() {
        let state = staged_execution_confirmation_state();

        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(should_quit(
            &state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::ConfirmStagedExecution)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CancelStagedExecutionConfirmation)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('j'))), None);
    }

    #[test]
    fn typed_staged_execution_confirmation_routes_text_input() {
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

        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('T'))),
            Some(Action::EditStagedExecutionConfirmation(_))
        ));
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::ConfirmStagedExecution)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CancelStagedExecutionConfirmation)
        );
    }

    #[test]
    fn quit_router_accepts_q_and_control_c_only() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());

        assert!(should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(should_quit(
            &state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('c'))));

        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::CommandPalette,
        )));
        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('q'))),
            Some(Action::EditCommandQuery(_))
        ));

        state.reduce(Action::CloseFocusedFloating);
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::SymbolSearch,
        )));
        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('q'))),
            Some(Action::EditSymbolSearchQuery(_))
        ));

        state.reduce(Action::CloseFocusedFloating);
        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::TradingProfile,
        )));
        assert!(!should_quit(&state, KeyEvent::from(KeyCode::Char('q'))));
        assert!(matches!(
            key_action(&state, KeyEvent::from(KeyCode::Char('q'))),
            Some(Action::EditTradingProfileQuery(_))
        ));
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

    #[test]
    fn settings_focus_routes_local_controls() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Settings,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Settings)));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Down)),
            Some(Action::MoveSettingsSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Right)),
            Some(Action::AdjustSelectedSetting(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('h'))),
            Some(Action::AdjustSelectedSetting(-1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('u'))),
            Some(Action::UndoConfigChange)
        );
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('e'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('v'))), None);
        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('t'))), None);
    }

    #[test]
    fn settings_local_keys_do_not_shadow_modified_global_keymap() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Settings,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Settings)));
        move_to_setting(&mut state, "key live writes");
        state.reduce(Action::AdjustSelectedSetting(1));

        assert_eq!(
            key_action(
                &state,
                KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)
            ),
            Some(Action::Execute(ActionId::ToggleLiveWrites))
        );
    }

    #[test]
    fn settings_local_controls_do_not_keep_profile_risk_shortcuts() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Settings,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Settings)));

        assert_eq!(key_action(&state, KeyEvent::from(KeyCode::Char('t'))), None);
    }
}
