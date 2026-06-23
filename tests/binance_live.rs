use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const SYMBOL: &str = "BTCUSDT";

#[test]
fn crypto_watch_json_reports_errors_and_fails() {
    let base_url = one_shot_error_server();
    let output = Command::new(env!("CARGO_BIN_EXE_agent-finance"))
        .env("BINANCE_SPOT_BASE_URL", base_url)
        .args([
            "--no-proxy",
            "watch",
            "BAD",
            "--asset",
            "crypto",
            "--iterations",
            "1",
            "--json",
        ])
        .output()
        .expect("agent-finance command should start");

    assert!(
        !output.status.success(),
        "watch should fail when every crypto quote fails: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("watch should print JSON");
    assert!(
        json["points"].as_array().unwrap().is_empty(),
        "failed watch should not fabricate price points"
    );
    assert!(
        json["errors"]["BAD"]
            .as_str()
            .unwrap()
            .contains("status=400"),
        "failed watch should expose the provider error: {json}"
    );
}

#[test]
#[ignore = "requires AGENT_FINANCE_LIVE_BINANCE=1 and live Binance network access"]
fn binance_live_cli_surface_is_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_BINANCE").ok().as_deref() != Some("1") {
        eprintln!("skipping live Binance test; set AGENT_FINANCE_LIVE_BINANCE=1");
        return;
    }

    assert_aggregate(
        command(&["crypto", "snapshot", SYMBOL, "--json"]),
        "spot.ticker",
    );
    assert_aggregate(command(&["crypto", "sentiment", SYMBOL, "--json"]), "mark");

    for args in [
        &[
            "crypto",
            "quote",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "spot",
            "--json",
        ][..],
        &[
            "crypto",
            "book",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "spot",
            "--limit",
            "5",
            "--json",
        ],
        &[
            "crypto",
            "trades",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "spot",
            "--limit",
            "2",
            "--json",
        ],
        &[
            "crypto",
            "candles",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "spot",
            "--interval",
            "1m",
            "--limit",
            "2",
            "--json",
        ],
        &[
            "crypto",
            "quote",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--json",
        ],
        &[
            "crypto",
            "book",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--limit",
            "5",
            "--json",
        ],
        &[
            "crypto",
            "trades",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--limit",
            "2",
            "--json",
        ],
        &[
            "crypto",
            "candles",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--interval",
            "1m",
            "--limit",
            "2",
            "--json",
        ],
        &[
            "crypto",
            "funding",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--limit",
            "2",
            "--json",
        ],
        &[
            "crypto",
            "open-interest",
            SYMBOL,
            "--provider",
            "binance",
            "--instrument",
            "swap",
            "--json",
        ],
    ] {
        assert_evidence(command(args), "binance");
    }

    assert_stream(command(&[
        "crypto",
        "stream",
        SYMBOL,
        "--kind",
        "trade",
        "--messages",
        "1",
        "--json",
    ]));
    assert_stream(command(&[
        "crypto",
        "stream",
        SYMBOL,
        "--instrument",
        "swap",
        "--kind",
        "mark-price",
        "--messages",
        "1",
        "--json",
    ]));
    assert_price(command(&["price", SYMBOL, "--asset", "crypto", "--json"]));
    assert_history(command(&[
        "history",
        SYMBOL,
        "--asset",
        "crypto",
        "--interval",
        "1m",
        "--limit",
        "2",
        "--json",
    ]));
}

#[test]
#[ignore = "requires AGENT_FINANCE_LIVE_CRYPTO_PROVIDERS=1 and live Coinbase/OKX/CoinGecko network access"]
fn crypto_provider_live_cli_surface_is_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_CRYPTO_PROVIDERS")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping live multi-provider crypto test; set AGENT_FINANCE_LIVE_CRYPTO_PROVIDERS=1"
        );
        return;
    }

    assert_evidence(
        command(&[
            "crypto",
            "quote",
            "BTC-USD",
            "--provider",
            "coinbase",
            "--instrument",
            "spot",
            "--json",
        ]),
        "coinbase",
    );
    assert_evidence(
        command(&[
            "crypto",
            "quote",
            "BTC/USDT",
            "--provider",
            "okx",
            "--instrument",
            "swap",
            "--json",
        ]),
        "okx",
    );
    assert_evidence(
        command(&[
            "crypto",
            "quote",
            "bitcoin",
            "--provider",
            "coingecko",
            "--instrument",
            "spot",
            "--json",
        ]),
        "coingecko",
    );

    assert_payload_len_at_most(
        command(&[
            "crypto",
            "candles",
            "BTC-USD",
            "--provider",
            "coinbase",
            "--instrument",
            "spot",
            "--interval",
            "1m",
            "--limit",
            "2",
            "--json",
        ]),
        "candles",
        2,
    );
    assert_payload_len_at_most(
        command(&[
            "crypto",
            "candles",
            "bitcoin",
            "--provider",
            "coingecko",
            "--instrument",
            "spot",
            "--interval",
            "1d",
            "--limit",
            "2",
            "--json",
        ]),
        "ohlc",
        2,
    );

    let human = command_text(&["crypto", "quote", "BTC-USD", "--provider", "coinbase"]);
    assert!(
        human.lines().count() < 40,
        "human output should summarize payloads instead of dumping JSON: {human}"
    );
    assert!(
        human.contains("payload: object fields="),
        "human output should describe payload shape: {human}"
    );

    let raw = command_text(&[
        "crypto",
        "quote",
        "BTC-USD",
        "--provider",
        "coinbase",
        "--raw",
    ]);
    assert!(
        raw.lines().count() > human.lines().count(),
        "raw output should include provider payloads"
    );
}

