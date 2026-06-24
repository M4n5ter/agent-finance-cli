use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_finance_core::DecimalValue;

#[test]
#[ignore = "requires AGENT_FINANCE_LIVE_BINANCE_PLACE_AND_CANCEL_ORDER=1, explicit ACK, live order params, and persistent smoke data home"]
fn binance_live_order_cancel_smoke_is_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_BINANCE_PLACE_AND_CANCEL_ORDER")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping live Binance place-and-cancel smoke; set AGENT_FINANCE_LIVE_BINANCE_PLACE_AND_CANCEL_ORDER=1"
        );
        return;
    }
    require_confirmation(
        "AGENT_FINANCE_LIVE_BINANCE_WRITE_ACK",
        "I_UNDERSTAND_THIS_PLACES_A_LIVE_ORDER",
    );
    require_binance_hmac_env();

    let spec = LiveOrderSmokeSpec::from_env();
    let env = SignedProfileEnv::new_live_write("live-binance-order");
    env.write_profile_toml("live-binance-order", signed_live_order_profile_toml(&spec));
    assert_non_marketable_order(&env, &spec);

    let intent = env.command_json(&[
        "order",
        "create",
        &spec.symbol,
        "--profile",
        "live-binance-order",
        "--market",
        &spec.market,
        "--side",
        &spec.side,
        "--kind",
        "limit-maker",
        "--quantity",
        &spec.quantity,
        "--price",
        &spec.price,
        "--json",
    ]);
    assert_eq!(intent["risk"]["allowed"], true);
    let intent_id = intent["intent"]["id"]
        .as_str()
        .expect("order intent should have id");
    let client_order_id = intent["intent"]["kind"]["client_order_id"]
        .as_str()
        .expect("order intent should have client order id");

    let live_risk = env.command_json(&[
        "risk",
        "check",
        intent_id,
        "--profile",
        "live-binance-order",
        "--live",
        "--json",
    ]);
    assert_eq!(live_risk["allowed"], true);

    let mut cleanup =
        LiveOrderCleanup::new(&env.runner, "live-binance-order", &spec, client_order_id);
    let submitted = env.command_json(&[
        "order",
        "submit",
        intent_id,
        "--profile",
        "live-binance-order",
        "--live",
        "--json",
    ]);
    assert_eq!(submitted["mode"], "Live");
    assert_eq!(submitted["risk"]["allowed"], true);
    assert_eq!(
        submitted["response"]["exchange_response"]["clientOrderId"],
        client_order_id
    );
    assert_audit_contains_intent(&env, "live-submit", intent_id);

    let canceled = cleanup.cancel();
    assert_eq!(canceled.response["mode"], "Live");
    assert_eq!(canceled.response["risk"]["allowed"], true);
    assert_eq!(
        canceled.response["response"]["clientOrderId"],
        client_order_id
    );
    assert_audit_contains_intent(&env, "cancel", &canceled.intent_id);
    let order = env.command_json(&[
        "order",
        "query",
        &spec.symbol,
        "--profile",
        "live-binance-order",
        "--market",
        &spec.market,
        "--client-order-id",
        client_order_id,
        "--json",
    ]);
    assert_eq!(order["clientOrderId"], client_order_id);
    assert_ne!(
        order["status"], "NEW",
        "live smoke order should not remain open after cancel"
    );
}

