use agent_finance_core::{FuturesStateChange, FuturesStateIntent, MarginType, PositionMode};

pub(super) fn path(intent: &FuturesStateIntent) -> &'static str {
    match intent.change {
        FuturesStateChange::Leverage { .. } => "/fapi/v1/leverage",
        FuturesStateChange::MarginType { .. } => "/fapi/v1/marginType",
        FuturesStateChange::PositionMode { .. } => "/fapi/v1/positionSide/dual",
    }
}

pub(super) fn params(intent: &FuturesStateIntent) -> Vec<(String, String)> {
    match &intent.change {
        FuturesStateChange::Leverage { symbol, leverage } => vec![
            ("symbol".to_string(), symbol.to_ascii_uppercase()),
            ("leverage".to_string(), leverage.to_string()),
        ],
        FuturesStateChange::MarginType {
            symbol,
            margin_type,
        } => vec![
            ("symbol".to_string(), symbol.to_ascii_uppercase()),
            (
                "marginType".to_string(),
                binance_margin_type(*margin_type).to_string(),
            ),
        ],
        FuturesStateChange::PositionMode { mode } => vec![(
            "dualSidePosition".to_string(),
            binance_position_mode(*mode).to_string(),
        )],
    }
}

fn binance_margin_type(value: MarginType) -> &'static str {
    match value {
        MarginType::Cross => "CROSSED",
        MarginType::Isolated => "ISOLATED",
    }
}

fn binance_position_mode(value: PositionMode) -> &'static str {
    match value {
        PositionMode::OneWay => "false",
        PositionMode::Hedge => "true",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{Environment, Provider};

    #[test]
    fn maps_position_modes_to_account_scoped_binance_params() {
        let cases = [
            (PositionMode::Hedge, "true"),
            (PositionMode::OneWay, "false"),
        ];

        for (mode, expected) in cases {
            let intent = intent(FuturesStateChange::PositionMode { mode });

            assert_eq!(path(&intent), "/fapi/v1/positionSide/dual");
            assert!(
                params(&intent).contains(&("dualSidePosition".to_string(), expected.to_string())),
                "position mode {mode} should map to dualSidePosition={expected}"
            );
            assert!(
                !params(&intent).iter().any(|(key, _)| key == "symbol"),
                "position mode is account-scoped and must not send symbol"
            );
        }
    }

    #[test]
    fn maps_symbol_scoped_state_params() {
        let leverage = intent(FuturesStateChange::Leverage {
            symbol: "btcusdt".to_string(),
            leverage: 2,
        });
        assert_eq!(path(&leverage), "/fapi/v1/leverage");
        assert!(params(&leverage).contains(&("symbol".to_string(), "BTCUSDT".to_string())));
        assert!(params(&leverage).contains(&("leverage".to_string(), "2".to_string())));

        let margin = intent(FuturesStateChange::MarginType {
            symbol: "btcusdt".to_string(),
            margin_type: MarginType::Isolated,
        });
        assert_eq!(path(&margin), "/fapi/v1/marginType");
        assert!(params(&margin).contains(&("symbol".to_string(), "BTCUSDT".to_string())));
        assert!(params(&margin).contains(&("marginType".to_string(), "ISOLATED".to_string())));
    }

    fn intent(change: FuturesStateChange) -> FuturesStateIntent {
        FuturesStateIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Live,
            change,
        }
    }
}
