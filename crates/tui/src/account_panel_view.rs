use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::account::ACCOUNT_READ_PLAN;
use crate::command::ActionId;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::open_order_view::OpenOrderRow;
use crate::panel_action_line_view::{PanelActionLine, PanelActionSpan, styled_panel_action_line};
use crate::profile_snapshot::TradingProfileSnapshot;
use crate::state::AppState;

use crate::render::profile_policy::{ProfilePolicyFormat, profile_policy_lines};
use crate::render::widgets::compact_text;

const VISIBLE_TRANSFER_LIMIT: usize = 4;
const VISIBLE_BALANCE_LIMIT: usize = 4;
const VISIBLE_POSITION_LIMIT: usize = 4;
const ACCOUNT_TRANSFER_LABEL: &str = "[transfer]";
const ACCOUNT_FUTURES_STATE_LABEL: &str = "[futures state]";

pub(crate) struct AccountPanelRow {
    pub line: Line<'static>,
    pub open_order_index: Option<usize>,
    pub actions: Vec<PanelActionSpan>,
}

impl AccountPanelRow {
    fn text(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            open_order_index: None,
            actions: Vec::new(),
        }
    }

    fn line(line: Line<'static>) -> Self {
        Self {
            line,
            open_order_index: None,
            actions: Vec::new(),
        }
    }

    fn open_order(line: Line<'static>, index: usize) -> Self {
        Self {
            line,
            open_order_index: Some(index),
            actions: Vec::new(),
        }
    }

    fn action_line(line: Line<'static>, actions: Vec<PanelActionSpan>) -> Self {
        Self {
            line,
            open_order_index: None,
            actions,
        }
    }
}

pub(crate) fn rows_for_width(
    state: &AppState,
    mouse_target: Option<MouseTarget>,
    content_width: u16,
) -> Vec<AccountPanelRow> {
    let mut rows = profile_rows(state);
    rows.extend(account_action_rows(state, mouse_target, content_width));

    match state.account_snapshot.as_ref() {
        Some(snapshot) => {
            rows.extend(profile_risk_rows(state, &snapshot.profile_config));
            rows.extend(account_read_rows(snapshot));
            rows.extend(open_order_rows(
                state,
                snapshot,
                mouse_target,
                content_width,
            ));
            rows.extend(transfer_history_rows(state, snapshot));
            rows.extend(warning_rows(state, snapshot));
            rows.extend(holdings_rows(state, snapshot));
        }
        None if state.trading_profile.is_some() => rows.push(AccountPanelRow::text(
            "No account snapshot loaded yet. Waiting for signed read.",
        )),
        None => rows.push(AccountPanelRow::text(
            "Start the TUI with --profile <name> to enable signed account reads.",
        )),
    }

    rows
}

fn account_action_rows(
    state: &AppState,
    mouse_target: Option<MouseTarget>,
    width: u16,
) -> Vec<AccountPanelRow> {
    if state.trading_profile.is_none() {
        return Vec::new();
    }

    let action_line = account_action_line(width);
    let actions = action_line.actions.clone();
    vec![AccountPanelRow::action_line(
        styled_panel_action_line(&action_line, &state.theme, Panel::Account, mouse_target),
        actions,
    )]
}

fn account_action_line(width: u16) -> PanelActionLine {
    let mut line = PanelActionLine::new("actions", width);
    line.push_visible_text("  ");
    line.push_visible_action(
        ACCOUNT_TRANSFER_LABEL,
        ActionId::FocusPanel(Panel::TransferTicket),
    );
    line.push_visible_text("  ");
    line.push_visible_action(
        ACCOUNT_FUTURES_STATE_LABEL,
        ActionId::FocusPanel(Panel::FuturesState),
    );
    line
}

pub(crate) fn open_order_index_at_content_row(
    state: &AppState,
    width: u16,
    content_row: usize,
) -> Option<usize> {
    rows_for_width(state, None, width)
        .get(content_row)?
        .open_order_index
}

