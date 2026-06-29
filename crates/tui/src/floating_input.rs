use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::to_input_request;

use ratatui::layout::Rect;

use crate::confirmation_dialog::{self, ConfirmationButtonAction};
use crate::model::FloatingKind;
use crate::mouse_target::{FloatingMouseAction, MouseTarget};
use crate::search_floating_view::SearchFloatingLayout;
use crate::state::{Action, AppState};

pub(crate) struct FloatingKeyRouting {
    kind: FloatingKeyRoute,
    action: Option<Action>,
}

impl FloatingKeyRouting {
    fn captured(action: Option<Action>) -> Self {
        Self {
            kind: FloatingKeyRoute::Captured,
            action,
        }
    }

    fn pass_through() -> Self {
        Self {
            kind: FloatingKeyRoute::PassThrough,
            action: None,
        }
    }

    pub(crate) fn captured_action(self) -> Option<Option<Action>> {
        match self.kind {
            FloatingKeyRoute::Captured => Some(self.action),
            FloatingKeyRoute::PassThrough => None,
        }
    }
}

enum FloatingKeyRoute {
    Captured,
    PassThrough,
}

pub(crate) fn key_route(state: &AppState, key: KeyEvent) -> FloatingKeyRouting {
    let action = match top_floating_kind(state) {
        Some(FloatingKind::CommandPalette) => command_palette_key_action(state, key),
        Some(FloatingKind::SymbolSearch) => symbol_search_key_action(key),
        Some(FloatingKind::WatchlistAdd) => watchlist_add_key_action(key),
        Some(FloatingKind::TradingProfile) => trading_profile_key_action(key),
        Some(FloatingKind::OrderTicketInput) => order_ticket_input_key_action(key),
        Some(FloatingKind::LiveWritesConfirmation) => live_writes_confirmation_key_action(key),
        Some(FloatingKind::StagedExecutionConfirmation) => {
            staged_execution_confirmation_key_action(state, key)
        }
        Some(FloatingKind::Help | FloatingKind::ProviderDetails) | None => {
            return FloatingKeyRouting::pass_through();
        }
    };
    FloatingKeyRouting::captured(action)
}

pub(crate) fn wheel_route(state: &AppState, direction: isize) -> Option<Option<Action>> {
    let action = match top_floating_kind(state)? {
        FloatingKind::CommandPalette => Some(Action::MoveCommandSelection(direction)),
        FloatingKind::SymbolSearch => Some(Action::MoveSymbolSearchSelection(direction)),
        FloatingKind::Help
        | FloatingKind::WatchlistAdd
        | FloatingKind::TradingProfile
        | FloatingKind::OrderTicketInput
        | FloatingKind::LiveWritesConfirmation
        | FloatingKind::StagedExecutionConfirmation
        | FloatingKind::ProviderDetails => None,
    };
    Some(action)
}

pub(crate) fn live_writes_confirmation_is_top(state: &AppState) -> bool {
    top_floating_kind(state) == Some(FloatingKind::LiveWritesConfirmation)
}

pub(crate) fn staged_execution_confirmation_is_top(state: &AppState) -> bool {
    top_floating_kind(state) == Some(FloatingKind::StagedExecutionConfirmation)
}

