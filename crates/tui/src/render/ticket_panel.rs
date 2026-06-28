use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::model::Panel;
use crate::state::AppState;
use crate::ticket_panel_view::{TicketPanelRow, TicketPanelRows};

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
    let rows = TicketPanelRows {
        detail_count: ticket.detail_lines.len(),
        field_count: ticket.fields.len(),
        ready: ticket.ready,
        blocker_count: ticket.blockers.len(),
    };
    let lines = rows
        .rows()
        .into_iter()
        .map(|row| ticket_line(state, &ticket, row))
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(lines.into_iter().map(ListItem::new)).block(panel_block(ticket.panel, state)),
        area,
    );
}

fn ticket_line(state: &AppState, ticket: &TicketPanel, row: TicketPanelRow) -> Line<'static> {
    match row {
        TicketPanelRow::Header => Line::from(vec![
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
        ]),
        TicketPanelRow::Detail(index) => Line::from(ticket.detail_lines[index].clone()),
        TicketPanelRow::Field(index) => {
            Line::from(vec![ticket_field_span(state, &ticket.fields[index])])
        }
        TicketPanelRow::ReadyAction => Line::from(Span::styled(
            format!("[stage] {}", ticket.ready_label),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
        TicketPanelRow::Blocker(index) => Line::from(Span::styled(
            format!("blocked: {}", ticket.blockers[index]),
            state.theme.warning_style(),
        )),
        TicketPanelRow::Hint => Line::from(ticket.hint.clone()),
    }
}

fn ticket_field_span(state: &AppState, field: &TicketField) -> Span<'static> {
    let marker = if field.selected { ">" } else { " " };
    let style = if field.selected {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    Span::styled(format!("{marker} {}: {}", field.label, field.value), style)
}
