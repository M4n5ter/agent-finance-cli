use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use agent_finance_core::{
    FuturesStatePolicy, Market, OrderKind, ProfilePermission, SymbolPolicy, TransferPolicy,
};

use crate::profile_snapshot::TradingProfileSnapshot;
use crate::theme::ThemeConfig;

use super::widgets::compact_text;

pub(super) enum ProfilePolicyFormat {
    Account,
    ProfileRisk,
}

pub(super) fn profile_policy_lines(
    theme: &ThemeConfig,
    profile: &TradingProfileSnapshot,
    format: ProfilePolicyFormat,
) -> Vec<Line<'static>> {
    match format {
        ProfilePolicyFormat::Account => account_policy_lines(theme, profile),
        ProfilePolicyFormat::ProfileRisk => profile_risk_policy_lines(theme, profile),
    }
}

fn account_policy_lines(
    theme: &ThemeConfig,
    profile: &TradingProfileSnapshot,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!(
            "risk: live:{}  daily order cap:{}",
            if profile.risk.allow_live {
                "allowed"
            } else {
                "blocked"
            },
            daily_order_cap(profile)
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
            theme.warning_style(),
        )));
    }

    if profile.risk.allowed_symbols.is_empty() {
        lines.push(Line::from(Span::styled(
            "risk.allowed_symbols is empty",
            theme.warning_style(),
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

fn profile_risk_policy_lines(
    theme: &ThemeConfig,
    profile: &TradingProfileSnapshot,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!(
            "declared permissions: {}",
            permission_list_or_none(&profile.declared_permissions)
        )),
        Line::from(format!(
            "required permissions: {}",
            permission_list_or_none(&profile.required_permissions)
        )),
        Line::from(format!(
            "missing permissions: {}",
            permission_list_or_none(&profile.missing_permissions)
        )),
        Line::from(format!(
            "risk.allow_live: {}",
            if profile.risk.allow_live {
                "true"
            } else {
                "false"
            }
        )),
        Line::from(format!("daily order cap: {}", daily_order_cap(profile))),
    ];

    if profile.risk.allowed_symbols.is_empty() {
        lines.push(Line::from(Span::styled(
            "allowed symbols: none",
            theme.warning_style(),
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
                "+{} more symbols",
                profile.risk.allowed_symbols.len() - 4
            )));
        }
    }

    lines.push(Line::from(format!(
        "allowed transfers: {}",
        list_or_none(
            profile
                .risk
                .allowed_transfers
                .iter()
                .take(3)
                .map(transfer_policy_label)
        )
    )));
    lines.push(Line::from(format!(
        "allowed futures state: {}",
        list_or_none(
            profile
                .risk
                .allowed_futures_state_changes
                .iter()
                .take(3)
                .map(futures_state_policy_label)
        )
    )));
    lines
}

pub(super) fn profile_policy_heading(theme: &ThemeConfig) -> Line<'static> {
    Line::from(Span::styled(
        "profile and risk policy",
        theme.accent_style().add_modifier(Modifier::BOLD),
    ))
}

fn daily_order_cap(profile: &TradingProfileSnapshot) -> String {
    profile
        .risk
        .max_daily_order_notional_usdt
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".to_string())
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

fn list_or_none(values: impl Iterator<Item = String>) -> String {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join("; ")
    }
}
