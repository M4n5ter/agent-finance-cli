use crossterm::event::{KeyCode, KeyEvent};

use crate::state::Action;

pub(crate) fn account_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('c') => {
            crate::open_order_controls::open_order_key_action(key)
        }
        _ => None,
    }
}

pub(crate) fn account_key_hints() -> Vec<String> {
    crate::open_order_controls::OPEN_ORDER_HINTS
        .iter()
        .chain(&["q quit"])
        .copied()
        .map(str::to_string)
        .collect()
}
