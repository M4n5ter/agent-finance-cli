use agent_finance_core::{CancelIntent, FuturesStateIntent, OrderIntent, Profile, TransferIntent};
use chrono::Utc;

use crate::state::{CancelReview, FuturesStateReview, OrderTicketReview, TransferReview};

pub(crate) fn order_intent_from_review(
    profile: &Profile,
    review: &OrderTicketReview,
    client_order_id: String,
) -> OrderIntent {
    OrderIntent {
        profile: profile.name.clone(),
        provider: profile.provider.provider,
        environment: profile.provider.environment,
        market: review.market,
        symbol: review.symbol.to_ascii_uppercase(),
        side: review.side,
        quantity: review.parsed_quantity.clone(),
        spec: review.order_spec.clone(),
        reduce_only: review.reduce_only,
        position_side: None,
        client_order_id,
    }
}

pub(crate) fn transfer_intent_from_review(
    profile: &Profile,
    review: &TransferReview,
    client_transfer_id: String,
) -> TransferIntent {
    TransferIntent {
        profile: profile.name.clone(),
        provider: profile.provider.provider,
        environment: profile.provider.environment,
        direction: review.direction,
        asset: review.asset.to_ascii_uppercase(),
        amount: review.parsed_amount.clone(),
        client_transfer_id,
    }
}

pub(crate) fn futures_state_intent_from_review(
    profile: &Profile,
    review: &FuturesStateReview,
) -> FuturesStateIntent {
    FuturesStateIntent {
        profile: profile.name.clone(),
        provider: profile.provider.provider,
        environment: profile.provider.environment,
        change: review.change.clone(),
    }
}

pub(crate) fn cancel_intent_from_review(profile: &Profile, review: &CancelReview) -> CancelIntent {
    CancelIntent {
        profile: profile.name.clone(),
        provider: profile.provider.provider,
        environment: profile.provider.environment,
        market: review.market,
        symbol: review.symbol.to_ascii_uppercase(),
        target: review.target(),
    }
}

pub(crate) fn generated_order_client_id() -> String {
    format!("af-tui-{}", Utc::now().timestamp_millis())
}

pub(crate) fn generated_transfer_client_id() -> String {
    format!("af-tui-transfer-{}", Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::str::FromStr;

    use agent_finance_core::{
        DecimalValue, Environment, FuturesStateChange, ProfilePermissions, Provider,
        ProviderConfig, RiskPolicy, SubmitMode, TransferDirection, TransferPolicy,
    };

    use super::*;

    #[test]
    fn transfer_intent_from_review_preserves_profile_and_normalizes_asset() {
        let profile = Profile {
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
                spot_trading: false,
                usds_futures: true,
                universal_transfer: true,
            },
            risk: RiskPolicy {
                allow_live: false,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::new(),
                allowed_transfers: vec![TransferPolicy {
                    direction: TransferDirection::SpotToUsdsFutures,
                    asset: "USDT".to_string(),
                    max_amount: DecimalValue::from_str("10").unwrap(),
                }],
                allowed_futures_state_changes: Vec::new(),
            },
        };
        let review = TransferReview {
            profile: "mainnet".to_string(),
            direction: TransferDirection::SpotToUsdsFutures,
            asset: "usdt".to_string(),
            amount: "5".to_string(),
            parsed_amount: DecimalValue::from_str("5").unwrap(),
            effective_mode: SubmitMode::DryRun,
        };

        let intent =
            transfer_intent_from_review(&profile, &review, "af-tui-transfer-test".to_string());

        assert_eq!(intent.profile, "mainnet");
        assert_eq!(intent.provider, Provider::Binance);
        assert_eq!(intent.environment, Environment::Live);
        assert_eq!(intent.direction, TransferDirection::SpotToUsdsFutures);
        assert_eq!(intent.asset, "USDT");
        assert_eq!(intent.amount, DecimalValue::from_str("5").unwrap());
        assert_eq!(intent.client_transfer_id, "af-tui-transfer-test");
    }

    #[test]
    fn futures_state_intent_from_review_preserves_profile_and_change() {
        let profile = Profile {
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
                spot_trading: false,
                usds_futures: true,
                universal_transfer: false,
            },
            risk: RiskPolicy {
                allow_live: false,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::new(),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: Vec::new(),
            },
        };
        let review = FuturesStateReview {
            profile: "mainnet".to_string(),
            change: FuturesStateChange::Leverage {
                symbol: "ETHUSDT".to_string(),
                leverage: 2,
            },
            effective_mode: SubmitMode::DryRun,
        };

        let intent = futures_state_intent_from_review(&profile, &review);

        assert_eq!(intent.profile, "mainnet");
        assert_eq!(intent.provider, Provider::Binance);
        assert_eq!(intent.environment, Environment::Live);
        assert_eq!(intent.change, review.change);
    }
}
