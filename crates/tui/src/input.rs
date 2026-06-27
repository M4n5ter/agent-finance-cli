use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use tui_input::backend::crossterm::to_input_request;

use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::FloatingKind;
use crate::state::{Action, AppState};

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
    if command_palette_is_top(state) {
        return command_palette_key_action(state, key);
    }
    if symbol_search_is_top(state) {
        return symbol_search_key_action(key);
    }
    if live_writes_confirmation_is_top(state) {
        return live_writes_confirmation_key_action(key);
    }
    if state.panels.focused() == crate::model::Panel::OrderTicket
        && let Some(action) = order_ticket_key_action(key)
    {
        return Some(action);
    }

    state.keymap.normal_action(key).map(Action::Execute)
}

pub fn should_quit(state: &AppState, key: KeyEvent) -> bool {
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    if live_writes_confirmation_is_top(state) {
        return false;
    }
    matches!(key.code, KeyCode::Char('q')) && !text_input_floating_is_top(state)
}

pub fn handle_mouse_event(
    terminal_area: Rect,
    state: &mut AppState,
    drag: &mut MouseDrag,
    mouse: MouseEvent,
) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if live_writes_confirmation_is_top(state) {
                return;
            }
            let layout = layout::build(
                terminal_area,
                &state.layout,
                &state.floating,
                &state.visible_panels(),
            );
            drag.target = None;
            match layout.hit_test(mouse.column, mouse.row) {
                Some(LayoutHit::Panel(panel)) => state.reduce(Action::Focus(panel)),
                Some(LayoutHit::DockedSplit(split)) => {
                    drag.target = Some(MouseDragTarget::DockedSplit(split));
                }
                Some(LayoutHit::FloatingResize(kind)) => {
                    state.reduce(Action::FocusFloating(kind));
                    drag.target = Some(MouseDragTarget::FloatingResize(kind));
                }
                Some(LayoutHit::Floating(kind)) => state.reduce(Action::FocusFloating(kind)),
                Some(LayoutHit::Status) | None => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => match drag.target {
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
        },
        MouseEventKind::Up(MouseButton::Left) => {
            drag.target = None;
        }
        _ => {}
    }
}

fn command_palette_key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Down => Some(Action::MoveCommandSelection(1)),
        KeyCode::Up => Some(Action::MoveCommandSelection(-1)),
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveCommandSelection(1))
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveCommandSelection(-1))
        }
        KeyCode::Enter => state.command_palette.selected_action().map(Action::Execute),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => to_input_request(&Event::Key(key)).map(Action::EditCommandQuery),
    }
}

fn symbol_search_key_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Down => Some(Action::MoveSymbolSearchSelection(1)),
        KeyCode::Up => Some(Action::MoveSymbolSearchSelection(-1)),
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveSymbolSearchSelection(1))
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveSymbolSearchSelection(-1))
        }
        KeyCode::Enter => Some(Action::AcceptSymbolSearch),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => to_input_request(&Event::Key(key)).map(Action::EditSymbolSearchQuery),
    }
}

fn live_writes_confirmation_key_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(Action::SetLiveWritesEnabled(true)),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => None,
    }
}

fn order_ticket_key_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Up => Some(Action::MoveOrderTicketField(-1)),
        KeyCode::Down => Some(Action::MoveOrderTicketField(1)),
        KeyCode::Left => Some(Action::AdjustOrderTicketField(-1)),
        KeyCode::Right | KeyCode::Enter => Some(Action::AdjustOrderTicketField(1)),
        _ => None,
    }
}

fn command_palette_is_top(state: &AppState) -> bool {
    state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::CommandPalette)
}

fn live_writes_confirmation_is_top(state: &AppState) -> bool {
    state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::LiveWritesConfirmation)
}

fn symbol_search_is_top(state: &AppState) -> bool {
    state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::SymbolSearch)
}

fn text_input_floating_is_top(state: &AppState) -> bool {
    command_palette_is_top(state) || symbol_search_is_top(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::model::{Panel, WorkspaceKind};
    use crossterm::event::KeyEvent;

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
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
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
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::Execute(ActionId::SelectSymbolBy(1)))
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
    fn live_writes_confirmation_blocks_mouse_focus_behind_the_modal() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
        state.reduce(Action::Execute(ActionId::ToggleLiveWrites));
        let mut drag = MouseDrag::default();

        handle_mouse_event(
            Rect::new(0, 0, 120, 40),
            &mut state,
            &mut drag,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 1,
                row: 1,
                modifiers: KeyModifiers::empty(),
            },
        );

        assert_eq!(
            state.floating.last().map(|pane| pane.kind),
            Some(FloatingKind::LiveWritesConfirmation)
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

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }
}
