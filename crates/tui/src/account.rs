use agent_finance_core::{
    Environment, Market, OrderIdentifier, Provider, SignedReadRequest, SignedReadSnapshot,
    SignedReadSnapshotKind, TransferDirection,
};
use rust_decimal::Decimal;
use serde::Serialize;
use std::cmp::Reverse;
use std::str::FromStr;

use crate::account_holdings::AccountHoldingsSummary;
use crate::profile_snapshot::TradingProfileSnapshot;

pub const ACCOUNT_TRANSFER_HISTORY_PAGE_SIZE: usize = 10;

pub const ACCOUNT_READ_PLAN: [AccountReadPlan; 7] = [
    AccountReadPlan::new("permissions", SignedReadRequest::ApiPermissions, true),
    AccountReadPlan::new("spot balances", SignedReadRequest::SpotBalances, false),
    AccountReadPlan::new(
        "USD-M positions",
        SignedReadRequest::UsdsFuturesPositions,
        false,
    ),
    AccountReadPlan::new(
        "spot open orders",
        SignedReadRequest::OpenOrders {
            market: Market::Spot,
            symbol: None,
        },
        false,
    ),
    AccountReadPlan::new(
        "USD-M open orders",
        SignedReadRequest::OpenOrders {
            market: Market::UsdsFutures,
            symbol: None,
        },
        false,
    ),
    AccountReadPlan::new(
        "spot -> USD-M transfers",
        SignedReadRequest::TransferHistory {
            direction: TransferDirection::SpotToUsdsFutures,
            current: 1,
            size: ACCOUNT_TRANSFER_HISTORY_PAGE_SIZE,
        },
        true,
    ),
    AccountReadPlan::new(
        "USD-M -> spot transfers",
        SignedReadRequest::TransferHistory {
            direction: TransferDirection::UsdsFuturesToSpot,
            current: 1,
            size: ACCOUNT_TRANSFER_HISTORY_PAGE_SIZE,
        },
        true,
    ),
];

#[derive(Debug, Clone, PartialEq)]
pub struct AccountReadPlan {
    label: &'static str,
    request: SignedReadRequest,
    live_only: bool,
}

impl AccountReadPlan {
    pub const fn new(label: &'static str, request: SignedReadRequest, live_only: bool) -> Self {
        Self {
            label,
            request,
            live_only,
        }
    }

    pub fn request(&self) -> SignedReadRequest {
        self.request.clone()
    }

    pub const fn kind(&self) -> SignedReadSnapshotKind {
        self.request.snapshot_kind()
    }

