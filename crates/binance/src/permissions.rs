use agent_finance_core::{IntentKind, Profile, ProfilePermission, ProfilePermissionSet};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct PermissionCheck {
    pub name: &'static str,
    pub ok: bool,
    pub required: bool,
    pub message: String,
}

pub fn profile_permission_checks(profile: &Profile, payload: &Value) -> Vec<PermissionCheck> {
    permission_checks(profile.permissions.declared_profile_permissions(), payload)
}

pub fn intent_permission_checks(intent: &IntentKind, payload: &Value) -> Vec<PermissionCheck> {
    permission_checks(intent.required_profile_permissions(), payload)
}

pub fn blocking_permission_error(checks: &[PermissionCheck]) -> Option<String> {
    let failed = checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .map(|check| check.name)
        .collect::<Vec<_>>();
    if failed.is_empty() {
        None
    } else {
        Some(format!(
            "Binance API key permissions are insufficient: {}",
            failed.join(", ")
        ))
    }
}

fn permission_checks(requirements: ProfilePermissionSet, payload: &Value) -> Vec<PermissionCheck> {
    ProfilePermission::ALL
        .into_iter()
        .map(|permission| {
            permission_check(
                permission,
                requirements.contains(permission),
                bool_any(payload, &[binance_payload_field(permission)]),
            )
        })
        .collect()
}

fn permission_check(
    permission: ProfilePermission,
    required: bool,
    granted: Option<bool>,
) -> PermissionCheck {
    let ok = !required || granted == Some(true);
    let message = match (required, granted) {
        (false, Some(true)) => "permission is present but not required by this profile".to_string(),
        (false, Some(false)) => "permission is absent and not required by this profile".to_string(),
        (false, None) => "permission field is missing and not required by this profile".to_string(),
        (true, Some(true)) => "required permission is present".to_string(),
        (true, Some(false)) => format!(
            "required permission is disabled; {}",
            binance_required_message(permission)
        ),
        (true, None) => format!(
            "required permission field is missing; {}",
            binance_required_message(permission)
        ),
    };
    PermissionCheck {
        name: binance_check_name(permission),
        ok,
        required,
        message,
    }
}

fn bool_any(payload: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| payload.get(*key)?.as_bool())
}

const fn binance_check_name(permission: ProfilePermission) -> &'static str {
    match permission {
        ProfilePermission::SpotTrading => "binance-spot-trading",
        ProfilePermission::UsdsFutures => "binance-usds-futures",
        ProfilePermission::UniversalTransfer => "binance-universal-transfer",
    }
}

const fn binance_payload_field(permission: ProfilePermission) -> &'static str {
    match permission {
        ProfilePermission::SpotTrading => "enableSpotAndMarginTrading",
        ProfilePermission::UsdsFutures => "enableFutures",
        ProfilePermission::UniversalTransfer => "permitsUniversalTransfer",
    }
}

