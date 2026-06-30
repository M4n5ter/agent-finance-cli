use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::chart::ChartPreset;
use crate::command::ActionId;
use crate::model::{FloatingKind, Panel};
use crate::scheduler::SymbolTaskKind;
use crate::state::{Action, AppState};

pub(crate) fn key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match state.panels.focused() {
        Panel::Watchlist => watchlist_key_action(key),
        Panel::History => history_key_action(state, key),
        Panel::OrderTicket => crate::order_ticket_controls::order_ticket_key_action(key),
        Panel::OpenOrders => crate::open_order_controls::open_order_key_action(key),
        Panel::Account => crate::account_controls::account_key_action(key),
        Panel::TransferTicket => crate::transfer_ticket_controls::transfer_ticket_key_action(key),
        Panel::FuturesState => crate::futures_state_controls::futures_state_key_action(key),
        Panel::ProfileRisk => crate::profile_risk_controls::profile_risk_key_action(key),
        Panel::Settings => crate::settings_controls::settings_key_action(key),
        Panel::IntentReview => intent_review_key_action(key),
        Panel::Quote
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => None,
    }
}

pub(crate) fn wheel_action(state: &AppState, direction: isize) -> Option<Action> {
    wheel_action_for_panel(state, state.panels.focused(), direction)
}

pub(crate) fn wheel_action_for_panel(
    _state: &AppState,
    panel: Panel,
    direction: isize,
) -> Option<Action> {
    match panel {
        Panel::Watchlist => Some(Action::Execute(ActionId::SelectSymbolBy(direction))),
        Panel::OrderTicket => Some(Action::MoveOrderTicketField(direction)),
        Panel::OpenOrders | Panel::Account => Some(Action::MoveOpenOrderSelection(direction)),
        Panel::IntentReview => Some(Action::MoveStagedChangeSelection(direction)),
        Panel::TransferTicket => Some(Action::MoveTransferTicketField(direction)),
        Panel::FuturesState => Some(Action::MoveFuturesStateTicketField(direction)),
        Panel::Settings => Some(Action::MoveSettingsSelection(direction)),
        Panel::History => Some(Action::ZoomChartWindow(-direction)),
        Panel::ProfileRisk
        | Panel::Quote
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => None,
    }
}

fn history_key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        || key.modifiers.contains(KeyModifiers::ALT)
        || key.modifiers.contains(KeyModifiers::SUPER)
    {
        return None;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Left | KeyCode::Char('h'), KeyModifiers::NONE) => {
            Some(Action::MoveChartCursor(-1))
        }
        (KeyCode::Right | KeyCode::Char('l'), KeyModifiers::NONE) => {
            Some(Action::MoveChartCursor(1))
        }
        (KeyCode::Up | KeyCode::Char('k'), KeyModifiers::NONE) => {
            chart_reference_line_action(state, -1)
        }
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
            chart_reference_line_action(state, 1)
        }
        (KeyCode::Char('['), KeyModifiers::NONE) => Some(Action::ZoomChartWindow(-1)),
        (KeyCode::Char(']'), KeyModifiers::NONE) => Some(Action::ZoomChartWindow(1)),
        (KeyCode::Char('r'), KeyModifiers::NONE) => {
            Some(Action::RequestSymbolDataRefresh(SymbolTaskKind::History))
        }
        (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::ResetChartView),
        (KeyCode::Char(key), KeyModifiers::NONE) => {
            ChartPreset::from_key(key).map(Action::SetChartPreset)
        }
        _ => None,
    }
}

fn chart_reference_line_action(state: &AppState, direction: isize) -> Option<Action> {
    if !state.chart.overlays_visible() {
        return None;
    }
    let symbol = state.selected_symbol()?;
    let line_count = crate::chart_overlay::lines_for_state(state, symbol).len();
    (line_count > 0).then_some(Action::MoveChartReferenceLine {
        direction,
        line_count,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_market::snapshot::{MarketSnapshot, QuoteSnapshot, RegularBasisSnapshot};
    use crossterm::event::KeyEvent;

    #[test]
    fn history_keys_drive_cursor_zoom_and_presets() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        assert_eq!(
            wheel_action_for_panel(&state, Panel::History, -1),
            Some(Action::ZoomChartWindow(1))
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Left)),
            Some(Action::MoveChartCursor(-1))
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('l'))),
            Some(Action::MoveChartCursor(1))
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('['))),
            Some(Action::ZoomChartWindow(-1))
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char(']'))),
            Some(Action::ZoomChartWindow(1))
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Esc)),
            Some(Action::ResetChartView)
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('2'))),
            Some(Action::SetChartPreset(ChartPreset::FiveDays))
        );
    }

    #[test]
    fn history_helper_line_keys_follow_visible_chart_overlays() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            ..crate::config::TuiConfig::default()
        });

        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            None
        );

        state.market_snapshot = Some(MarketSnapshot {
            fetched_at_local: None,
            quotes: vec![QuoteSnapshot {
                symbol: "CRDO".to_string(),
                price: Some(109.5),
                currency: Some("USD".to_string()),
                provider: "test".to_string(),
                session: Some("regular".to_string()),
                market_time_local: None,
                change_pct: None,
                aliases: Vec::new(),
                regular_basis: RegularBasisSnapshot {
                    previous_close: Some(105.0),
                    open: Some(101.0),
                    high: Some(112.0),
                    low: Some(96.0),
                    volume: None,
                },
            }],
            errors: Vec::new(),
        });

        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            Some(Action::MoveChartReferenceLine {
                direction: 1,
                line_count: 5
            })
        );
        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('k'))),
            Some(Action::MoveChartReferenceLine {
                direction: -1,
                line_count: 5
            })
        );

        state.reduce(Action::ToggleChartOverlays);

        assert_eq!(
            history_key_action(&state, KeyEvent::from(KeyCode::Char('j'))),
            None
        );
    }
}