#[test]
#[ignore = "requires AGENT_FINANCE_LIVE_BINANCE_TRANSFERS=1, explicit ACK, live transfer params, and persistent smoke data home"]
fn binance_live_transfer_smoke_is_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_BINANCE_TRANSFERS")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping live Binance transfer smoke; set AGENT_FINANCE_LIVE_BINANCE_TRANSFERS=1"
        );
        return;
    }
    require_confirmation(
        "AGENT_FINANCE_LIVE_BINANCE_TRANSFER_ACK",
        "I_UNDERSTAND_THIS_MOVES_FUNDS",
    );
    require_binance_hmac_env();

    let spec = LiveTransferSmokeSpec::from_env();
    let env = SignedProfileEnv::new_live_write("live-binance-transfer");
    env.write_profile_toml(
        "live-binance-transfer",
        signed_live_transfer_profile_toml(&spec),
    );

    let intent = env.command_json(&[
        "transfer",
        "create",
        &spec.asset,
        "--profile",
        "live-binance-transfer",
        "--direction",
        &spec.direction,
        "--amount",
        &spec.amount,
        "--json",
    ]);
    assert_eq!(intent["risk"]["allowed"], true);
    let intent_id = intent["intent"]["id"]
        .as_str()
        .expect("transfer intent should have id");

    let live_risk = env.command_json(&[
        "risk",
        "check",
        intent_id,
        "--profile",
        "live-binance-transfer",
        "--live",
        "--json",
    ]);
    assert_eq!(live_risk["allowed"], true);

    let submitted = env.command_json(&[
        "transfer",
        "submit",
        intent_id,
        "--profile",
        "live-binance-transfer",
        "--live",
        "--json",
    ]);
    assert_eq!(submitted["mode"], "Live");
    assert_eq!(submitted["risk"]["allowed"], true);
    assert!(
        submitted["response"].get("tranId").is_some()
            || submitted["response"].get("clientTranId").is_some(),
        "live transfer response should include an exchange transfer identifier"
    );
    assert_audit_contains_intent(&env, "transfer", intent_id);
}

fn require_binance_hmac_env() {
    assert!(
        std::env::var("BINANCE_API_KEY").is_ok(),
        "BINANCE_API_KEY must be exported for signed Binance smoke tests"
    );
    assert!(
        std::env::var("BINANCE_PRIVATE_KEY").is_ok(),
        "BINANCE_PRIVATE_KEY must be exported for signed Binance smoke tests"
    );
}

fn require_confirmation(env_var: &str, expected: &str) {
    assert_eq!(
        std::env::var(env_var).ok().as_deref(),
        Some(expected),
        "{env_var} must be set to {expected:?}"
    );
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be exported"))
}

fn assert_audit_contains_intent(env: &SignedProfileEnv, expected_kind: &str, intent_id: &str) {
    let audit = env.command_json(&["audit", "tail", "--limit", "20", "--json"]);
    let events = audit.as_array().expect("audit tail should be an array");
    assert!(
        events
            .iter()
            .any(|event| event["kind"] == expected_kind && event["intent_id"] == intent_id),
        "audit should include {expected_kind} for intent {intent_id}; audit={audit}"
    );
}

fn assert_non_marketable_order(env: &SignedProfileEnv, spec: &LiveOrderSmokeSpec) {
    let book = env.command_json(&[
        "market",
        "crypto",
        "book",
        &spec.symbol,
        "--provider",
        "binance",
        "--instrument",
        "spot",
        "--limit",
        "1",
        "--json",
    ]);
    let payload = &book["results"][0]["endpoints"][0]["payload"]["payload"];
    let best_bid = book_price(payload, "bids");
    let best_ask = book_price(payload, "asks");
    let price = spec.price();
    match spec.side.as_str() {
        "buy" => assert!(
            price.0 < best_bid.0,
            "live buy smoke price must be below best bid to avoid taking liquidity: price={price} best_bid={best_bid}"
        ),
        "sell" => assert!(
            price.0 > best_ask.0,
            "live sell smoke price must be above best ask to avoid taking liquidity: price={price} best_ask={best_ask}"
        ),
        other => panic!("unsupported live smoke side {other:?}"),
    }
}

fn book_price(payload: &serde_json::Value, side: &str) -> DecimalValue {
    payload[side][0][0]
        .as_str()
        .unwrap_or_else(|| panic!("missing best {side} price in order book: {payload}"))
        .parse()
        .unwrap_or_else(|_| panic!("invalid best {side} price in order book: {payload}"))
}

