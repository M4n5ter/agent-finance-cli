use ratatui::Frame;
use ratatui::layout::Rect;

use crate::futures_state_ticket::FuturesStateTicketPreview;
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
    render_ticket_panel(
        frame,
        state,
        area,
        TicketPanel {
            panel: Panel::FuturesState,
            heading: "futures state ticket",
            live_writes_enabled: preview.live_writes_enabled,
            effective_mode: preview.effective_mode.to_string(),
            detail_lines: Vec::new(),
            actions: crate::futures_state_controls::FUTURES_STATE_ACTIONS,
            fields: vec![
                TicketField::new("kind", preview.kind.to_string(), selected),
                TicketField::new("scope", preview.scope_label(), selected),
                TicketField::new("value", futures_state_value(&preview), selected),
            ],
            ready: preview.ready,
            ready_label: "ready",
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
