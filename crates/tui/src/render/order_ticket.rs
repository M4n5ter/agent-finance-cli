use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::Panel;
use crate::state::AppState;

use super::ticket_panel::{TicketField, TicketPanel, render_ticket_panel};

pub(super) fn render_order_ticket(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let ticket = &state.order_ticket;
    let preview = state.order_ticket_preview();
    let selected = ticket.selected_field_label();

    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::OrderTicket,
            heading: "staged order",
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: vec![format!(
                "symbol: {}  profile: {}",
                preview.symbol.as_deref().unwrap_or("-"),
                preview.profile.as_deref().unwrap_or("-")
            )],
            fields: vec![
                TicketField::new("market", ticket.market().to_string(), selected),
                TicketField::new("side", ticket.side().to_string(), selected),
                TicketField::new("kind", ticket.kind().to_string(), selected),
                TicketField::new(
                    "quantity",
                    preview.quantity.as_deref().unwrap_or("-").to_string(),
                    selected,
                ),
                TicketField::new(
                    "price",
                    preview.price.as_deref().unwrap_or("-").to_string(),
                    selected,
                ),
                TicketField::new(
                    "time in force",
                    ticket.time_in_force().to_string(),
                    selected,
                ),
                TicketField::new("reduce only", ticket.reduce_only().to_string(), selected),
            ],
            ready: preview.ready,
            ready_label: "ready for intent review",
            blockers: preview.blockers,
            hint: crate::order_ticket_controls::order_ticket_panel_hint(),
        },
    );
}
