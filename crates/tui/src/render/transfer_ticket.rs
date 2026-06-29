use ratatui::Frame;
use ratatui::layout::Rect;

use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::state::AppState;

use super::ticket_panel::{TicketField, TicketPanel, render_ticket_panel};

pub(super) fn render_transfer_ticket(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let preview = state.transfer_ticket_preview();
    let selected = state.transfer_ticket.selected_field_label();
    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::TransferTicket,
            heading: "transfer ticket",
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: Vec::new(),
            actions: crate::transfer_ticket_controls::TRANSFER_TICKET_ACTIONS,
            fields: vec![
                TicketField::new("direction", preview.direction.to_string(), selected),
                TicketField::new("asset", preview.asset.clone(), selected),
                TicketField::new(
                    "amount",
                    preview.amount.as_deref().unwrap_or("-").to_string(),
                    selected,
                ),
            ],
            ready: preview.ready,
            ready_label: "ready",
            blockers: preview.blockers,
            hint: crate::transfer_ticket_controls::transfer_ticket_section_hint(),
        },
        mouse_target,
    );
}
