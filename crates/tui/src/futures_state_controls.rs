use crossterm::event::{KeyCode, KeyEvent};

use crate::state::Action;

const FUTURES_STATE_HINTS: &[&str] = &["u futures field", "i futures adjust", "f stage state"];

pub(crate) fn futures_state_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char('u') => Some(Action::MoveFuturesStateTicketField(1)),
        KeyCode::Char('i') | KeyCode::Enter => Some(Action::AdjustFuturesStateTicketField(1)),
        KeyCode::Char('f') => Some(Action::StageFuturesStateTicket),
        _ => None,
    }
}

pub(crate) fn futures_state_key_hints() -> Vec<String> {
    FUTURES_STATE_HINTS
        .iter()
        .chain(&["q quit"])
        .copied()
        .map(str::to_string)
        .collect()
}

pub(crate) fn futures_state_section_hint() -> String {
    section_hint(FUTURES_STATE_HINTS)
}

fn section_hint(hints: &[&str]) -> String {
    hints
        .iter()
        .map(|hint| hint.strip_prefix("futures ").unwrap_or(hint))
        .collect::<Vec<_>>()
        .join("  ")
}
