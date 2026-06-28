use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::ActionId;
use crate::model::{FloatingKind, Panel};
use crate::state::{Action, AppState};

pub(crate) fn key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match state.panels.focused() {
        Panel::Watchlist => watchlist_key_action(key),
        Panel::OrderTicket => crate::order_ticket_controls::order_ticket_key_action(key),
        Panel::OpenOrders => crate::open_order_controls::open_order_key_action(key),
        Panel::Account => crate::account_controls::account_key_action(key),
        Panel::TransferTicket => crate::transfer_ticket_controls::transfer_ticket_key_action(key),
        Panel::FuturesState => crate::futures_state_controls::futures_state_key_action(key),
        Panel::ProfileRisk => crate::profile_risk_controls::profile_risk_key_action(key),
        Panel::Settings => crate::settings_controls::settings_key_action(key),
        Panel::IntentReview => intent_review_key_action(key),
        Panel::Quote
        | Panel::History
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => None,
    }
}

fn watchlist_key_action(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        || key.modifiers.contains(KeyModifiers::ALT)
        || key.modifiers.contains(KeyModifiers::SUPER)
    {
        return None;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Up | KeyCode::Char('k'), KeyModifiers::NONE) => {
            Some(Action::Execute(ActionId::SelectSymbolBy(-1)))
        }
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
            Some(Action::Execute(ActionId::SelectSymbolBy(1)))
        }
        (KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('K'), KeyModifiers::SHIFT) => {
            Some(Action::MoveSelectedWatchlistSymbol(-1))
        }
        (KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('J'), KeyModifiers::SHIFT) => {
            Some(Action::MoveSelectedWatchlistSymbol(1))
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => Some(Action::Execute(ActionId::OpenFloating(
            FloatingKind::WatchlistAdd,
        ))),
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Action::DeleteSelectedWatchlistSymbol),
        (KeyCode::Char('u'), KeyModifiers::NONE) => Some(Action::UndoConfigChange),
        _ => None,
    }
}

fn intent_review_key_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveStagedChangeSelection(-1)),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveStagedChangeSelection(1)),
        KeyCode::Enter => Some(Action::ExecuteStagedChange),
        KeyCode::Char('d') | KeyCode::Backspace => Some(Action::CloseSelectedStagedChange),
        _ => None,
    }
}