pub(crate) fn mouse_action(
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<Action> {
    floating_hit_at(state, kind, area, column, row).and_then(|hit| hit.action_for(state, kind))
}

pub(crate) fn hover_target(
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<MouseTarget> {
    floating_hit_at(state, kind, area, column, row)
        .map(|hit| MouseTarget::FloatingAction {
            kind,
            action: hit.mouse_action(),
        })
        .or(Some(MouseTarget::Floating(kind)))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FloatingHit {
    CommandResult(usize),
    SymbolResult(usize),
    ConfirmationButton(ConfirmationButtonAction),
}

impl FloatingHit {
    fn action_for(self, state: &AppState, kind: FloatingKind) -> Option<Action> {
        match self {
            Self::CommandResult(index) => state
                .command_palette
                .command_at(index)
                .map(|command| Action::Execute(command.action)),
            Self::SymbolResult(index) => state
                .symbol_search
                .symbol_index_at(index)
                .map(Action::SelectSymbolSearchSymbol),
            Self::ConfirmationButton(ConfirmationButtonAction::Primary) => match kind {
                FloatingKind::LiveWritesConfirmation => Some(Action::SetLiveWritesEnabled(true)),
                FloatingKind::StagedExecutionConfirmation => Some(Action::ConfirmStagedExecution),
                _ => None,
            },
            Self::ConfirmationButton(ConfirmationButtonAction::Cancel) => match kind {
                FloatingKind::LiveWritesConfirmation => Some(Action::CloseFocusedFloating),
                FloatingKind::StagedExecutionConfirmation => {
                    Some(Action::CancelStagedExecutionConfirmation)
                }
                _ => None,
            },
        }
    }

    const fn mouse_action(self) -> FloatingMouseAction {
        match self {
            Self::CommandResult(index) => FloatingMouseAction::ExecuteResult { index },
            Self::SymbolResult(index) => FloatingMouseAction::SelectResult { index },
            Self::ConfirmationButton(ConfirmationButtonAction::Primary) => {
                FloatingMouseAction::Confirm
            }
            Self::ConfirmationButton(ConfirmationButtonAction::Cancel) => {
                FloatingMouseAction::Cancel
            }
        }
    }
}

fn floating_hit_at(
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<FloatingHit> {
    match kind {
        FloatingKind::CommandPalette => search_result_index_at(
            state.command_palette.len(),
            state.command_palette.selected(),
            area,
            column,
            row,
        )
        .filter(|index| state.command_palette.command_at(*index).is_some())
        .map(FloatingHit::CommandResult),
        FloatingKind::SymbolSearch => search_result_index_at(
            state.symbol_search.len(),
            state.symbol_search.selected(),
            area,
            column,
            row,
        )
        .filter(|index| state.symbol_search.symbol_index_at(*index).is_some())
        .map(FloatingHit::SymbolResult),
        FloatingKind::LiveWritesConfirmation | FloatingKind::StagedExecutionConfirmation => {
            confirmation_button_at(state, kind, area, column, row)
                .map(FloatingHit::ConfirmationButton)
        }
        FloatingKind::Help
        | FloatingKind::TradingProfile
        | FloatingKind::ProviderDetails
        | FloatingKind::WatchlistAdd
        | FloatingKind::OrderTicketInput => None,
    }
}

pub(crate) fn text_input_floating_is_top(state: &AppState) -> bool {
    top_floating_kind(state).is_some_and(FloatingKind::text_input)
}

fn top_floating_kind(state: &AppState) -> Option<FloatingKind> {
    state.floating.last().map(|pane| pane.kind)
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

fn watchlist_add_key_action(key: KeyEvent) -> Option<Action> {
    simple_text_input_key_action(
        key,
        Action::EditWatchlistAddQuery,
        Action::AcceptWatchlistAdd,
    )
}

fn trading_profile_key_action(key: KeyEvent) -> Option<Action> {
    simple_text_input_key_action(
        key,
        Action::EditTradingProfileQuery,
        Action::AcceptTradingProfile,
    )
}

fn order_ticket_input_key_action(key: KeyEvent) -> Option<Action> {
    simple_text_input_key_action(
        key,
        Action::EditOrderTicketInput,
        Action::AcceptOrderTicketInput,
    )
}

fn simple_text_input_key_action(
    key: KeyEvent,
    edit: impl FnOnce(tui_input::InputRequest) -> Action,
    accept: Action,
) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(accept),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => to_input_request(&Event::Key(key)).map(edit),
    }
}

fn live_writes_confirmation_key_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(Action::SetLiveWritesEnabled(true)),
        KeyCode::Esc => Some(Action::CloseFocusedFloating),
        _ => None,
    }
}

fn staged_execution_confirmation_key_action(state: &AppState, key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Enter => Some(Action::ConfirmStagedExecution),
        KeyCode::Esc => Some(Action::CancelStagedExecutionConfirmation),
        _ if state.pending_staged_confirmation_accepts_text_input() => {
            to_input_request(&Event::Key(key)).map(Action::EditStagedExecutionConfirmation)
        }
        _ => None,
    }
}

fn confirmation_button_at(
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<ConfirmationButtonAction> {
    let (content_column, content_row) = floating_content_position(area, column, row)?;
    let content_width = area.width.saturating_sub(2) as usize;
    let rows = confirmation_dialog::rows_for(
        kind,
        state.pending_staged_confirmation_view(),
        content_width,
    );
    confirmation_dialog::click_action_at(&rows, content_column, content_row)
}

fn search_result_index_at(
    total: usize,
    selected: usize,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<usize> {
    SearchFloatingLayout::new(area, total, selected).item_at_point(column, row)
}

fn floating_content_position(area: Rect, column: u16, row: u16) -> Option<(usize, usize)> {
    if column <= area.x
        || column >= area.right().saturating_sub(1)
        || row <= area.y
        || row >= area.bottom().saturating_sub(1)
    {
        return None;
    }
    Some((
        column.saturating_sub(area.x).saturating_sub(1) as usize,
        row.saturating_sub(area.y).saturating_sub(1) as usize,
    ))
}
