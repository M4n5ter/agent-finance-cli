use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::account::OpenOrderSummary;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::open_order_view::OpenOrderRow;
use crate::state::AppState;

use super::panels::panel_row_hovered;
use super::widgets::panel_block;

pub(super) fn render_open_orders(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let mut lines = Vec::new();
    match state.account_snapshot.as_ref() {
        Some(snapshot) => {
            lines.extend(open_order_lines(
                state,
                snapshot,
                Panel::OpenOrders,
                mouse_target,
            ));
            if snapshot.open_orders().is_empty() {
                lines.push(Line::from("No open orders."));
            } else {
                lines.push(open_order_action_line(state, area, mouse_target));
                lines.push(Line::from(
                    crate::open_order_controls::open_order_section_hint(),
                ));
            }
        }
        None if state.trading_profile.is_some() => lines.push(Line::from(
            "No account snapshot loaded yet. Waiting for signed open order reads.",
        )),
        None => lines.push(Line::from(
            "Start the TUI with --profile <name> to load open orders.",
        )),
    }

    let items = lines.into_iter().map(ListItem::new);
    frame.render_widget(
        List::new(items).block(panel_block(Panel::OpenOrders, state)),
        area,
    );
}

pub(super) fn open_order_lines(
    state: &AppState,
    snapshot: &crate::AccountSnapshot,
    panel: Panel,
    mouse_target: Option<MouseTarget>,
) -> Vec<Line<'static>> {
    let open_orders = snapshot.open_orders();
    if open_orders.is_empty() {
        return Vec::new();
    }

    let selected = state
        .selected_open_order
        .min(open_orders.len().saturating_sub(1));
    crate::open_order_view::open_order_rows(&open_orders, selected)
        .into_iter()
        .map(|row| open_order_row_line(state, row, panel, mouse_target))
        .collect()
}

fn open_order_row_line(
    state: &AppState,
    row: OpenOrderRow<'_>,
    panel: Panel,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    match row {
        OpenOrderRow::Spacer => Line::from(""),
        OpenOrderRow::Header { total } => Line::from(Span::styled(
            format!("open orders ({total})"),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
        OpenOrderRow::Earlier { hidden } => Line::from(Span::styled(
            format!("+{hidden} earlier open orders"),
            state.theme.warning_style(),
        )),
        OpenOrderRow::Order { index, order } => {
            open_order_line(state, panel, index, order, mouse_target)
        }
        OpenOrderRow::More { hidden } => Line::from(Span::styled(
            format!("+{hidden} more open orders"),
            state.theme.warning_style(),
        )),
    }
}

pub(crate) fn open_order_line(
    state: &AppState,
    panel: Panel,
    index: usize,
    order: &OpenOrderSummary,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let hovered = panel_row_hovered(mouse_target, panel, index);
    let marker = if index == state.selected_open_order {
        ">"
    } else {
        " "
    };
    let style = if hovered {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else if index == state.selected_open_order {
        state.theme.accent_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    Line::from(Span::styled(
        format!(
            "{marker} {} {} {} {} @ {} [{}]",
            order.market,
            order.side.as_deref().unwrap_or("-"),
            order.remaining_quantity.as_deref().unwrap_or("-"),
            order.symbol,
            order.price.as_deref().unwrap_or("-"),
            order.identifier()
        ),
        style,
    ))
}

fn open_order_action_line(
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let action_line = crate::open_order_view::open_order_action_line(area.width.saturating_sub(2));
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    for action in action_line.actions {
        let start = action.start as usize;
        let end = action.end as usize;
        if cursor < start {
            spans.push(Span::styled(
                action_line.text[cursor..start].to_string(),
                state.theme.text_style(),
            ));
        }
        let hovered = mouse_target
            .is_some_and(|target| target.panel_action_hovered(Panel::OpenOrders, action.action));
        let style = if hovered {
            state.theme.selected_style().add_modifier(Modifier::BOLD)
        } else {
            state.theme.accent_style()
        };
        spans.push(Span::styled(
            action_line.text[start..end].to_string(),
            style,
        ));
        cursor = end;
    }

    if cursor < action_line.text.len() {
        spans.push(Span::styled(
            action_line.text[cursor..].to_string(),
            state.theme.text_style(),
        ));
    }

    Line::from(spans)
}
