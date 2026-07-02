use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::i18n::TuiText;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::panel_action_line_view::styled_panel_action_line;
use crate::state::AppState;
use crate::ticket_panel_view::{
    TicketPanelAction, TicketPanelRow, TicketPanelRows, field_action_line,
};

use super::widgets::panel_block;

pub(super) struct TicketPanel {
    pub panel: Panel,
    pub heading: String,
    pub live_writes_enabled: bool,
    pub effective_mode: String,
    pub detail_lines: Vec<String>,
    pub rows: TicketPanelRows,
    pub fields: Vec<TicketField>,
    pub ready_label: String,
    pub blockers: Vec<String>,
    pub hint: String,
}

pub(super) struct TicketField {
    pub label: String,
    pub value: String,
    pub selected: bool,
}

impl TicketField {
    pub(super) fn new(label: String, value: String, selected: bool) -> Self {
        Self {
            label,
            value,
            selected,
        }
    }
}

pub(super) fn render_ticket_panel(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    ticket: TicketPanel,
    mouse_target: Option<MouseTarget>,
) {
    debug_assert_eq!(ticket.rows.field_count(), ticket.fields.len());
    debug_assert_eq!(ticket.rows.detail_count, ticket.detail_lines.len());
    debug_assert_eq!(ticket.rows.blocker_count, ticket.blockers.len());

    let lines = ticket
        .rows
        .rows()
        .into_iter()
        .map(|row| {
            ticket_line(
                state,
                &ticket,
                row,
                area.width.saturating_sub(2),
                mouse_target,
            )
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(lines.into_iter().map(ListItem::new)).block(panel_block(ticket.panel, state)),
        area,
    );
}

fn ticket_line(
    state: &AppState,
    ticket: &TicketPanel,
    row: TicketPanelRow,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let text = TuiText::new(state.locale);
    match row {
        TicketPanelRow::Header => Line::from(vec![
            Span::styled(
                ticket.heading.clone(),
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} / {}",
                if ticket.live_writes_enabled {
                    text.t("tui-ticket-live-on")
                } else {
                    text.t("tui-ticket-live-off")
                },
                ticket.effective_mode
            )),
        ]),
        TicketPanelRow::Detail(index) => Line::from(ticket.detail_lines[index].clone()),
        TicketPanelRow::Action(index) => ticket_action_line(
            state,
            ticket,
            ticket.rows.actions[index],
            width,
            mouse_target,
        ),
        TicketPanelRow::Field(index) => ticket_field_line(
            state,
            ticket,
            &ticket.fields[index],
            index,
            width,
            mouse_target,
        ),
        TicketPanelRow::ReadyAction => Line::from(Span::styled(
            text.f("tui-ticket-stage", &[("label", &ticket.ready_label)]),
            if ticket_ready_hovered(mouse_target, ticket.panel) {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.accent_style().add_modifier(Modifier::BOLD)
            },
        )),
        TicketPanelRow::Blocker(index) => Line::from(Span::styled(
            text.f(
                "tui-ticket-blocked",
                &[("message", &ticket.blockers[index])],
            ),
            state.theme.warning_style(),
        )),
        TicketPanelRow::Hint => Line::from(ticket.hint.clone()),
    }
}

fn ticket_action_line(
    state: &AppState,
    ticket: &TicketPanel,
    action: TicketPanelAction,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    styled_panel_action_line(
        &action.line(width),
        &state.theme,
        ticket.panel,
        mouse_target,
    )
}

fn ticket_field_line(
    state: &AppState,
    ticket: &TicketPanel,
    field: &TicketField,
    index: usize,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let panel = ticket.panel;
    let marker = if field.selected { ">" } else { " " };
    let hovered = ticket_field_hovered(mouse_target, panel, index);
    let style = if hovered || field.selected {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    let field_text = format!("{marker} {}: {}", field.label, field.value);
    let action_line = field_action_line(
        width,
        index,
        &field_text,
        ticket.rows.field_is_adjustable(index),
    );
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for action in &action_line.actions {
        push_ticket_field_text_span(
            &mut spans,
            action_line.text_before(action.byte_start, cursor),
            style,
        );
        let hovered = mouse_target.is_some_and(|target| {
            target.panel_field_adjust_hovered(panel, index, action.action.direction)
        });
        let action_style = if hovered {
            state.theme.selected_style().add_modifier(Modifier::BOLD)
        } else {
            state.theme.accent_style()
        };
        push_ticket_field_text_span(&mut spans, action_line.action_text(action), action_style);
        cursor = action.byte_end;
    }
    push_ticket_field_text_span(&mut spans, action_line.text_after(cursor), style);
    Line::from(spans)
}

fn push_ticket_field_text_span(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    style: ratatui::style::Style,
) {
    if !text.is_empty() {
        spans.push(Span::styled(text.to_string(), style));
    }
}

fn ticket_field_hovered(mouse_target: Option<MouseTarget>, panel: Panel, index: usize) -> bool {
    mouse_target.is_some_and(|target| target.panel_field_hovered(panel, index))
}

fn ticket_ready_hovered(mouse_target: Option<MouseTarget>, panel: Panel) -> bool {
    mouse_target.is_some_and(|target| target.panel_ready_action_hovered(panel))
}
