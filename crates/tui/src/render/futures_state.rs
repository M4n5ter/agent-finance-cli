use ratatui::Frame;
use ratatui::layout::Rect;

use crate::futures_state_ticket::FuturesStateTicketPreview;
use crate::i18n::TuiText;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::state::AppState;

use super::ticket_panel::{TicketField, TicketPanel, render_ticket_panel};

pub(super) fn render_futures_state(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let preview = state.futures_state_ticket_preview();
    let selected = state.futures_state_ticket.selected_field_label();
    let text = TuiText::new(state.locale);
    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::FuturesState,
            heading: text.t("tui-ticket-futures-state-heading"),
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: Vec::new(),
            rows: crate::ticket_panel_view::futures_state_ticket_rows(state),
            fields: vec![
                TicketField::new(
                    text.t("tui-ticket-field-kind"),
                    preview.kind.to_string(),
                    selected == "kind",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-scope"),
                    preview.scope_label(),
                    selected == "scope",
                ),
                TicketField::new(
                    text.t("tui-ticket-field-value"),
                    futures_state_value(&preview),
                    selected == "value",
                ),
            ],
            ready_label: text.t("tui-ticket-ready"),
            blockers: preview.blockers,
            hint: crate::futures_state_controls::futures_state_section_hint(),
        },
        mouse_target,
    );
}

fn futures_state_value(preview: &FuturesStateTicketPreview) -> String {
    match preview.kind {
        agent_finance_core::FuturesStateChangeKind::Leverage => preview
            .leverage
            .map(|leverage| leverage.to_string())
            .unwrap_or_else(|| "-".to_string()),
        agent_finance_core::FuturesStateChangeKind::MarginType => preview
            .margin_type
            .map(|margin_type| margin_type.to_string())
            .unwrap_or_else(|| "-".to_string()),
        agent_finance_core::FuturesStateChangeKind::PositionMode => preview
            .position_mode
            .map(|mode| mode.to_string())
            .unwrap_or_else(|| "-".to_string()),
    }
}
