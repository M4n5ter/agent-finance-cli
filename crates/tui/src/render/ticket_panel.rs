use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::model::Panel;
use crate::state::AppState;

use super::widgets::panel_block;

pub(super) struct TicketPanel {
    pub panel: Panel,
    pub heading: &'static str,
    pub live_writes_enabled: bool,
    pub effective_mode: String,
    pub detail_lines: Vec<String>,
    pub fields: Vec<TicketField>,
    pub ready: bool,
    pub ready_label: &'static str,
    pub blockers: Vec<String>,
    pub hint: String,
}

pub(super) struct TicketField {
    pub label: &'static str,
    pub value: String,
    pub selected: bool,
}

impl TicketField {
    pub(super) fn new(label: &'static str, value: String, selected_label: &'static str) -> Self {
        Self {
            label,
            value,
            selected: label == selected_label,
        }
    }
}

pub(super) fn render_ticket_panel(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    ticket: TicketPanel,
) {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            ticket.heading,
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {} / {}",
            if ticket.live_writes_enabled {
                "live:on"
            } else {
                "live:off"
            },
            ticket.effective_mode
        )),
    ])];
    lines.extend(ticket.detail_lines.iter().cloned().map(Line::from));
    let readiness = readiness_lines(state, &ticket);
    lines.extend(
        ticket
            .fields
            .into_iter()
            .map(|field| Line::from(vec![ticket_field_span(state, field)])),
    );
    lines.extend(readiness);
    lines.push(Line::from(ticket.hint));

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(ticket.panel, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn readiness_lines(state: &AppState, ticket: &TicketPanel) -> Vec<Line<'static>> {
    if ticket.ready {
        return vec![Line::from(Span::styled(
            ticket.ready_label,
            state.theme.accent_style(),
        ))];
    }

    ticket
        .blockers
        .iter()
        .take(3)
        .map(|blocker| {
            Line::from(Span::styled(
                format!("blocked: {blocker}"),
                state.theme.warning_style(),
            ))
        })
        .collect()
}

fn ticket_field_span(state: &AppState, field: TicketField) -> Span<'static> {
    let marker = if field.selected { ">" } else { " " };
    let style = if field.selected {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    Span::styled(format!("{marker} {}: {}", field.label, field.value), style)
}