#[test]
#[ignore = "requires AGENT_FINANCE_LIVE_BINANCE_SIGNED=1 and exported live Binance HMAC env vars"]
fn binance_live_signed_read_only_surface_is_usable() {
    if std::env::var("AGENT_FINANCE_LIVE_BINANCE_SIGNED")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping live signed Binance read-only test; set AGENT_FINANCE_LIVE_BINANCE_SIGNED=1"
        );
        return;
    }
    require_binance_hmac_env();

    let env = SignedProfileEnv::new("live-binance");
    env.write_profile("live-binance", SignedProfileEnvironment::Live);

    let doctor = env.command_json(&["profile", "doctor", "--profile", "live-binance", "--json"]);
    assert_check_ok(&doctor, "profile-parse");
    assert_check_ok(&doctor, "api-key-env");
    assert_check_ok(&doctor, "api-secret-env");
    assert_check_ok(&doctor, "binance-permissions");

    let permissions = env.command_json(&[
        "account",
        "permissions",
        "--profile",
        "live-binance",
        "--json",
    ]);
    for key in [
        "enableReading",
        "enableSpotAndMarginTrading",
        "enableFutures",
        "permitsUniversalTransfer",
    ] {
        assert!(
            permissions.get(key).is_some(),
            "permissions payload should include {key}"
        );
    }

    let balances =
        env.command_json(&["account", "balances", "--profile", "live-binance", "--json"]);
    assert!(
        balances["balances"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "spot account should include balances array"
    );

    let positions = env.command_json(&[
        "account",
        "positions",
        "--profile",
        "live-binance",
        "--json",
    ]);
    assert!(
        positions["assets"].as_array().is_some(),
        "USD-M account should include assets array"
    );
    assert!(
        positions["positions"].as_array().is_some(),
        "USD-M account should include positions array"
    );
}

#[test]
#[ignore = "requires AGENT_FINANCE_TESTNET_BINANCE_SIGNED=1 and exported Binance testnet HMAC env vars"]
fn binance_testnet_signed_order_test_surface_is_usable() {
    if std::env::var("AGENT_FINANCE_TESTNET_BINANCE_SIGNED")
        .ok()
        .as_deref()
        != Some("1")
    {
        eprintln!(
            "skipping Binance testnet signed order-test smoke; set AGENT_FINANCE_TESTNET_BINANCE_SIGNED=1"
        );
        return;
    }
    require_binance_hmac_env();

    let env = SignedProfileEnv::new("testnet-binance");
    env.write_profile("testnet-binance", SignedProfileEnvironment::Testnet);

    let intent = env.command_json(&[
        "order",
        "intent",
        "BTCUSDT",
        "--profile",
        "testnet-binance",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "limit",
        "--quantity",
        "0.0001",
        "--price",
        "50000",
        "--time-in-force",
        "gtc",
        "--json",
    ]);
    assert_eq!(intent["risk"]["allowed"], true);
    let intent_id = intent["intent"]["id"]
        .as_str()
        .expect("order intent should have id");

    let submit = env.command_json(&[
        "order",
        "submit",
        intent_id,
        "--profile",
        "testnet-binance",
        "--test",
        "--json",
    ]);
    assert_eq!(submit["risk"]["allowed"], true);
    assert!(
        submit["response"]["exchange_rules"]["allowed"]
            .as_bool()
            .unwrap_or(false),
        "exchangeInfo rule preflight should allow the order-test"
    );
}