pub(crate) fn action_at_content_cell(
    state: &AppState,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<PanelActionSpan> {
    rows_for_width(state, None, width)
        .get(content_row)?
        .actions
        .iter()
        .copied()
        .find(|span| (span.start..span.end).contains(&content_column))
}

fn profile_rows(state: &AppState) -> Vec<AccountPanelRow> {
    if let Some(profile) = state.trading_profile.as_deref() {
        vec![AccountPanelRow::line(Line::from(vec![
            Span::styled(
                profile.to_string(),
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(if state.account_loading() {
                " account loading..."
            } else {
                " account"
            }),
        ]))]
    } else {
        vec![AccountPanelRow::text("No trading profile selected.")]
    }
}

fn profile_risk_rows(state: &AppState, profile: &TradingProfileSnapshot) -> Vec<AccountPanelRow> {
    profile_policy_lines(&state.theme, profile, ProfilePolicyFormat::Account)
        .into_iter()
        .map(AccountPanelRow::line)
        .collect()
}

fn account_read_rows(snapshot: &crate::AccountSnapshot) -> Vec<AccountPanelRow> {
    let mut rows = vec![
        AccountPanelRow::text(format!(
            "provider: {}  environment: {}",
            snapshot.provider, snapshot.environment
        )),
        AccountPanelRow::text(format!(
            "signed reads: {} ok / {} warning",
            snapshot.reads.len(),
            snapshot.errors.len()
        )),
    ];
    rows.extend(ACCOUNT_READ_PLAN.into_iter().map(|plan| {
        let request = plan.request();
        let label = if snapshot.read_request(&request).is_some() {
            "ok"
        } else {
            "missing"
        };
        AccountPanelRow::text(format!("{}: {label}", plan.label()))
    }));
    rows
}

struct LimitedAccountSection {
    title: String,
    hidden_label: &'static str,
    visible_limit: usize,
    rows: Vec<String>,
}

impl LimitedAccountSection {
    fn new(
        title: impl Into<String>,
        hidden_label: &'static str,
        visible_limit: usize,
        rows: Vec<String>,
    ) -> Self {
        Self {
            title: title.into(),
            hidden_label,
            visible_limit,
            rows,
        }
    }

    fn into_panel_rows(self, state: &AppState, include_spacer: bool) -> Vec<AccountPanelRow> {
        if self.rows.is_empty() {
            return Vec::new();
        }

        let mut rows = Vec::new();
        if include_spacer {
            rows.push(AccountPanelRow::text(""));
        }
        rows.push(AccountPanelRow::line(Line::from(Span::styled(
            self.title,
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ))));
        let total = self.rows.len();
        rows.extend(
            self.rows
                .into_iter()
                .take(self.visible_limit)
                .map(AccountPanelRow::text),
        );
        push_hidden_row(
            state,
            &mut rows,
            total.saturating_sub(self.visible_limit),
            self.hidden_label,
        );
        rows
    }
}

fn holdings_rows(state: &AppState, snapshot: &crate::AccountSnapshot) -> Vec<AccountPanelRow> {
    let holdings = snapshot.holdings();
    if holdings.is_empty() {
        return Vec::new();
    }

    [
        LimitedAccountSection::new(
            format!("spot balances ({})", holdings.spot_balances.len()),
            "more spot balances",
            VISIBLE_BALANCE_LIMIT,
            holdings
                .spot_balances
                .iter()
                .map(|balance| {
                    format!(
                        "{} free:{} locked:{}",
                        balance.asset,
                        balance.free.as_deref().unwrap_or("-"),
                        balance.locked.as_deref().unwrap_or("-")
                    )
                })
                .collect(),
        ),
        LimitedAccountSection::new(
            format!("USD-M assets ({})", holdings.futures_assets.len()),
            "more USD-M assets",
            VISIBLE_BALANCE_LIMIT,
            holdings
                .futures_assets
                .iter()
                .map(|asset| {
                    format!(
                        "{} wallet:{} availableUsd:{} margin:{} withdraw:{} upnl:{}",
                        asset.asset,
                        asset.wallet_balance.as_deref().unwrap_or("-"),
                        asset.available_balance_usd.as_deref().unwrap_or("-"),
                        asset.margin_balance.as_deref().unwrap_or("-"),
                        asset.max_withdraw_amount.as_deref().unwrap_or("-"),
                        asset.unrealized_profit.as_deref().unwrap_or("-")
                    )
                })
                .collect(),
        ),
        LimitedAccountSection::new(
            format!("USD-M positions ({})", holdings.futures_positions.len()),
            "more USD-M positions",
            VISIBLE_POSITION_LIMIT,
            holdings
                .futures_positions
                .iter()
                .map(|position| {
                    format!(
                        "{} {} amt:{} notional:{} isoMargin:{} isoWallet:{} upnl:{}",
                        position.symbol,
                        position.position_side.as_deref().unwrap_or("-"),
                        position.position_amount,
                        position.notional.as_deref().unwrap_or("-"),
                        position.isolated_margin.as_deref().unwrap_or("-"),
                        position.isolated_wallet.as_deref().unwrap_or("-"),
                        position.unrealized_profit.as_deref().unwrap_or("-")
                    )
                })
                .collect(),
        ),
    ]
    .into_iter()
    .fold(Vec::new(), |mut rows, section| {
        let include_spacer = rows.is_empty();
        rows.extend(section.into_panel_rows(state, include_spacer));
        rows
    })
}

fn open_order_rows(
    state: &AppState,
    snapshot: &crate::AccountSnapshot,
    mouse_target: Option<MouseTarget>,
    content_width: u16,
) -> Vec<AccountPanelRow> {
    let open_orders = snapshot.open_orders();
    let selected = state
        .selected_open_order
        .min(open_orders.len().saturating_sub(1));
    let mut rows = crate::open_order_view::open_order_rows(&open_orders, selected)
        .into_iter()
        .map(|row| match row {
            OpenOrderRow::Order { index, order } => AccountPanelRow::open_order(
                crate::open_order_view::styled_open_order_line(
                    &state.theme,
                    state.selected_open_order,
                    Panel::Account,
                    index,
                    order,
                    mouse_target,
                ),
                index,
            ),
            row => AccountPanelRow::line(non_order_open_order_line(state, row)),
        })
        .collect::<Vec<_>>();

    if !open_orders.is_empty() {
        rows.push(open_order_action_row(state, content_width, mouse_target));
    }
    rows
}

fn open_order_action_row(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> AccountPanelRow {
    let action_line = crate::open_order_view::open_order_action_line(width);
    let actions = action_line.actions.clone();
    AccountPanelRow::action_line(
        styled_panel_action_line(&action_line, &state.theme, Panel::Account, mouse_target),
        actions,
    )
}

fn non_order_open_order_line(state: &AppState, row: OpenOrderRow<'_>) -> Line<'static> {
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
        OpenOrderRow::More { hidden } => Line::from(Span::styled(
            format!("+{hidden} more open orders"),
            state.theme.warning_style(),
        )),
        OpenOrderRow::Order { .. } => unreachable!("orders are rendered with action metadata"),
    }
}

fn transfer_history_rows(
    state: &AppState,
    snapshot: &crate::AccountSnapshot,
) -> Vec<AccountPanelRow> {
    let transfers = snapshot.transfer_history();
    if transfers.is_empty() {
        return Vec::new();
    }

    let mut rows = vec![
        AccountPanelRow::text(""),
        AccountPanelRow::line(Line::from(Span::styled(
            format!("transfer history ({})", transfers.len()),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ))),
    ];
    rows.extend(
        transfers
            .iter()
            .take(VISIBLE_TRANSFER_LIMIT)
            .map(|transfer| {
                AccountPanelRow::text(format!(
                    "{} {} {} {} [{}]",
                    transfer.direction,
                    transfer.amount.as_deref().unwrap_or("-"),
                    transfer.asset.as_deref().unwrap_or("-"),
                    transfer.status.as_deref().unwrap_or("-"),
                    transfer.identifier()
                ))
            }),
    );
    if transfers.len() > VISIBLE_TRANSFER_LIMIT {
        push_hidden_row(
            state,
            &mut rows,
            transfers.len() - VISIBLE_TRANSFER_LIMIT,
            "more transfers",
        );
    }
    rows
}

fn push_hidden_row(state: &AppState, rows: &mut Vec<AccountPanelRow>, hidden: usize, label: &str) {
    if hidden > 0 {
        rows.push(AccountPanelRow::line(Line::from(Span::styled(
            format!("+{hidden} {label}"),
            state.theme.warning_style(),
        ))));
    }
}

fn warning_rows(state: &AppState, snapshot: &crate::AccountSnapshot) -> Vec<AccountPanelRow> {
    snapshot
        .errors
        .iter()
        .take(2)
        .map(|error| {
            AccountPanelRow::line(Line::from(Span::styled(
                format!(
                    "{} warning: {}",
                    error.label,
                    compact_text(&error.error, 96)
                ),
                state.theme.warning_style(),
            )))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{
        Environment, Market, Provider, SignedReadRequest, SignedReadSnapshot,
    };

    #[test]
    fn rows_mark_rendered_open_orders_as_clickable_metadata() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..crate::config::TuiConfig::default()
        });
        state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));

        let clickable = rows_for_width(&state, None, 100)
            .into_iter()
            .filter_map(|row| row.open_order_index)
            .collect::<Vec<_>>();

        assert_eq!(clickable, vec![0, 1]);
    }

    #[test]
    fn rows_mark_account_open_order_cancel_action_metadata() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..crate::config::TuiConfig::default()
        });
        state.account_snapshot = Some(account_snapshot_with_open_orders("mainnet"));

        let action = rows_for_width(&state, None, 100)
            .into_iter()
            .flat_map(|row| row.actions)
            .find(|span| span.action == ActionId::StageSelectedOpenOrderCancel)
            .expect("account open order cancel action");

        assert_eq!(action.action, ActionId::StageSelectedOpenOrderCancel);
        assert_eq!(
            action_at_content_cell(
                &state,
                100,
                open_order_action_row_index(&state, 100),
                action.start
            ),
            Some(action)
        );
        assert_eq!(
            rows_for_width(&state, None, 18)
                .into_iter()
                .flat_map(|row| row.actions)
                .count(),
            0
        );
    }

    #[test]
    fn rows_mark_account_operation_shortcuts_as_action_metadata() {
        let state = AppState::from_config(crate::config::TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..crate::config::TuiConfig::default()
        });

        let row = rows_for_width(&state, None, 100)
            .into_iter()
            .find(|row| {
                row.actions
                    .iter()
                    .any(|span| span.action == ActionId::FocusPanel(Panel::TransferTicket))
            })
            .expect("account action row");
        let actions = row
            .actions
            .iter()
            .map(|span| span.action)
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                ActionId::FocusPanel(Panel::TransferTicket),
                ActionId::FocusPanel(Panel::FuturesState),
            ]
        );
    }

    #[test]
    fn rows_render_orders_before_account_holdings() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..crate::config::TuiConfig::default()
        });
        state.account_snapshot = Some(account_snapshot_with_orders_balances_and_positions(
            "mainnet",
        ));

        let text = rows_text(rows_for_width(&state, None, 120));
        let open_orders_index = text.find("open orders (1)").expect("open orders row");
        let holdings_index = text.find("spot balances (2)").expect("holdings row");

        assert!(open_orders_index < holdings_index);
        assert!(text.contains("spot balances (2)"));
        assert!(text.contains("USDT free:12.5 locked:0"));
        assert!(text.contains("BTC free:0 locked:0.01"));
        assert!(text.contains("USD-M assets (1)"));
        assert!(
            text.contains("USDT wallet:7.25 availableUsd:5 margin:6.75 withdraw:4.5 upnl:-0.5")
        );
        assert!(text.contains("USD-M positions (2)"));
        assert!(
            text.contains("BTCUSDT LONG amt:0.002 notional:130 isoMargin:0 isoWallet:0 upnl:2")
        );
        assert!(
            text.contains(
                "BTCUSDT SHORT amt:-0.001 notional:-65 isoMargin:1.25 isoWallet:10 upnl:-1"
            )
        );
        assert!(!text.contains("ETH free:0 locked:0"));
        assert!(!text.contains("ETHUSDT amt:0"));
    }

    fn open_order_action_row_index(state: &AppState, width: u16) -> usize {
        rows_for_width(state, None, width)
            .into_iter()
            .position(|row| {
                row.actions
                    .iter()
                    .any(|span| span.action == ActionId::StageSelectedOpenOrderCancel)
            })
            .expect("account open order action row")
    }

    fn account_snapshot_with_open_orders(profile: &str) -> crate::AccountSnapshot {
        crate::AccountSnapshot::new(
            profile.to_string(),
            Provider::Binance,
            Environment::Live,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            vec![SignedReadSnapshot::new(
                profile.to_string(),
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::OpenOrders {
                    market: Market::Spot,
                    symbol: None,
                },
                serde_json::json!([
                    {
                        "symbol": "BTCUSDT",
                        "orderId": 1001,
                        "clientOrderId": "spot-order",
                        "side": "BUY",
                        "type": "LIMIT",
                        "origQty": "0.10",
                        "executedQty": "0",
                        "price": "64000"
                    },
                    {
                        "symbol": "ETHUSDT",
                        "orderId": 1002,
                        "clientOrderId": "eth-order",
                        "side": "SELL",
                        "type": "LIMIT",
                        "origQty": "0.20",
                        "executedQty": "0.05",
                        "price": "3200"
                    }
                ]),
            )],
            Vec::new(),
        )
    }

    fn account_snapshot_with_orders_balances_and_positions(
        profile: &str,
    ) -> crate::AccountSnapshot {
        crate::AccountSnapshot::new(
            profile.to_string(),
            Provider::Binance,
            Environment::Live,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            vec![
                SignedReadSnapshot::new(
                    profile.to_string(),
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::OpenOrders {
                        market: Market::Spot,
                        symbol: None,
                    },
                    serde_json::json!([
                        {
                            "symbol": "BTCUSDT",
                            "orderId": 1001,
                            "clientOrderId": "spot-order",
                            "side": "BUY",
                            "type": "LIMIT",
                            "origQty": "0.10",
                            "executedQty": "0",
                            "price": "64000"
                        }
                    ]),
                ),
                SignedReadSnapshot::new(
                    profile.to_string(),
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::SpotBalances,
                    serde_json::json!({
                        "balances": [
                            { "asset": "USDT", "free": "12.5", "locked": "0" },
                            { "asset": "BTC", "free": "0", "locked": "0.01" },
                            { "asset": "ETH", "free": "0", "locked": "0" }
                        ]
                    }),
                ),
                SignedReadSnapshot::new(
                    profile.to_string(),
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::UsdsFuturesPositions,
                    serde_json::json!({
                        "assets": [
                            {
                                "asset": "USDT",
                                "walletBalance": "7.25",
                                "availableBalance": "5",
                                "marginBalance": "6.75",
                                "maxWithdrawAmount": "4.5",
                                "unrealizedProfit": "-0.5"
                            },
                            {
                                "asset": "BNB",
                                "walletBalance": "0",
                                "availableBalance": "0",
                                "marginBalance": "0",
                                "maxWithdrawAmount": "0",
                                "unrealizedProfit": "0"
                            }
                        ],
                        "positions": [
                            {
                                "symbol": "BTCUSDT",
                                "positionSide": "LONG",
                                "positionAmt": "0.002",
                                "notional": "130",
                                "isolatedMargin": "0",
                                "isolatedWallet": "0",
                                "unrealizedProfit": "2"
                            },
                            {
                                "symbol": "BTCUSDT",
                                "positionSide": "SHORT",
                                "positionAmt": "-0.001",
                                "notional": "-65",
                                "isolatedMargin": "1.25",
                                "isolatedWallet": "10",
                                "unrealizedProfit": "-1"
                            },
                            {
                                "symbol": "ETHUSDT",
                                "positionSide": "BOTH",
                                "positionAmt": "0",
                                "notional": "0",
                                "isolatedMargin": "0",
                                "isolatedWallet": "0",
                                "unrealizedProfit": "0"
                            }
                        ]
                    }),
                ),
            ],
            Vec::new(),
        )
    }

    fn rows_text(rows: Vec<AccountPanelRow>) -> String {
        rows.into_iter()
            .map(|row| {
                row.line
                    .spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
