use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::layout::{self, DockedColumnSplit, LayoutHit};
use crate::model::{FloatingKind, Panel};
use crate::state::{Action, AppState};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct MouseDrag {
    split: Option<DockedColumnSplit>,
}

pub fn key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if command_palette_is_top(state) {
        return command_palette_key_action(state, key);
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::SelectNextSymbol),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::SelectPreviousSymbol),
        KeyCode::Char('h') | KeyCode::F(1) => Some(Action::ToggleFloating(FloatingKind::Help)),
        KeyCode::Char(':') => Some(Action::ToggleFloating(FloatingKind::CommandPalette)),
        KeyCode::Char('p') => Some(Action::ToggleFloating(FloatingKind::ProviderDetails)),
        KeyCode::Char('r') => Some(Action::ResetLayout),
        KeyCode::Char('x') => Some(Action::CloseFocusedPanel),
        KeyCode::Char('0') => Some(Action::RestorePanels),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        KeyCode::Char('1') => Some(Action::Focus(Panel::Watchlist)),
        KeyCode::Char('2') => Some(Action::Focus(Panel::Quote)),
        KeyCode::Char('3') => Some(Action::Focus(Panel::History)),
        _ => None,
    }
}

pub fn should_quit(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q'))
        || (matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL))
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
                state.panels.open_panels(),
            );
            drag.split = None;
            match layout.hit_test(mouse.column, mouse.row) {
                Some(LayoutHit::Panel(panel)) => state.reduce(Action::Focus(panel)),
                Some(LayoutHit::DockedSplit(split)) => drag.split = Some(split),
                Some(LayoutHit::Floating(_)) | Some(LayoutHit::Status) | None => {}
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(split) = drag.split {
                let next = layout::resize_docked_columns(
                    terminal_area,
                    split,
                    mouse.column,
                    &state.layout,
                    state.panels.open_panels(),
                );
                state.reduce(Action::ResizeDockedColumns {
                    left_ratio: next.left_ratio,
                    main_ratio: next.main_ratio,
                });
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            drag.split = None;
        }
        _ => {}
    }
}

fn command_palette_key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveCommandSelection(1)),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveCommandSelection(-1)),
        KeyCode::Enter => Some(Action::ApplyCommand(
            state.command_palette.selected_effect(),
        )),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => None,
    }
}

fn command_palette_is_top(state: &AppState) -> bool {
    state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::CommandPalette)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    #[test]
    fn normal_mode_routes_navigation_and_overlays_to_actions() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::SelectNextSymbol)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char(':'))),
            Some(Action::ToggleFloating(FloatingKind::CommandPalette))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::CloseFocusedFloating)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('x'))),
            Some(Action::CloseFocusedPanel)
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('0'))),
            Some(Action::RestorePanels)
        );
    }

    #[test]
    fn command_palette_mode_routes_selection_and_execution() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        state.reduce(Action::ToggleFloating(FloatingKind::CommandPalette));

        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::MoveCommandSelection(1))
        );
        assert_eq!(
            key_action(&state, KeyEvent::from(KeyCode::Enter)),
            Some(Action::ApplyCommand(
                state.command_palette.selected_effect()
            ))
        );
    }

    #[test]
    fn quit_router_accepts_q_and_control_c_only() {
        assert!(should_quit(KeyEvent::from(KeyCode::Char('q'))));
        assert!(should_quit(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!should_quit(KeyEvent::from(KeyCode::Char('c'))));
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
            state.panels.open_panels(),
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
                split: Some(DockedColumnSplit::LeftMain),
            }
        );

        handle_mouse_event(
            area,
            &mut state,
            &mut drag,
            mouse_event(MouseEventKind::Drag(MouseButton::Left), 50, 2),
        );
        assert!(state.layout.left_ratio > crate::config::LayoutConfig::default().left_ratio);

        handle_mouse_event(
            area,
            &mut state,
            &mut drag,
            mouse_event(MouseEventKind::Up(MouseButton::Left), 50, 2),
        );
        assert_eq!(drag, MouseDrag::default());
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
