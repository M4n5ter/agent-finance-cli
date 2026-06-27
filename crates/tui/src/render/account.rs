use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use agent_finance_core::{
    FuturesStatePolicy, Market, OrderKind, ProfilePermission, SymbolPolicy, TransferPolicy,
};

use crate::account::ACCOUNT_READ_PLAN;
use crate::futures_state_ticket::FuturesStateTicketPreview;
use crate::model::Panel;
use crate::state::AppState;

use super::widgets::{compact_text, panel_block};

const VISIBLE_OPEN_ORDER_LIMIT: usize = 4;
const VISIBLE_TRANSFER_LIMIT: usize = 4;

pub(super) fn render_account(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let mut lines = profile_lines(state);
    lines.extend(transfer_ticket_lines(state));
    lines.extend(futures_state_ticket_lines(state));

    match state.account_snapshot.as_ref() {
        Some(snapshot) => {
            lines.extend(profile_risk_lines(state, snapshot));
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

fn profile_risk_lines(state: &AppState, snapshot: &crate::AccountSnapshot) -> Vec<Line<'static>> {
    let profile = &snapshot.profile_config;
    let mut lines = vec![
        Line::from(format!(
            "risk: live:{}  daily order cap:{}",
            if profile.risk.allow_live {
                "allowed"
            } else {
                "blocked"
            },
            profile
                .risk
                .max_daily_order_notional_usdt
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_string())
        )),
        Line::from(format!(
            "permissions: declared [{}]  required [{}]",
            permission_list_or_none(&profile.declared_permissions),
            permission_list_or_none(&profile.required_permissions)
        )),
    ];
    if !profile.missing_permissions.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "missing profile permissions: {}",
                permission_list_or_none(&profile.missing_permissions)
            ),
            state.theme.warning_style(),
        )));
    }

    if profile.risk.allowed_symbols.is_empty() {
        lines.push(Line::from(Span::styled(
            "risk.allowed_symbols is empty",
            state.theme.warning_style(),
        )));
    } else {
        lines.push(Line::from(format!(
            "allowed symbols: {}",
            profile
                .risk
                .allowed_symbols
                .iter()
                .take(4)
                .map(|(symbol, policy)| symbol_policy_label(symbol, policy))
                .collect::<Vec<_>>()
                .join("; ")
        )));
        if profile.risk.allowed_symbols.len() > 4 {
            lines.push(Line::from(format!(
                "+{} more risk symbols",
                profile.risk.allowed_symbols.len() - 4
            )));
        }
    }

    if !profile.risk.allowed_transfers.is_empty() {
        lines.push(Line::from(format!(
            "transfers: {}",
            profile
                .risk
                .allowed_transfers
                .iter()
                .take(3)
                .map(transfer_policy_label)
                .map(|line| compact_text(&line, 40))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    if !profile.risk.allowed_futures_state_changes.is_empty() {
        lines.push(Line::from(format!(
            "futures state: {}",
            profile
                .risk
                .allowed_futures_state_changes
                .iter()
                .take(3)
                .map(futures_state_policy_label)
                .map(|line| compact_text(&line, 40))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    lines
}

fn permission_list_or_none(values: &[ProfilePermission]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn symbol_policy_label(symbol: &str, policy: &SymbolPolicy) -> String {
    format!(
        "{} {} {} <= {}",
        symbol,
        market_list_or_none(&policy.markets),
        order_kind_list_or_none(&policy.order_kinds),
        policy.max_order_notional_usdt
    )
}

fn market_list_or_none(values: &[Market]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn order_kind_list_or_none(values: &[OrderKind]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn transfer_policy_label(policy: &TransferPolicy) -> String {
    format!(
        "{} {} <= {}",
        policy.direction, policy.asset, policy.max_amount
    )
}

fn futures_state_policy_label(policy: &FuturesStatePolicy) -> String {
    policy.to_string()
}

fn account_read_lines(snapshot: &crate::AccountSnapshot) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
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

fn transfer_ticket_lines(state: &AppState) -> Vec<Line<'static>> {
    let preview = state.transfer_ticket_preview();
    let selected = state.transfer_ticket.selected_field_label();
    let readiness = if preview.ready {
        Span::styled("ready", state.theme.accent_style())
    } else {
        Span::styled(
            format!(
                "blocked: {}",
                preview
                    .blockers
                    .first()
                    .map(String::as_str)
                    .unwrap_or("not ready")
            ),
            state.theme.warning_style(),
        )
    };
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "transfer ticket",
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} / {}  ",
                if preview.live_writes_enabled {
                    "live:on"
                } else {
                    "live:off"
                },
                preview.effective_mode
            )),
            ticket_field_span(state, "direction", preview.direction.to_string(), selected),
            Span::raw("  "),
            ticket_field_span(state, "asset", preview.asset.clone(), selected),
            Span::raw("  "),
            ticket_field_span(
                state,
                "amount",
                preview.amount.as_deref().unwrap_or("-").to_string(),
                selected,
            ),
        ]),
        Line::from(vec![
            readiness,
            Span::raw("  [/] field  left/right adjust  t stage transfer  c cancel order"),
        ]),
    ];
    lines
}

fn futures_state_ticket_lines(state: &AppState) -> Vec<Line<'static>> {
    let preview = state.futures_state_ticket_preview();
    let selected = state.futures_state_ticket.selected_field_label();
    let readiness = if preview.ready {
        Span::styled("ready", state.theme.accent_style())
    } else {
        Span::styled(
            format!(
                "blocked: {}",
                preview
                    .blockers
                    .first()
                    .map(String::as_str)
                    .unwrap_or("not ready")
            ),
            state.theme.warning_style(),
        )
    };
    vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "futures state ticket",
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} / {}  ",
                if preview.live_writes_enabled {
                    "live:on"
                } else {
                    "live:off"
                },
                preview.effective_mode
            )),
            ticket_field_span(state, "kind", preview.kind.to_string(), selected),
            Span::raw("  "),
            ticket_field_span(state, "scope", preview.scope_label(), selected),
            Span::raw("  "),
            ticket_field_span(state, "value", futures_state_value(&preview), selected),
        ]),
        Line::from(vec![
            readiness,
            Span::raw("  u field  i adjust  f stage state"),
        ]),
    ]
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

fn ticket_field_span(
    state: &AppState,
    label: &'static str,
    value: String,
    selected: &'static str,
) -> Span<'static> {
    let marker = if label == selected { ">" } else { "" };
    let style = if label == selected {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    Span::styled(format!("{marker}{label}: {value}"), style)
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
