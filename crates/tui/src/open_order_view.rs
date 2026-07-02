use std::ops::Range;

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::account::OpenOrderSummary;
use crate::command::ActionId;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::panel_action_line_view::{PanelActionLine, PanelActionSpan, styled_panel_action_line};
use crate::theme::ThemeConfig;

pub(crate) const VISIBLE_OPEN_ORDER_LIMIT: usize = 4;

const STAGE_CANCEL_LABEL: &str = "[stage cancel]";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OpenOrderRow<'a> {
    Spacer,
    Header {
        total: usize,
    },
    Earlier {
        hidden: usize,
    },
    Order {
        index: usize,
        order: &'a OpenOrderSummary,
    },
    More {
        hidden: usize,
    },
}

pub(crate) fn visible_open_order_window(len: usize, selected: usize) -> Range<usize> {
    if len == 0 {
        return 0..0;
    }
    let selected = selected.min(len - 1);
    let start = selected
        .saturating_add(1)
        .saturating_sub(VISIBLE_OPEN_ORDER_LIMIT);
    start..(start + VISIBLE_OPEN_ORDER_LIMIT).min(len)
}

pub(crate) fn open_order_rows(
    open_orders: &[OpenOrderSummary],
    selected: usize,
) -> Vec<OpenOrderRow<'_>> {
    if open_orders.is_empty() {
        return Vec::new();
    }

    let visible = visible_open_order_window(open_orders.len(), selected);
    let mut rows = vec![
        OpenOrderRow::Spacer,
        OpenOrderRow::Header {
            total: open_orders.len(),
        },
    ];
    if visible.start > 0 {
        rows.push(OpenOrderRow::Earlier {
            hidden: visible.start,
        });
    }
    rows.extend(
        open_orders
            .iter()
            .enumerate()
            .skip(visible.start)
            .take(visible.len())
            .map(|(index, order)| OpenOrderRow::Order { index, order }),
    );
    let hidden_after = open_orders.len().saturating_sub(visible.end);
    if hidden_after > 0 {
        rows.push(OpenOrderRow::More {
            hidden: hidden_after,
        });
    }
    rows
}

pub(crate) fn open_order_index_at_content_row(
    open_orders: &[OpenOrderSummary],
    selected: usize,
    content_row: usize,
) -> Option<usize> {
    match open_order_rows(open_orders, selected).get(content_row)? {
        OpenOrderRow::Order { index, .. } => Some(*index),
        _ => None,
    }
}

pub(crate) fn styled_open_order_line(
    theme: &ThemeConfig,
    selected_open_order: usize,
    panel: Panel,
    index: usize,
    order: &OpenOrderSummary,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let hovered = mouse_target.is_some_and(|target| target.panel_row_hovered(panel, index));
    let marker = if index == selected_open_order {
        ">"
    } else {
        " "
    };
    let style = if hovered {
        theme.selected_style().add_modifier(Modifier::BOLD)
    } else if index == selected_open_order {
        theme.accent_style().add_modifier(Modifier::BOLD)
    } else {
        theme.text_style()
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

pub(crate) fn open_order_action_line(width: u16) -> PanelActionLine {
    let mut line = PanelActionLine::new("selected order", width);
    line.push_visible_text("  ");
    line.push_visible_action(STAGE_CANCEL_LABEL, ActionId::StageSelectedOpenOrderCancel);
    line
}

pub(crate) fn styled_open_order_action_line(
    theme: &ThemeConfig,
    panel: Panel,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    styled_panel_action_line(&open_order_action_line(width), theme, panel, mouse_target)
}

pub(crate) fn open_order_action_at_content_cell(
    open_orders: &[OpenOrderSummary],
    selected: usize,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<PanelActionSpan> {
    if open_orders.is_empty() || content_row != open_order_rows(open_orders, selected).len() {
        return None;
    }
    open_order_action_line(width).action_at(content_column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::Market;

    fn order(symbol: &str) -> OpenOrderSummary {
        OpenOrderSummary {
            market: Market::Spot,
            symbol: symbol.to_string(),
            order_id: None,
            client_order_id: None,
            side: None,
            order_type: None,
            original_quantity: None,
            executed_quantity: None,
            remaining_quantity: None,
            price: None,
        }
    }

    #[test]
    fn visible_window_keeps_selected_open_order_in_view() {
        assert_eq!(visible_open_order_window(0, 0), 0..0);
        assert_eq!(visible_open_order_window(2, 0), 0..2);
        assert_eq!(visible_open_order_window(8, 0), 0..4);
        assert_eq!(visible_open_order_window(8, 4), 1..5);
        assert_eq!(visible_open_order_window(8, 7), 4..8);
    }

    #[test]
    fn rows_mark_only_visible_orders_as_selectable() {
        let open_orders = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "ADAUSDT"]
            .into_iter()
            .map(order)
            .collect::<Vec<_>>();

        let selected = 4;
        let rows = open_order_rows(&open_orders, selected);
        let selectable = rows
            .iter()
            .enumerate()
            .filter_map(|(row, item)| match item {
                OpenOrderRow::Order { index, order } => Some((row, *index, order.symbol.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            selectable,
            vec![
                (3, 1, "ETHUSDT"),
                (4, 2, "SOLUSDT"),
                (5, 3, "BNBUSDT"),
                (6, 4, "ADAUSDT"),
            ]
        );
        assert_eq!(
            open_order_index_at_content_row(&open_orders, selected, 2),
            None
        );
        assert_eq!(
            open_order_index_at_content_row(&open_orders, selected, 3),
            Some(1)
        );
    }

    #[test]
    fn action_line_maps_visible_cancel_to_action() {
        let line = open_order_action_line(80);
        let span = line
            .actions
            .iter()
            .find(|span| span.action == ActionId::StageSelectedOpenOrderCancel)
            .expect("cancel action");
        assert_eq!(line.action_text(span), STAGE_CANCEL_LABEL);

        let open_orders = ["BTCUSDT", "ETHUSDT"]
            .into_iter()
            .map(order)
            .collect::<Vec<_>>();
        let row = open_order_rows(&open_orders, 0).len();
        assert_eq!(
            open_order_action_at_content_cell(&open_orders, 0, 80, row, span.start),
            Some(span.clone())
        );
        assert_eq!(
            open_order_action_at_content_cell(&open_orders, 0, 80, row - 1, span.start),
            None
        );
    }

    #[test]
    fn narrow_action_line_does_not_expose_hidden_cancel() {
        let open_orders = [order("BTCUSDT")];
        let row = open_order_rows(&open_orders, 0).len();

        assert!(open_order_action_line(18).actions.is_empty());
        assert_eq!(
            open_order_action_at_content_cell(&open_orders, 0, 18, row, 17),
            None
        );
    }
}
