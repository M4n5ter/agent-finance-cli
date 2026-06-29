use crossterm::event::{KeyCode, KeyEvent};

use crate::command::ActionId;
use crate::state::Action;
use crate::ticket_panel_view::TicketPanelAction;

const ORDER_TICKET_HINTS: &[&str] = &[
    "e edit field",
    "c capture price",
    "up/down field",
    "left/right adjust",
    "enter adjust",
    "s stage order",
];

pub(crate) const ORDER_TICKET_ACTIONS: &[TicketPanelAction] = &[
    TicketPanelAction {
        label: "[edit field]",
        action: ActionId::OpenOrderTicketInput,
    },
    TicketPanelAction {
        label: "[capture price]",
        action: ActionId::CaptureOrderReferencePrice,
    },
];

pub(crate) fn order_ticket_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Up => Some(Action::MoveOrderTicketField(-1)),
        KeyCode::Down => Some(Action::MoveOrderTicketField(1)),
        KeyCode::Left => Some(Action::AdjustOrderTicketField(-1)),
        KeyCode::Right | KeyCode::Enter => Some(Action::AdjustOrderTicketField(1)),
        KeyCode::Char('e') => Some(Action::OpenOrderTicketInput),
        KeyCode::Char('c') => Some(Action::CaptureOrderReferencePrice),
        KeyCode::Char('s') => Some(Action::StageOrderTicket),
        _ => None,
    }
}

pub(crate) fn order_ticket_key_hints() -> Vec<String> {
    ORDER_TICKET_HINTS
        .iter()
        .chain(&["q quit"])
        .copied()
        .map(str::to_string)
        .collect()
}

pub(crate) fn order_ticket_panel_hint() -> String {
    ORDER_TICKET_HINTS.join("  ")
}