const fn binance_required_message(permission: ProfilePermission) -> &'static str {
    match permission {
        ProfilePermission::SpotTrading => {
            "spot trading is required for live spot orders and cancels"
        }
        ProfilePermission::UsdsFutures => {
            "USD-M futures permission is required for futures orders and state changes"
        }
        ProfilePermission::UniversalTransfer => {
            "universal transfer permission is required for Spot <-> USD-M transfers"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{
        DecimalValue, Environment, FuturesStateIntent, Market, OrderIntent, OrderSide, OrderSpec,
        ProfilePermissions, Provider, ProviderConfig, RiskPolicy, TimeInForce,
    };
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn profile_checks_follow_declared_risk_surface() {
        let profile = profile_with_risk(RiskPolicy {
            allow_live: true,
            max_daily_order_notional_usdt: None,
            allowed_symbols: BTreeMap::from([(
                "BTCUSDT".to_string(),
                agent_finance_core::SymbolPolicy {
                    markets: vec![Market::Spot, Market::UsdsFutures],
                    order_kinds: Vec::new(),
                    max_order_notional_usdt: decimal("10"),
                },
            )]),
            allowed_transfers: vec![agent_finance_core::TransferPolicy {
                direction: agent_finance_core::TransferDirection::SpotToUsdsFutures,
                asset: "USDT".to_string(),
                max_amount: decimal("1"),
            }],
            allowed_futures_state_changes: Vec::new(),
        });
        let checks = profile_permission_checks(
            &profile,
            &json!({
                "enableSpotAndMarginTrading": true,
                "enableFutures": false,
                "permitsUniversalTransfer": false
            }),
        );

        assert!(check(&checks, "binance-spot-trading").ok);
        assert!(!check(&checks, "binance-usds-futures").ok);
        assert!(!check(&checks, "binance-universal-transfer").ok);
        assert!(blocking_permission_error(&checks).is_some());
    }

    #[test]
    fn profile_api_probe_checks_declared_permissions_even_when_risk_is_empty() {
        let profile = profile_with_risk(RiskPolicy {
            allow_live: true,
            max_daily_order_notional_usdt: None,
            allowed_symbols: BTreeMap::new(),
            allowed_transfers: Vec::new(),
            allowed_futures_state_changes: Vec::new(),
        });

        let checks = profile_permission_checks(
            &profile,
            &json!({
                "enableSpotAndMarginTrading": false,
                "enableFutures": false,
                "permitsUniversalTransfer": false
            }),
        );

        assert!(
            !check(&checks, "binance-spot-trading").ok,
            "declared permissions should drive API key validation even before risk needs them"
        );
        assert!(!check(&checks, "binance-usds-futures").ok);
        assert!(!check(&checks, "binance-universal-transfer").ok);
    }

    #[test]
    fn intent_checks_are_scoped_to_the_live_write() {
        let payload = json!({
            "enableSpotAndMarginTrading": false,
            "enableFutures": true,
            "permitsUniversalTransfer": false
        });
        let spot = intent_permission_checks(&spot_order(), &payload);
        let futures = intent_permission_checks(&futures_state(), &payload);
        let cancel = intent_permission_checks(&spot_cancel(), &payload);
        let transfer = intent_permission_checks(&transfer(), &payload);

        assert!(!check(&spot, "binance-spot-trading").ok);
        assert!(!check(&spot, "binance-usds-futures").required);
        assert!(!check(&cancel, "binance-spot-trading").ok);
        assert!(!check(&cancel, "binance-usds-futures").required);
        assert!(check(&futures, "binance-usds-futures").ok);
        assert!(!check(&futures, "binance-spot-trading").required);
        assert!(!check(&transfer, "binance-universal-transfer").ok);
        assert!(!check(&transfer, "binance-spot-trading").required);
    }

    fn check<'a>(checks: &'a [PermissionCheck], name: &str) -> &'a PermissionCheck {
        checks.iter().find(|check| check.name == name).expect(name)
    }

    fn profile_with_risk(risk: RiskPolicy) -> Profile {
        Profile {
            name: "test".to_string(),
            provider: ProviderConfig {
                provider: Provider::Binance,
                environment: Environment::Live,
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
            risk,
        }
    }

    fn spot_order() -> IntentKind {
        IntentKind::Order(OrderIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Live,
            market: Market::Spot,
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            quantity: decimal("0.001"),
            spec: OrderSpec::Limit {
                price: decimal("50000"),
                time_in_force: TimeInForce::Gtc,
            },
            reduce_only: false,
            position_side: None,
            client_order_id: "af-test".to_string(),
        })
    }

    fn spot_cancel() -> IntentKind {
        IntentKind::Cancel(agent_finance_core::CancelIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Live,
            market: Market::Spot,
            symbol: "BTCUSDT".to_string(),
            target: agent_finance_core::OrderIdentifier::ClientOrderId {
                client_order_id: "af-test".to_string(),
            },
        })
    }

    fn transfer() -> IntentKind {
        IntentKind::Transfer(agent_finance_core::TransferIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Live,
            direction: agent_finance_core::TransferDirection::SpotToUsdsFutures,
            asset: "USDT".to_string(),
            amount: decimal("1"),
            client_transfer_id: "af-transfer".to_string(),
        })
    }

    fn futures_state() -> IntentKind {
        IntentKind::FuturesState(FuturesStateIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Live,
            change: agent_finance_core::FuturesStateChange::Leverage {
                symbol: "BTCUSDT".to_string(),
                leverage: 2,
            },
        })
    }

    fn decimal(value: &str) -> DecimalValue {
        DecimalValue(value.parse::<Decimal>().expect("decimal"))
    }
}
