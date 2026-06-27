use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::account::ACCOUNT_READ_PLAN;
use crate::model::Panel;
use crate::state::AppState;

use super::widgets::{compact_text, panel_block};

const VISIBLE_OPEN_ORDER_LIMIT: usize = 4;
const VISIBLE_TRANSFER_LIMIT: usize = 4;

pub(super) fn render_account(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let mut lines = profile_lines(state);

    match state.account_snapshot.as_ref() {
        Some(snapshot) => {
            lines.extend(account_read_lines(snapshot));
            lines.extend(open_order_lines(state, snapshot));
            lines.extend(transfer_history_lines(state, snapshot));
            lines.extend(warning_lines(state, snapshot));
        }
        None if state.trading_profile.is_some() => lines.push(Line::from(
            "No account snapshot loaded yet. Waiting for signed read.",
        )),
        None => lines.push(Line::from(
            "Start the TUI with --profile <name> to enable signed account reads.",
        )),
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::Account, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn profile_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(profile) = state.trading_profile.as_deref() {
        lines.push(Line::from(vec![
            Span::styled(
                profile.to_string(),
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(if state.account_loading() {
                " account loading..."
            } else {
                " account"
            }),
        ]));
    } else {
        lines.push(Line::from("No trading profile selected."));
    }
    lines
}

fn account_read_lines(snapshot: &crate::AccountSnapshot) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!(
            "provider: {}  environment: {}",
            snapshot.provider, snapshot.environment
        )),
        Line::from(format!(
            "signed reads: {} ok / {} warning",
            snapshot.reads.len(),
            snapshot.errors.len()
        )),
    ];
    for plan in ACCOUNT_READ_PLAN {
        let request = plan.request();
        let label = if snapshot.read_request(&request).is_some() {
            "ok"
        } else {
            "missing"
        };
        lines.push(Line::from(format!("{}: {label}", plan.label())));
    }
    lines
}

fn open_order_lines(state: &AppState, snapshot: &crate::AccountSnapshot) -> Vec<Line<'static>> {
    let open_orders = snapshot.open_orders();
    if open_orders.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("open orders ({})", open_orders.len()),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
    ];
    let selected = state
        .selected_open_order
        .min(open_orders.len().saturating_sub(1));
    let start = selected
        .saturating_add(1)
        .saturating_sub(VISIBLE_OPEN_ORDER_LIMIT);
    if start > 0 {
        lines.push(Line::from(Span::styled(
            format!("+{start} earlier open orders"),
            state.theme.warning_style(),
        )));
    }
    for (index, order) in open_orders
        .iter()
        .enumerate()
        .skip(start)
        .take(VISIBLE_OPEN_ORDER_LIMIT)
    {
        let marker = if index == state.selected_open_order {
            ">"
        } else {
            " "
        };
        lines.push(Line::from(format!(
            "{marker} {} {} {} {} @ {} [{}]",
            order.market,
            order.side.as_deref().unwrap_or("-"),
            order.remaining_quantity.as_deref().unwrap_or("-"),
            order.symbol,
            order.price.as_deref().unwrap_or("-"),
            order.identifier()
        )));
    }
    let hidden_after = open_orders
        .len()
        .saturating_sub(start.saturating_add(VISIBLE_OPEN_ORDER_LIMIT));
    if hidden_after > 0 {
        lines.push(Line::from(Span::styled(
            format!("+{hidden_after} more open orders"),
            state.theme.warning_style(),
        )));
    }
    lines
}

fn transfer_history_lines(
    state: &AppState,
    snapshot: &crate::AccountSnapshot,
) -> Vec<Line<'static>> {
    let transfers = snapshot.transfer_history();
    if transfers.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("transfer history ({})", transfers.len()),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
    ];
    for transfer in transfers.iter().take(VISIBLE_TRANSFER_LIMIT) {
        lines.push(Line::from(format!(
            "{} {} {} {} [{}]",
            transfer.direction,
            transfer.amount.as_deref().unwrap_or("-"),
            transfer.asset.as_deref().unwrap_or("-"),
            transfer.status.as_deref().unwrap_or("-"),
            transfer.identifier()
        )));
    }
    if transfers.len() > VISIBLE_TRANSFER_LIMIT {
        lines.push(Line::from(Span::styled(
            format!(
                "+{} more transfers",
                transfers.len() - VISIBLE_TRANSFER_LIMIT
            ),
            state.theme.warning_style(),
        )));
    }
    lines
}

fn warning_lines(state: &AppState, snapshot: &crate::AccountSnapshot) -> Vec<Line<'static>> {
    snapshot
        .errors
        .iter()
        .take(2)
        .map(|error| {
            Line::from(Span::styled(
                format!(
                    "{} warning: {}",
                    error.label,
                    compact_text(&error.error, 96)
                ),
                state.theme.warning_style(),
            ))
        })
        .collect()
}
