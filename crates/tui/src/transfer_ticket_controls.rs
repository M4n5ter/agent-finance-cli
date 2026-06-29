use crossterm::event::{KeyCode, KeyEvent};

use crate::command::ActionId;
use crate::state::Action;
use crate::ticket_panel_view::TicketPanelAction;

const TRANSFER_HINTS: &[&str] = &[
    "e edit field",
    "[/] transfer field",
    "left/right transfer",
    "t stage transfer",
];

pub(crate) const TRANSFER_TICKET_ACTIONS: &[TicketPanelAction] = &[TicketPanelAction {
    label: "[edit field]",
    action: ActionId::OpenTicketTextInput,
}];

pub(crate) fn transfer_ticket_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char('e') => Some(Action::OpenTicketTextInput),
        KeyCode::Char('[') => Some(Action::MoveTransferTicketField(-1)),
        KeyCode::Char(']') => Some(Action::MoveTransferTicketField(1)),
        KeyCode::Left => Some(Action::AdjustTransferTicketField(-1)),
        KeyCode::Right | KeyCode::Enter => Some(Action::AdjustTransferTicketField(1)),
        KeyCode::Char('t') => Some(Action::StageTransferTicket),
        _ => None,
    }
}

pub(crate) fn transfer_ticket_key_hints() -> Vec<String> {
    TRANSFER_HINTS
        .iter()
        .chain(&["q quit"])
        .copied()
        .map(str::to_string)
        .collect()
}

pub(crate) fn transfer_ticket_section_hint() -> String {
    section_hint(TRANSFER_HINTS)
}

fn section_hint(hints: &[&str]) -> String {
    hints
        .iter()
        .map(|hint| hint.strip_prefix("transfer ").unwrap_or(hint))
        .collect::<Vec<_>>()
        .join("  ")
}