fn command(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-finance"))
        .args(args)
        .output()
        .expect("agent-finance command should start");
    assert!(
        output.status.success(),
        "command failed: args={args:?} stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command should print JSON")
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

fn assert_check_ok(report: &serde_json::Value, name: &str) {
    let checks = report["checks"].as_array().expect("doctor checks");
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == name && check["ok"] == true),
        "doctor check {name} should pass"
    );
}

enum SignedProfileEnvironment {
    Live,
    Testnet,
}

struct SignedProfileEnv {
    root: PathBuf,
    data_home: PathBuf,
}

impl SignedProfileEnv {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "agent-finance-signed-{name}-{}-{nanos}",
            std::process::id(),
        ));
        fs::create_dir_all(&root).expect("signed profile test root");
        let data_home = root.join("data");
        Self { root, data_home }
    }

    fn write_profile(&self, profile: &str, environment: SignedProfileEnvironment) {
        let profile_dir = self.root.join("config/agent-finance/profiles");
        fs::create_dir_all(&profile_dir).expect("profile dir");
        let content = signed_profile_toml(profile, environment);
        fs::write(profile_dir.join(format!("{profile}.toml")), content).expect("profile write");
    }

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
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_DATA_HOME", &self.data_home)
            .args(args)
            .output()
            .expect("agent-finance command should start")
    }
}

impl Drop for SignedProfileEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn signed_profile_toml(profile: &str, environment: SignedProfileEnvironment) -> String {
    let provider_block = match environment {
        SignedProfileEnvironment::Live => {
            r#"[provider]
provider = "binance"
environment = "live"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_PRIVATE_KEY"
"#
        }
        SignedProfileEnvironment::Testnet => {
            r#"[provider]
provider = "binance"
environment = "testnet"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_PRIVATE_KEY"
spot_base_url = "https://testnet.binance.vision"
usds_futures_base_url = "https://testnet.binancefuture.com"
"#
        }
    };
    format!(
        r#"name = "{profile}"

{provider_block}
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

[[risk.allowed_futures_state_changes]]
kind = "position-mode"
mode = "hedge"

[risk.allowed_symbols.BTCUSDT]
markets = ["spot", "usds-futures"]
order_kinds = ["market", "limit", "limit-maker"]
max_order_notional_usdt = "25"
"#
    )
}

fn assert_aggregate(json: serde_json::Value, required_key: &str) {
    assert_eq!(json["symbol"], SYMBOL);
    assert!(
        json["spot"].get(required_key).is_some() || json["futures"].get(required_key).is_some(),
        "aggregate should include required key {required_key}: {json}"
    );
}

fn assert_evidence(json: serde_json::Value, provider: &str) {
    let results = json["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["provider"], provider);
    assert!(
        results[0]["ok"].as_bool().unwrap(),
        "provider evidence should be successful: {json}"
    );
}

fn assert_payload_len_at_most(json: serde_json::Value, endpoint: &str, limit: usize) {
    let endpoints = json["results"][0]["endpoints"].as_array().unwrap();
    let payload = endpoints
        .iter()
        .find(|value| value["endpoint"] == endpoint)
        .and_then(|value| value["payload"].as_array())
        .unwrap_or_else(|| panic!("missing array payload for endpoint {endpoint}: {json}"));
    assert!(
        payload.len() <= limit,
        "payload should honor limit={limit}: {json}"
    );
}

fn assert_stream(json: serde_json::Value) {
    assert_eq!(json["symbol"], SYMBOL);
    assert!(
        !json["messages"].as_array().unwrap().is_empty(),
        "stream should contain at least one message"
    );
}

fn assert_price(json: serde_json::Value) {
    assert!(json["errors"].as_object().unwrap().is_empty());
    let quote = &json["points"].as_array().unwrap()[0];
    assert_eq!(quote["symbol"], SYMBOL);
    assert_eq!(quote["provider"], "binance-spot");
    assert!(quote["price"].as_f64().unwrap() > 0.0);
}

fn assert_history(json: serde_json::Value) {
    assert_eq!(json["symbol"], SYMBOL);
    assert_eq!(json["provider"], "binance-spot");
    assert_eq!(json["bars"].as_array().unwrap().len(), 2);
}

fn command_text(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-finance"))
        .args(args)
        .output()
        .expect("agent-finance command should start");
    assert!(
        output.status.success(),
        "command failed: args={args:?} stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command should print UTF-8")
}

fn one_shot_error_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 4096];
        let read = stream.read(&mut buffer).unwrap();
        let request = String::from_utf8_lossy(&buffer[..read]);
        assert!(
            request.starts_with("GET /api/v3/ticker/price?symbol=BAD "),
            "request was {request:?}"
        );
        let body = r#"{"code":-1121,"msg":"Invalid symbol."}"#;
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    format!("http://{address}")
}
