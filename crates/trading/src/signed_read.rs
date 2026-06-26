use anyhow::{Result, anyhow};
use serde_json::Value;

trait SignedReadExecutor {
    async fn execute_signed_read(
        &self,
        request: agent_finance_core::SignedReadRequest,
    ) -> Result<Value>;
}

pub(crate) async fn run_signed_read(
    profile: &agent_finance_core::Profile,
    request: agent_finance_core::SignedReadRequest,
    timeout_seconds: u64,
) -> Result<agent_finance_core::SignedReadSnapshot> {
    let request = request.normalized();
    validate_signed_read_request(profile, &request)?;
    let client = crate::write::binance_client(profile, timeout_seconds)?;
    signed_read_snapshot(profile, &client, request).await
}

impl SignedReadExecutor for agent_finance_binance::BinanceClient {
    async fn execute_signed_read(
        &self,
        request: agent_finance_core::SignedReadRequest,
    ) -> Result<Value> {
        match request {
            agent_finance_core::SignedReadRequest::ApiPermissions => {
                self.account_permissions().await
            }
            agent_finance_core::SignedReadRequest::SpotBalances => self.spot_account().await,
            agent_finance_core::SignedReadRequest::UsdsFuturesPositions => {
                self.futures_account().await
            }
            agent_finance_core::SignedReadRequest::OrderQuery {
                market,
                symbol,
                target,
            } => self.query_order(market, &symbol, &target).await,
            agent_finance_core::SignedReadRequest::OpenOrders { market, symbol } => {
                self.open_orders(market, symbol.as_deref()).await
            }
            agent_finance_core::SignedReadRequest::TransferHistory {
                direction,
                current,
                size,
            } => self.transfer_history(direction, current, size).await,
        }
    }
}

async fn signed_read_snapshot(
    profile: &agent_finance_core::Profile,
    executor: &impl SignedReadExecutor,
    request: agent_finance_core::SignedReadRequest,
) -> Result<agent_finance_core::SignedReadSnapshot> {
    let request = request.normalized();
    validate_signed_read_request(profile, &request)?;
    let payload = executor.execute_signed_read(request.clone()).await?;
    Ok(agent_finance_core::SignedReadSnapshot::new(
        profile.name.clone(),
        profile.provider.provider,
        profile.provider.environment,
        request,
        payload,
    ))
}

fn validate_signed_read_request(
    profile: &agent_finance_core::Profile,
    request: &agent_finance_core::SignedReadRequest,
) -> Result<()> {
    if matches!(
        request,
        agent_finance_core::SignedReadRequest::TransferHistory { .. }
    ) && profile.provider.environment != agent_finance_core::Environment::Live
    {
        return Err(anyhow!(
            "transfer history uses Binance SAPI live account data; use a live profile after reviewing the request"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{
        Environment, Market, OrderIdentifier, Profile, ProfilePermissions, Provider,
        ProviderConfig, RiskPolicy, SignedReadRequest, SignedReadSnapshotKind, TransferDirection,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn service_wraps_signed_read_scope_and_payload() {
        let profile = test_profile(Environment::Live);
        let executor = FakeExecutor;
        let cases = [
            (
                SignedReadRequest::ApiPermissions,
                SignedReadSnapshotKind::ApiPermissions,
                "api-permissions",
            ),
            (
                SignedReadRequest::SpotBalances,
                SignedReadSnapshotKind::SpotBalances,
                "spot-balances",
            ),
            (
                SignedReadRequest::UsdsFuturesPositions,
                SignedReadSnapshotKind::UsdsFuturesPositions,
                "usds-futures-positions",
            ),
            (
                SignedReadRequest::OrderQuery {
                    market: Market::Spot,
                    symbol: "BTCUSDT".to_string(),
                    target: OrderIdentifier::ClientOrderId {
                        client_order_id: "af-test".to_string(),
                    },
                },
                SignedReadSnapshotKind::OrderQuery,
                "order-query",
            ),
            (
                SignedReadRequest::OpenOrders {
                    market: Market::UsdsFutures,
                    symbol: Some("ETHUSDT".to_string()),
                },
                SignedReadSnapshotKind::OpenOrders,
                "open-orders",
            ),
            (
                SignedReadRequest::transfer_history(TransferDirection::SpotToUsdsFutures, 2, 25),
                SignedReadSnapshotKind::TransferHistory,
                "transfer-history",
            ),
        ];

        for (request, expected_kind, expected_label) in cases {
            let snapshot = signed_read_snapshot(&profile, &executor, request.clone())
                .await
                .expect("signed read snapshot");

            assert_eq!(snapshot.profile, "test");
            assert_eq!(snapshot.provider, Provider::Binance);
            assert_eq!(snapshot.environment, Environment::Live);
            assert_eq!(snapshot.kind, expected_kind);

            let value = serde_json::to_value(snapshot).expect("snapshot json");
            assert_eq!(value["kind"], expected_label);
            assert_eq!(value["request"], serde_json::to_value(request).unwrap());
            assert_eq!(value["payload"]["source_kind"], expected_label);
        }
    }

    #[tokio::test]
    async fn service_blocks_transfer_history_outside_live_profiles() {
        let profile = test_profile(Environment::Testnet);
        let error = signed_read_snapshot(
            &profile,
            &FakeExecutor,
            SignedReadRequest::transfer_history(TransferDirection::SpotToUsdsFutures, 1, 10),
        )
        .await
        .expect_err("testnet transfer history should be blocked before executor");

        assert!(
            format!("{error:#}").contains("SAPI live account data"),
            "error should explain the live SAPI guard: {error:#}"
        );
    }

    struct FakeExecutor;

    impl SignedReadExecutor for FakeExecutor {
        async fn execute_signed_read(&self, request: SignedReadRequest) -> Result<Value> {
            Ok(json!({ "source_kind": request.snapshot_kind().to_string() }))
        }
    }

    fn test_profile(environment: Environment) -> Profile {
        Profile {
            name: "test".to_string(),
            provider: ProviderConfig {
                provider: Provider::Binance,
                environment,
                api_key_env: "BINANCE_API_KEY".to_string(),
                api_secret_env: "BINANCE_PRIVATE_KEY".to_string(),
                spot_base_url: None,
                usds_futures_base_url: None,
                sapi_base_url: None,
            },
            permissions: ProfilePermissions {
                spot_trading: true,
                usds_futures: true,
                universal_transfer: true,
            },
            risk: RiskPolicy {
                allow_live: false,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::new(),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: Vec::new(),
            },
        }
    }
}