struct SignedProfileEnv {
    root: PathBuf,
    runner: CommandEnv,
}

impl SignedProfileEnv {
    fn new_live_write(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "agent-finance-live-write-{name}-{}-{nanos}",
            std::process::id(),
        ));
        fs::create_dir_all(&root).expect("signed profile test root");
        let data_home = live_smoke_data_home();
        fs::create_dir_all(&data_home).expect("persistent live smoke data home");
        Self {
            runner: CommandEnv {
                config_home: root.join("config"),
                data_home,
            },
            root,
        }
    }

    fn write_profile_toml(&self, profile: &str, content: String) {
        let profile_dir = self.runner.config_home.join("agent-finance/profiles");
        fs::create_dir_all(&profile_dir).expect("profile dir");
        fs::write(profile_dir.join(format!("{profile}.toml")), content).expect("profile write");
    }

    fn command_json(&self, args: &[&str]) -> serde_json::Value {
        self.runner.command_json(args)
    }
}

impl Drop for SignedProfileEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct CommandEnv {
    config_home: PathBuf,
    data_home: PathBuf,
}

impl CommandEnv {
    fn command_json(&self, args: &[&str]) -> serde_json::Value {
        let output = self.command_output(args);
        assert!(
            output.status.success(),
            "command failed: args={args:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("command should print JSON")
    }

    fn command_output(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_agent-finance"))
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_DATA_HOME", &self.data_home)
            .args(args)
            .output()
            .expect("agent-finance command should start")
    }
}

struct LiveOrderCleanup {
    runner: CommandEnv,
    profile: String,
    symbol: String,
    market: String,
    client_order_id: String,
    active: bool,
}

impl LiveOrderCleanup {
    fn new(
        runner: &CommandEnv,
        profile: &str,
        spec: &LiveOrderSmokeSpec,
        client_order_id: &str,
    ) -> Self {
        Self {
            runner: runner.clone(),
            profile: profile.to_string(),
            symbol: spec.symbol.clone(),
            market: spec.market.clone(),
            client_order_id: client_order_id.to_string(),
            active: true,
        }
    }

    fn cancel(&mut self) -> LiveCancelResult {
        let cancel = self.runner.command_json(&[
            "order",
            "cancel",
            &self.symbol,
            "--profile",
            &self.profile,
            "--market",
            &self.market,
            "--client-order-id",
            &self.client_order_id,
            "--json",
        ]);
        assert_eq!(cancel["risk"]["allowed"], true);
        let cancel_id = cancel["intent"]["id"]
            .as_str()
            .expect("cancel intent should have id");
        let canceled = self.runner.command_json(&[
            "order",
            "submit",
            cancel_id,
            "--profile",
            &self.profile,
            "--live",
            "--json",
        ]);
        self.active = false;
        LiveCancelResult {
            response: canceled,
            intent_id: cancel_id.to_string(),
        }
    }
}

impl Drop for LiveOrderCleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let cancel = self.runner.command_output(&[
            "order",
            "cancel",
            &self.symbol,
            "--profile",
            &self.profile,
            "--market",
            &self.market,
            "--client-order-id",
            &self.client_order_id,
            "--json",
        ]);
        if !cancel.status.success() {
            eprintln!(
                "best-effort live order cleanup could not create cancel intent: {}",
                String::from_utf8_lossy(&cancel.stderr)
            );
            return;
        }
        let Ok(cancel_json) = serde_json::from_slice::<serde_json::Value>(&cancel.stdout) else {
            eprintln!("best-effort live order cleanup could not parse cancel intent");
            return;
        };
        let Some(cancel_id) = cancel_json["intent"]["id"].as_str() else {
            eprintln!("best-effort live order cleanup cancel intent had no id");
            return;
        };
        let submit = self.runner.command_output(&[
            "order",
            "submit",
            cancel_id,
            "--profile",
            &self.profile,
            "--live",
            "--json",
        ]);
        if !submit.status.success() {
            eprintln!(
                "best-effort live order cleanup failed: {}",
                String::from_utf8_lossy(&submit.stderr)
            );
        }
    }
}

