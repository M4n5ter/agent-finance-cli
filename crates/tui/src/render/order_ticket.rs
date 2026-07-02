use ratatui::Frame;
use ratatui::layout::Rect;

use crate::i18n::TuiText;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::state::AppState;

use super::ticket_panel::{TicketField, TicketPanel, render_ticket_panel};

pub(super) fn render_order_ticket(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let ticket = &state.order_ticket;
    let preview = state.order_ticket_preview();
    let selected = ticket.selected_field_label();
    let text = TuiText::new(state.locale);

    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::OrderTicket,
            heading: text.t("tui-ticket-order-heading"),
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: crate::ticket_panel_view::order_ticket_detail_lines(
                &preview,
                state.locale,
            ),
            rows: crate::ticket_panel_view::order_ticket_rows(state),
            fields: vec![
                TicketField::new(
                    text.t("tui-ticket-field-market"),
                    ticket.market().to_string(),
                    selected == "market",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-side"),
                    ticket.side().to_string(),
                    selected == "side",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-kind"),
                    ticket.kind().to_string(),
                    selected == "kind",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-quantity"),
                    preview.quantity.as_deref().unwrap_or("-").to_string(),
                    selected == "quantity",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-price"),
                    preview.price.as_deref().unwrap_or("-").to_string(),
                    selected == "price",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-time-in-force"),
                    ticket.time_in_force().to_string(),
                    selected == "time in force",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-reduce-only"),
                    ticket.reduce_only().to_string(),
                    selected == "reduce only",
                ),
            ],
            ready_label: text.t("tui-ticket-order-ready"),
            blockers: preview.blockers,
            hint: crate::order_ticket_controls::order_ticket_panel_hint(),
        },
        mouse_target,
    );
}
