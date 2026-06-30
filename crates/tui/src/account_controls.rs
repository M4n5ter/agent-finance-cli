use crossterm::event::{KeyCode, KeyEvent};

use crate::command::ActionId;
use crate::model::Panel;
use crate::state::Action;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct AccountOperation {
    pub key: char,
    pub hint: &'static str,
    pub action: ActionId,
    pub label: AccountOperationLabel,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AccountOperationLabel {
    Static(&'static str),
    LiveWrites,
}

pub(crate) const ACCOUNT_OPERATIONS: &[AccountOperation] = &[
    AccountOperation {
        key: 'r',
        hint: "r refresh",
        action: ActionId::RefreshAccountSnapshot,
        label: AccountOperationLabel::Static("[refresh]"),
    },
    AccountOperation {
        key: 'v',
        hint: "v revalidate",
        action: ActionId::RevalidateTradingProfile,
        label: AccountOperationLabel::Static("[revalidate]"),
    },
    AccountOperation {
        key: 'l',
        hint: "l live",
        action: ActionId::ToggleLiveWrites,
        label: AccountOperationLabel::LiveWrites,
    },
    AccountOperation {
        key: 't',
        hint: "t transfer",
        action: ActionId::FocusPanel(Panel::TransferTicket),
        label: AccountOperationLabel::Static("[transfer]"),
    },
    AccountOperation {
        key: 'f',
        hint: "f futures",
        action: ActionId::FocusPanel(Panel::FuturesState),
        label: AccountOperationLabel::Static("[futures state]"),
    },
];

pub(crate) fn account_operation_label(
    operation: AccountOperation,
    live_writes_enabled: bool,
) -> &'static str {
    match operation.label {
        AccountOperationLabel::Static(label) => label,
        AccountOperationLabel::LiveWrites if live_writes_enabled => "[disable live]",
        AccountOperationLabel::LiveWrites => "[enable live]",
    }
}

pub(crate) fn account_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Up | KeyCode::Down | KeyCode::Char('c') => {
            crate::open_order_controls::open_order_key_action(key)
        }
        KeyCode::Char(key) => ACCOUNT_OPERATIONS
            .iter()
            .find(|operation| operation.key == key)
            .map(|operation| Action::Execute(operation.action)),
        _ => None,
    }
}

pub(crate) fn account_key_hints() -> Vec<String> {
    crate::open_order_controls::OPEN_ORDER_HINTS
        .iter()
        .copied()
        .chain(ACCOUNT_OPERATIONS.iter().map(|operation| operation.hint))
        .chain(["q quit"])
        .map(str::to_string)
        .collect()
}
