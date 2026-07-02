use ratatui::Frame;
use ratatui::layout::Rect;

use crate::i18n::TuiText;
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
    let text = TuiText::new(state.locale);
    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::TransferTicket,
            heading: text.t("tui-ticket-transfer-heading"),
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: Vec::new(),
            rows: crate::ticket_panel_view::transfer_ticket_rows(state),
            fields: vec![
                TicketField::new(
                    text.t("tui-ticket-field-direction"),
                    preview.direction.to_string(),
                    selected == "direction",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-asset"),
                    preview.asset.clone(),
                    selected == "asset",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-amount"),
                    preview.amount.as_deref().unwrap_or("-").to_string(),
                    selected == "amount",
                ),
            ],
            ready_label: text.t("tui-ticket-ready"),
            blockers: preview.blockers,
            hint: crate::transfer_ticket_controls::transfer_ticket_section_hint(),
        },
        mouse_target,
    );
}
