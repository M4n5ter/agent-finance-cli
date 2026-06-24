use agent_finance_core::{Capability, ProviderCapability};

pub fn profile_template(name: &str) -> String {
    format!(
        r#"name = "{name}"

[provider]
provider = "binance"
environment = "testnet"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_PRIVATE_KEY"
spot_base_url = "https://testnet.binance.vision"
usds_futures_base_url = "https://testnet.binancefuture.com"

[permissions]
spot_trading = true
usds_futures = true
universal_transfer = false

[risk]
allow_live = false
max_daily_order_notional_usdt = "50"
allowed_transfers = []

[[risk.allowed_futures_state_changes]]
kind = "leverage"
symbol = "BTCUSDT"
max_leverage = 2

[[risk.allowed_futures_state_changes]]
kind = "margin-type"
symbol = "BTCUSDT"
margin_type = "isolated"

[risk.allowed_symbols.BTCUSDT]
markets = ["spot", "usds-futures"]
order_kinds = ["market", "limit", "limit-maker"]
max_order_notional_usdt = "25"
"#
    )
}

pub fn provider_capability() -> ProviderCapability {
    ProviderCapability::new(
        "binance",
        vec![
            Capability::new(
                "market-data",
                "no-key/read-only",
                strings(["spot", "usds-futures"]),
                strings(["Existing public market-data commands remain available."]),
            ),
            Capability::new(
                "account",
                "signed/read-only",
                strings(["spot", "usds-futures"]),
                strings(["Uses HMAC signed USER_DATA endpoints."]),
            ),
            Capability::new(
                "orders",
                "signed/write-gated",
                strings(["spot", "usds-futures"]),
                strings([
                    "Intent-first; live submit requires profile policy and --live.",
                    "Spot post-only orders are modeled as order kind limit-maker and mapped to Binance LIMIT_MAKER.",
                    "Daily live order notional limits are enforced from local audit events.",
                    "Test/live order submit checks locally checkable Binance exchangeInfo filters before sending the order.",
                    "Signed order query is available by exchange order id or client order id.",
                ]),
            ),
            Capability::new(
                "transfers",
                "signed/write-gated",
                strings(["spot<->usds-futures"]),
                strings([
                    "Universal transfer only; withdrawals are intentionally unsupported.",
                    "Signed user universal transfer history is available.",
                ]),
            ),
            Capability::new(
                "futures-state",
                "signed/write-gated",
                strings(["usds-futures-symbols", "binance-futures-account"]),
                strings([
                    "Intent-first leverage, margin type, and position mode changes.",
                    "Live submit requires explicit risk.allowed_futures_state_changes policy.",
                    "Position mode changes every symbol; Binance UM/CM share dualSidePosition and the exchange rejects the change when either side has open orders or open positions.",
                ]),
            ),
        ],
    )
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}
