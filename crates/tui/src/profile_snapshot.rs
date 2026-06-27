use agent_finance_core::{Profile, ProfilePermission, RiskPolicy};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TradingProfileSnapshot {
    pub declared_permissions: Vec<ProfilePermission>,
    pub required_permissions: Vec<ProfilePermission>,
    pub missing_permissions: Vec<ProfilePermission>,
    pub risk: RiskPolicy,
}

impl From<&Profile> for TradingProfileSnapshot {
    fn from(profile: &Profile) -> Self {
        let declared_permissions = ProfilePermission::ALL
            .into_iter()
            .filter(|permission| profile.permissions.allows(*permission))
            .collect::<Vec<_>>();
        let required_permissions = profile
            .risk
            .required_profile_permissions()
            .iter()
            .collect::<Vec<_>>();
        let missing_permissions = required_permissions
            .iter()
            .copied()
            .filter(|permission| !declared_permissions.contains(permission))
            .collect();

        Self {
            declared_permissions,
            required_permissions,
            missing_permissions,
            risk: profile.risk.clone(),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_trading_profile_snapshot() -> TradingProfileSnapshot {
    TradingProfileSnapshot::from(&test_profile())
}

#[cfg(test)]
fn test_profile() -> Profile {
    use std::collections::BTreeMap;

    use agent_finance_core::{
        DecimalValue, Environment, Market, OrderKind, ProfilePermissions, Provider, ProviderConfig,
        SymbolPolicy,
    };
    use rust_decimal::Decimal;

    Profile {
        name: "mainnet".to_string(),
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
            usds_futures: false,
            universal_transfer: false,
        },
        risk: RiskPolicy {
            allow_live: true,
            max_daily_order_notional_usdt: Some(DecimalValue::new(Decimal::new(100, 0))),
            allowed_symbols: BTreeMap::from([(
                "btcusdt".to_string(),
                SymbolPolicy {
                    markets: vec![Market::Spot],
                    order_kinds: vec![OrderKind::Limit],
                    max_order_notional_usdt: DecimalValue::new(Decimal::new(50, 0)),
                },
            )]),
            allowed_transfers: Vec::new(),
            allowed_futures_state_changes: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_snapshot_keeps_risk_policy_typed_and_preserves_symbol_keys() {
        let profile = test_profile();

        let snapshot = TradingProfileSnapshot::from(&profile);

        assert_eq!(
            snapshot.declared_permissions,
            vec![ProfilePermission::SpotTrading]
        );
        assert_eq!(
            snapshot.required_permissions,
            vec![ProfilePermission::SpotTrading]
        );
        assert!(snapshot.risk.allow_live);
        assert!(snapshot.risk.allowed_symbols.contains_key("btcusdt"));
        assert!(!snapshot.risk.allowed_symbols.contains_key("BTCUSDT"));
    }

    #[test]
    fn profile_snapshot_reports_missing_permissions() {
        let mut profile = test_profile();
        profile.permissions.spot_trading = false;

        let snapshot = TradingProfileSnapshot::from(&profile);

        assert_eq!(
            snapshot.missing_permissions,
            vec![ProfilePermission::SpotTrading]
        );
    }
}
