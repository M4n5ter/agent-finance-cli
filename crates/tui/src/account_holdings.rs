use agent_finance_core::{SignedReadSnapshot, SignedReadSnapshotKind};
use rust_decimal::Decimal;
use serde::Serialize;
use std::str::FromStr;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub(crate) struct AccountHoldingsSummary {
    pub spot_balances: Vec<SpotBalanceSummary>,
    pub futures_assets: Vec<FuturesAssetSummary>,
    pub futures_positions: Vec<FuturesPositionSummary>,
}

impl AccountHoldingsSummary {
    pub fn from_reads(reads: &[SignedReadSnapshot]) -> Self {
        let spot_balances = reads
            .iter()
            .filter(|read| read.kind == SignedReadSnapshotKind::SpotBalances)
            .flat_map(|read| spot_balance_payload_items(&read.payload))
            .collect();
        let futures_snapshot = reads
            .iter()
            .find(|read| read.kind == SignedReadSnapshotKind::UsdsFuturesPositions);
        let futures_assets = futures_snapshot
            .into_iter()
            .flat_map(|read| futures_asset_payload_items(&read.payload))
            .collect();
        let futures_positions = futures_snapshot
            .into_iter()
            .flat_map(|read| futures_position_payload_items(&read.payload))
            .collect();

        Self {
            spot_balances,
            futures_assets,
            futures_positions,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.spot_balances.is_empty()
            && self.futures_assets.is_empty()
            && self.futures_positions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct SpotBalanceSummary {
    pub asset: String,
    pub free: Option<String>,
    pub locked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct FuturesAssetSummary {
    pub asset: String,
    pub wallet_balance: Option<String>,
    pub available_balance_usd: Option<String>,
    pub margin_balance: Option<String>,
    pub max_withdraw_amount: Option<String>,
    pub unrealized_profit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct FuturesPositionSummary {
    pub symbol: String,
    pub position_side: Option<String>,
    pub position_amount: String,
    pub notional: Option<String>,
    pub isolated_margin: Option<String>,
    pub isolated_wallet: Option<String>,
    pub unrealized_profit: Option<String>,
}

fn spot_balance_payload_items(
    payload: &serde_json::Value,
) -> impl Iterator<Item = SpotBalanceSummary> + '_ {
    payload
        .get("balances")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(spot_balance_item)
        .filter(has_non_zero_spot_balance)
}

fn spot_balance_item(item: &serde_json::Value) -> Option<SpotBalanceSummary> {
    Some(SpotBalanceSummary {
        asset: string_field(item, "asset")?,
        free: string_field(item, "free"),
        locked: string_field(item, "locked"),
    })
}

fn has_non_zero_spot_balance(balance: &SpotBalanceSummary) -> bool {
    non_zero_decimal(balance.free.as_deref()) || non_zero_decimal(balance.locked.as_deref())
}

fn futures_asset_payload_items(
    payload: &serde_json::Value,
) -> impl Iterator<Item = FuturesAssetSummary> + '_ {
    payload
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(futures_asset_item)
        .filter(has_non_zero_futures_asset)
}

fn futures_asset_item(item: &serde_json::Value) -> Option<FuturesAssetSummary> {
    Some(FuturesAssetSummary {
        asset: string_field(item, "asset")?,
        wallet_balance: string_field(item, "walletBalance"),
        available_balance_usd: string_field(item, "availableBalance"),
        margin_balance: string_field(item, "marginBalance"),
        max_withdraw_amount: string_field(item, "maxWithdrawAmount"),
        unrealized_profit: string_field(item, "unrealizedProfit"),
    })
}

fn has_non_zero_futures_asset(asset: &FuturesAssetSummary) -> bool {
    non_zero_decimal(asset.wallet_balance.as_deref())
        || non_zero_decimal(asset.available_balance_usd.as_deref())
        || non_zero_decimal(asset.margin_balance.as_deref())
        || non_zero_decimal(asset.max_withdraw_amount.as_deref())
        || non_zero_decimal(asset.unrealized_profit.as_deref())
}

fn futures_position_payload_items(
    payload: &serde_json::Value,
) -> impl Iterator<Item = FuturesPositionSummary> + '_ {
    payload
        .get("positions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(futures_position_item)
        .filter(|position| non_zero_decimal(Some(&position.position_amount)))
}

fn futures_position_item(item: &serde_json::Value) -> Option<FuturesPositionSummary> {
    Some(FuturesPositionSummary {
        symbol: string_field(item, "symbol")?,
        position_side: string_field(item, "positionSide"),
        position_amount: string_field(item, "positionAmt")?,
        notional: string_field(item, "notional"),
        isolated_margin: string_field(item, "isolatedMargin"),
        isolated_wallet: string_field(item, "isolatedWallet"),
        unrealized_profit: string_field(item, "unrealizedProfit"),
    })
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    let field = value.get(key)?;
    field
        .as_str()
        .map(ToString::to_string)
        .or_else(|| field.as_i64().map(|number| number.to_string()))
        .or_else(|| field.as_u64().map(|number| number.to_string()))
        .or_else(|| field.as_bool().map(|value| value.to_string()))
}

fn non_zero_decimal(value: Option<&str>) -> bool {
    value
        .and_then(|value| Decimal::from_str(value).ok())
        .is_some_and(|value| !value.is_zero())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{Environment, Provider, SignedReadRequest};
    use serde_json::json;

    #[test]
    fn holdings_summary_extracts_non_zero_balances_and_hedged_positions() {
        let summary = AccountHoldingsSummary::from_reads(&[
            SignedReadSnapshot::new(
                "mainnet",
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::SpotBalances,
                json!({
                    "balances": [
                        { "asset": "USDT", "free": "12.5", "locked": "0" },
                        { "asset": "BTC", "free": "0", "locked": "0.01" },
                        { "asset": "ETH", "free": "0", "locked": "0" }
                    ]
                }),
            ),
            SignedReadSnapshot::new(
                "mainnet",
                Provider::Binance,
                Environment::Live,
                SignedReadRequest::UsdsFuturesPositions,
                json!({
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
        ]);

        assert_eq!(summary.spot_balances.len(), 2);
        assert_eq!(summary.spot_balances[0].asset, "USDT");
        assert_eq!(summary.spot_balances[1].locked.as_deref(), Some("0.01"));

        assert_eq!(summary.futures_assets.len(), 1);
        assert_eq!(summary.futures_assets[0].asset, "USDT");
        assert_eq!(
            summary.futures_assets[0].available_balance_usd.as_deref(),
            Some("5")
        );
        assert_eq!(
            summary.futures_assets[0].max_withdraw_amount.as_deref(),
            Some("4.5")
        );

        assert_eq!(summary.futures_positions.len(), 2);
        assert_eq!(summary.futures_positions[0].symbol, "BTCUSDT");
        assert_eq!(
            summary.futures_positions[0].position_side.as_deref(),
            Some("LONG")
        );
        assert_eq!(summary.futures_positions[1].position_amount, "-0.001");
        assert_eq!(
            summary.futures_positions[1].isolated_margin.as_deref(),
            Some("1.25")
        );
    }
}