struct LiveCancelResult {
    response: serde_json::Value,
    intent_id: String,
}

struct LiveOrderSmokeSpec {
    symbol: String,
    market: String,
    side: String,
    quantity: String,
    price: String,
    max_notional_usdt: String,
}

impl LiveOrderSmokeSpec {
    fn from_env() -> Self {
        let spec = Self {
            symbol: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_SYMBOL").to_ascii_uppercase(),
            market: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_MARKET"),
            side: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_SIDE"),
            quantity: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_QUANTITY"),
            price: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_PRICE"),
            max_notional_usdt: required_env("AGENT_FINANCE_LIVE_BINANCE_ORDER_MAX_NOTIONAL_USDT"),
        };
        assert_eq!(
            spec.market, "spot",
            "live place-and-cancel smoke uses Binance spot LIMIT_MAKER; set AGENT_FINANCE_LIVE_BINANCE_ORDER_MARKET=spot"
        );
        spec
    }

    fn price(&self) -> DecimalValue {
        self.price
            .parse()
            .expect("live smoke order price should be a positive decimal")
    }
}

fn live_smoke_data_home() -> PathBuf {
    let path = PathBuf::from(required_env("AGENT_FINANCE_LIVE_BINANCE_SMOKE_DATA_HOME"));
    let temp_dir = std::env::temp_dir();
    assert!(
        !path.starts_with(&temp_dir),
        "AGENT_FINANCE_LIVE_BINANCE_SMOKE_DATA_HOME must be persistent and must not be under {}",
        temp_dir.display()
    );
    path
}

struct LiveTransferSmokeSpec {
    asset: String,
    direction: String,
    amount: String,
    max_amount: String,
}

impl LiveTransferSmokeSpec {
    fn from_env() -> Self {
        Self {
            asset: required_env("AGENT_FINANCE_LIVE_BINANCE_TRANSFER_ASSET").to_ascii_uppercase(),
            direction: required_env("AGENT_FINANCE_LIVE_BINANCE_TRANSFER_DIRECTION"),
            amount: required_env("AGENT_FINANCE_LIVE_BINANCE_TRANSFER_AMOUNT"),
            max_amount: required_env("AGENT_FINANCE_LIVE_BINANCE_TRANSFER_MAX_AMOUNT"),
        }
    }
}

fn signed_live_order_profile_toml(spec: &LiveOrderSmokeSpec) -> String {
    let spot_trading = spec.market == "spot";
    let usds_futures = spec.market == "usds-futures";
    format!(
        r#"name = "live-binance-order"

[provider]
provider = "binance"
environment = "live"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_PRIVATE_KEY"

[permissions]
spot_trading = {spot_trading}
usds_futures = {usds_futures}
universal_transfer = false

[risk]
allow_live = true
max_daily_order_notional_usdt = "{max_notional}"
allowed_transfers = []

[risk.allowed_symbols.{symbol}]
markets = ["{market}"]
order_kinds = ["limit-maker"]
max_order_notional_usdt = "{max_notional}"
"#,
        symbol = spec.symbol,
        market = spec.market,
        max_notional = spec.max_notional_usdt,
    )
}

fn signed_live_transfer_profile_toml(spec: &LiveTransferSmokeSpec) -> String {
    format!(
        r#"name = "live-binance-transfer"

[provider]
provider = "binance"
environment = "live"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_PRIVATE_KEY"

[permissions]
spot_trading = false
usds_futures = false
universal_transfer = true

[risk]
allow_live = true
max_daily_order_notional_usdt = "1"

[[risk.allowed_transfers]]
direction = "{direction}"
asset = "{asset}"
max_amount = "{max_amount}"
"#,
        direction = spec.direction,
        asset = spec.asset,
        max_amount = spec.max_amount,
    )
}
