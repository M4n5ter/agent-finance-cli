use ratatui::layout::Rect;

use crate::futures_state_ticket::FuturesStateTicketField;
use crate::model::Panel;
use crate::order_ticket::OrderTicketField;
use crate::state::{Action, AppState};
use crate::ticket_panel_view::{TicketPanelClick, TicketPanelRows};
use crate::transfer_ticket::TransferTicketField;

pub(crate) fn click_action(
    state: &AppState,
    panel: Panel,
    area: Rect,
    _column: u16,
    row: u16,
) -> Option<Action> {
    match panel {
        Panel::Watchlist => watchlist_click_action(state, area, row),
        Panel::OpenOrders => open_order_click_action(state, area, row),
        Panel::IntentReview => staged_change_click_action(state, area, row),
        Panel::OrderTicket => ticket_click_action(
            content_row(area, row)?,
            order_ticket_rows(state),
            Action::SelectOrderTicketField,
            Action::StageOrderTicket,
        ),
        Panel::TransferTicket => ticket_click_action(
            content_row(area, row)?,
            transfer_ticket_rows(state),
            Action::SelectTransferTicketField,
            Action::StageTransferTicket,
        ),
        Panel::FuturesState => ticket_click_action(
            content_row(area, row)?,
            futures_state_ticket_rows(state),
            Action::SelectFuturesStateTicketField,
            Action::StageFuturesStateTicket,
        ),
        Panel::Account
        | Panel::Settings
        | Panel::ProfileRisk
        | Panel::Quote
        | Panel::History
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => None,
    }
}

fn watchlist_click_action(state: &AppState, area: Rect, row: u16) -> Option<Action> {
    let index = content_row(area, row)?;
    (index < state.watchlist.len()).then_some(Action::SelectWatchlistSymbol(index))
}

fn open_order_click_action(state: &AppState, area: Rect, row: u16) -> Option<Action> {
    let open_orders = state.account_snapshot.as_ref()?.open_orders();
    let index = crate::open_order_view::open_order_index_at_content_row(
        &open_orders,
        state.selected_open_order,
        content_row(area, row)?,
    )?;
    Some(Action::SelectOpenOrder(index))
}

fn staged_change_click_action(state: &AppState, area: Rect, row: u16) -> Option<Action> {
    let visible_len = state.staged_change_review_views().len();
    let index = crate::intent_review_view::staged_change_index_at_content_row(
        visible_len,
        content_row(area, row)?,
    )?;
    Some(Action::SelectStagedChange(index))
}

fn ticket_click_action(
    content_row: usize,
    rows: TicketPanelRows,
    select_field: impl FnOnce(usize) -> Action,
    stage: Action,
) -> Option<Action> {
    match rows.click_at(content_row)? {
        TicketPanelClick::Field(index) => Some(select_field(index)),
        TicketPanelClick::ReadyAction => Some(stage),
    }
}

fn order_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.order_ticket_preview();
    TicketPanelRows {
        detail_count: 1,
        field_count: OrderTicketField::COUNT,
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn transfer_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.transfer_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        field_count: TransferTicketField::COUNT,
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn futures_state_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.futures_state_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        field_count: FuturesStateTicketField::MAX_COUNT,
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn content_row(area: Rect, row: u16) -> Option<usize> {
    if row <= area.y || row >= area.bottom().saturating_sub(1) {
        return None;
    }
    Some(row.saturating_sub(area.y).saturating_sub(1) as usize)
}