    pub const fn label(&self) -> &'static str {
        self.label
    }

    pub const fn live_only(&self) -> bool {
        self.live_only
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountSnapshot {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub profile_config: TradingProfileSnapshot,
    pub reads: Vec<SignedReadSnapshot>,
    pub errors: Vec<AccountReadError>,
}

impl AccountSnapshot {
    pub fn new(
        profile: String,
        provider: Provider,
        environment: Environment,
        profile_config: TradingProfileSnapshot,
        reads: Vec<SignedReadSnapshot>,
        errors: Vec<AccountReadError>,
    ) -> Self {
        Self {
            profile,
            provider,
            environment,
            profile_config,
            reads,
            errors,
        }
    }

    pub fn read(&self, kind: SignedReadSnapshotKind) -> Option<&SignedReadSnapshot> {
        self.reads.iter().find(|read| read.kind == kind)
    }

    pub fn read_request(&self, request: &SignedReadRequest) -> Option<&SignedReadSnapshot> {
        self.reads.iter().find(|read| &read.request == request)
    }

    pub fn open_orders(&self) -> Vec<OpenOrderSummary> {
        self.reads
            .iter()
            .filter_map(|read| match read.request {
                SignedReadRequest::OpenOrders { market, .. } => Some((market, read)),
                _ => None,
            })
            .flat_map(|(market, read)| open_order_payload_items(&read.payload, market))
            .collect()
    }

    pub fn transfer_history(&self) -> Vec<TransferHistorySummary> {
        let mut transfers = self
            .reads
            .iter()
            .filter_map(|read| match read.request {
                SignedReadRequest::TransferHistory { direction, .. } => Some((direction, read)),
                _ => None,
            })
            .flat_map(|(direction, read)| transfer_history_payload_items(&read.payload, direction))
            .collect::<Vec<_>>();
        transfers.sort_by_key(|transfer| Reverse(transfer.timestamp_ms));
        transfers
    }

    pub(crate) fn holdings(&self) -> AccountHoldingsSummary {
        AccountHoldingsSummary::from_reads(&self.reads)
    }

    pub fn has_data(&self) -> bool {
        !self.reads.is_empty()
    }

    pub fn complete(&self) -> bool {
        self.errors.is_empty() && self.reads.len() == ACCOUNT_READ_PLAN.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountReadError {
    pub label: String,
    pub kind: SignedReadSnapshotKind,
    pub request: SignedReadRequest,
    pub error: String,
}

impl AccountReadError {
    pub fn from_plan(plan: &AccountReadPlan, error: impl Into<String>) -> Self {
        Self {
            label: plan.label().to_string(),
            kind: plan.kind(),
            request: plan.request(),
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenOrderSummary {
    pub market: Market,
    pub symbol: String,
    pub order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub side: Option<String>,
    pub order_type: Option<String>,
    pub original_quantity: Option<String>,
    pub executed_quantity: Option<String>,
    pub remaining_quantity: Option<String>,
    pub price: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransferHistorySummary {
    pub direction: TransferDirection,
    pub asset: Option<String>,
    pub amount: Option<String>,
    pub status: Option<String>,
    pub timestamp: Option<String>,
    pub timestamp_ms: Option<u64>,
    pub transfer_id: Option<String>,
    pub client_transfer_id: Option<String>,
}

impl TransferHistorySummary {
    pub fn identifier(&self) -> String {
        self.client_transfer_id
            .clone()
            .or_else(|| self.transfer_id.clone())
            .unwrap_or_else(|| "-".to_string())
    }
}

impl OpenOrderSummary {
    pub fn identifier(&self) -> String {
        self.client_order_id
            .clone()
            .or_else(|| self.order_id.clone())
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn cancel_target(&self) -> Option<OrderIdentifier> {
        if let Some(client_order_id) = self.client_order_id.clone() {
            return Some(OrderIdentifier::ClientOrderId { client_order_id });
        }
        self.order_id
            .clone()
            .map(|order_id| OrderIdentifier::OrderId { order_id })
    }
}

fn open_order_payload_items(
    payload: &serde_json::Value,
    market: Market,
) -> impl Iterator<Item = OpenOrderSummary> + '_ {
    payload
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(move |item| open_order_item(item, market))
}

fn open_order_item(item: &serde_json::Value, market: Market) -> Option<OpenOrderSummary> {
    let symbol = string_field(item, "symbol")?.to_string();
    Some(OpenOrderSummary {
        market,
        symbol,
        order_id: string_field(item, "orderId"),
        client_order_id: string_field(item, "clientOrderId")
            .or_else(|| string_field(item, "origClientOrderId")),
        side: string_field(item, "side"),
        order_type: string_field(item, "type"),
        original_quantity: string_field(item, "origQty"),
        executed_quantity: string_field(item, "executedQty"),
        remaining_quantity: remaining_quantity(item),
        price: string_field(item, "price"),
    })
}

fn transfer_history_payload_items(
    payload: &serde_json::Value,
    direction: TransferDirection,
) -> impl Iterator<Item = TransferHistorySummary> + '_ {
    payload
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(move |item| transfer_history_item(item, direction))
}

fn transfer_history_item(
    item: &serde_json::Value,
    direction: TransferDirection,
) -> Option<TransferHistorySummary> {
    Some(TransferHistorySummary {
        direction,
        asset: string_field(item, "asset"),
        amount: string_field(item, "amount"),
        status: string_field(item, "status"),
        timestamp: string_field(item, "timestamp"),
        timestamp_ms: u64_field(item, "timestamp"),
        transfer_id: string_field(item, "tranId"),
        client_transfer_id: string_field(item, "clientTranId"),
    })
}

fn u64_field(value: &serde_json::Value, key: &str) -> Option<u64> {
    let field = value.get(key)?;
    field
        .as_u64()
        .or_else(|| field.as_i64().and_then(|number| number.try_into().ok()))
        .or_else(|| field.as_str().and_then(|text| text.parse().ok()))
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    let field = value.get(key)?;
    field
        .as_str()
        .map(ToString::to_string)
        .or_else(|| field.as_i64().map(|number| number.to_string()))
        .or_else(|| field.as_u64().map(|number| number.to_string()))
}

fn remaining_quantity(value: &serde_json::Value) -> Option<String> {
    let original = decimal_field(value, "origQty")?;
    let executed = decimal_field(value, "executedQty").unwrap_or(Decimal::ZERO);
    Some((original - executed).normalize().to_string())
}

fn decimal_field(value: &serde_json::Value, key: &str) -> Option<Decimal> {
    string_field(value, key).and_then(|field| Decimal::from_str(&field).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn account_snapshot_extracts_open_orders_from_market_scoped_reads() {
        let snapshot = AccountSnapshot::new(
            "mainnet".to_string(),
            Provider::Binance,
            Environment::Live,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            vec![
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::OpenOrders {
                        market: Market::Spot,
                        symbol: None,
                    },
                    json!([
                        {
                            "symbol": "BTCUSDT",
                            "orderId": 12345,
                            "clientOrderId": "af-spot",
                            "side": "BUY",
                            "type": "LIMIT",
                            "origQty": "0.01",
                            "executedQty": "0.002",
                            "price": "65000"
                        }
                    ]),
                ),
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::OpenOrders {
                        market: Market::UsdsFutures,
                        symbol: None,
                    },
                    json!([
                        {
                            "symbol": "ETHUSDT",
                            "orderId": "67890",
                            "clientOrderId": "af-futures",
                            "side": "SELL",
                            "type": "LIMIT",
                            "origQty": "0.2",
                            "executedQty": "0",
                            "price": "3200"
                        }
                    ]),
                ),
            ],
            Vec::new(),
        );

        let orders = snapshot.open_orders();

        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].market, Market::Spot);
        assert_eq!(orders[0].symbol, "BTCUSDT");
        assert_eq!(orders[0].order_id.as_deref(), Some("12345"));
        assert_eq!(orders[0].identifier(), "af-spot");
        assert_eq!(orders[0].original_quantity.as_deref(), Some("0.01"));
        assert_eq!(orders[0].executed_quantity.as_deref(), Some("0.002"));
        assert_eq!(orders[0].remaining_quantity.as_deref(), Some("0.008"));
        assert_eq!(orders[1].market, Market::UsdsFutures);
        assert_eq!(orders[1].symbol, "ETHUSDT");
        assert_eq!(orders[1].order_id.as_deref(), Some("67890"));
        assert_eq!(orders[1].remaining_quantity.as_deref(), Some("0.2"));
    }

    #[test]
    fn account_snapshot_extracts_transfer_history_rows_by_direction() {
        let snapshot = AccountSnapshot::new(
            "mainnet".to_string(),
            Provider::Binance,
            Environment::Live,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            vec![
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::transfer_history(
                        TransferDirection::SpotToUsdsFutures,
                        1,
                        10,
                    ),
                    json!({
                        "total": 1,
                        "rows": [
                            {
                                "asset": "USDT",
                                "amount": "25.5",
                                "type": "MAIN_UMFUTURE",
                                "status": "CONFIRMED",
                                "timestamp": 1720000000000_u64,
                                "tranId": 98765,
                                "clientTranId": "af-transfer-1"
                            }
                        ]
                    }),
                ),
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::transfer_history(
                        TransferDirection::UsdsFuturesToSpot,
                        1,
                        10,
                    ),
                    json!({
                        "rows": [
                            {
                                "asset": "USDC",
                                "amount": "3",
                                "status": "CONFIRMED",
                                "tranId": "98766"
                            }
                        ]
                    }),
                ),
            ],
            Vec::new(),
        );

        let transfers = snapshot.transfer_history();

        assert_eq!(transfers.len(), 2);
        assert_eq!(transfers[0].direction, TransferDirection::SpotToUsdsFutures);
        assert_eq!(transfers[0].amount.as_deref(), Some("25.5"));
        assert_eq!(transfers[0].asset.as_deref(), Some("USDT"));
        assert_eq!(transfers[0].status.as_deref(), Some("CONFIRMED"));
        assert_eq!(transfers[0].identifier(), "af-transfer-1");
        assert_eq!(transfers[1].direction, TransferDirection::UsdsFuturesToSpot);
        assert_eq!(transfers[1].identifier(), "98766");
    }

    #[test]
    fn transfer_history_is_sorted_by_timestamp_across_directions() {
        let snapshot = AccountSnapshot::new(
            "mainnet".to_string(),
            Provider::Binance,
            Environment::Live,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            vec![
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::transfer_history(
                        TransferDirection::SpotToUsdsFutures,
                        1,
                        10,
                    ),
                    json!({
                        "rows": [
                            {
                                "asset": "USDT",
                                "amount": "1",
                                "status": "CONFIRMED",
                                "timestamp": 1000,
                                "clientTranId": "old-1"
                            },
                            {
                                "asset": "USDT",
                                "amount": "2",
                                "status": "CONFIRMED",
                                "timestamp": 2000,
                                "clientTranId": "old-2"
                            },
                            {
                                "asset": "USDT",
                                "amount": "3",
                                "status": "CONFIRMED",
                                "timestamp": 3000,
                                "clientTranId": "old-3"
                            },
                            {
                                "asset": "USDT",
                                "amount": "4",
                                "status": "CONFIRMED",
                                "timestamp": 4000,
                                "clientTranId": "old-4"
                            }
                        ]
                    }),
                ),
                SignedReadSnapshot::new(
                    "mainnet",
                    Provider::Binance,
                    Environment::Live,
                    SignedReadRequest::transfer_history(
                        TransferDirection::UsdsFuturesToSpot,
                        1,
                        10,
                    ),
                    json!({
                        "rows": [
                            {
                                "asset": "USDT",
                                "amount": "5",
                                "status": "CONFIRMED",
                                "timestamp": 9000,
                                "clientTranId": "newest"
                            }
                        ]
                    }),
                ),
            ],
            Vec::new(),
        );

        let transfers = snapshot.transfer_history();

        assert_eq!(transfers[0].identifier(), "newest");
        assert_eq!(transfers[1].identifier(), "old-4");
        assert_eq!(transfers[4].identifier(), "old-1");
    }
}
