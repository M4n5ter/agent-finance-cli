use std::fs;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use serde_json::Value;

#[test]
fn order_intent_can_be_risk_checked_and_dry_run_repeatedly() {
    let env = default_env("order-flow");
    let order = create_limit_order(&env);
    assert_eq!(order["risk"]["allowed"], true);
    let order_id = order["intent"]["id"].as_str().expect("order intent id");

    let risk = env.json(command(&[
        "risk",
        "check",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(risk["allowed"], true);

    let submit = env.json(command(&[
        "order",
        "submit",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(submit["response"]["dry_run"], true);
    assert_eq!(submit["response"]["request"]["method"], "POST");

    let second_plan = env.json(command(&[
        "order",
        "submit",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(second_plan["response"]["dry_run"], true);

    let audit = env.json(command(&["audit", "tail", "--limit", "10", "--json"]));
    let events = audit.as_array().expect("audit events");
    assert!(
        events.iter().any(|event| event["kind"] == "intent-created"),
        "audit should include intent-created events"
    );
    assert!(
        events.iter().any(|event| event["kind"] == "dry-run"),
        "audit should include dry-run events"
    );
}

#[test]
fn profile_permissions_are_live_policy_not_just_doctor_metadata() {
    let env = default_env("profile-permissions");
    env.replace_once_in_profile("default", "spot_trading = true", "spot_trading = false");

    let order = create_limit_order(&env);
    assert_eq!(order["risk"]["allowed"], false);
    assert!(
        order["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "profile-permission-spot-trading-disabled"),
        "risk should block spot orders when profile permissions do not declare spot trading: {order}"
    );
    let cancel = env.json(command(&[
        "order",
        "cancel",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--client-order-id",
        "af-test",
        "--json",
    ]));
    assert_eq!(cancel["risk"]["allowed"], false);
    assert_risk_finding(&cancel, "profile-permission-spot-trading-disabled");

    let doctor = env.json(command(&[
        "profile",
        "doctor",
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        doctor["checks"]
            .as_array()
            .expect("doctor checks")
            .iter()
            .any(|check| check["name"] == "profile-permission-spot-trading"
                && check["ok"] == false),
        "doctor should report profile permission and risk policy mismatch: {doctor}"
    );
}

#[test]
fn missing_or_partial_profile_permissions_fail_closed_with_diagnostics() {
    let legacy_env = default_env("legacy-profile-permissions");
    legacy_env.edit_profile("default", |content| {
        content.replace(
            "[permissions]\nspot_trading = true\nusds_futures = true\nuniversal_transfer = false\n\n",
            "",
        )
    });

    let order = create_limit_order(&legacy_env);
    assert_eq!(order["risk"]["allowed"], false);
    assert_risk_finding(&order, "profile-permission-spot-trading-disabled");

    let doctor = legacy_env.json(command(&[
        "profile",
        "doctor",
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        doctor["checks"]
            .as_array()
            .expect("doctor checks")
            .iter()
            .any(|check| check["name"] == "profile-permission-spot-trading"
                && check["ok"] == false),
        "legacy profiles without [permissions] should parse and produce actionable doctor output: {doctor}"
    );

    let partial_env = default_env("partial-profile-permissions");
    partial_env.edit_profile("default", |content| {
        content.replace(
            "[permissions]\nspot_trading = true\nusds_futures = true\nuniversal_transfer = false",
            "[permissions]\nspot_trading = true",
        )
    });
    let state = partial_env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    assert_eq!(state["risk"]["allowed"], false);
    assert_risk_finding(&state, "profile-permission-usds-futures-disabled");
}

#[test]
fn transfer_and_futures_state_use_profile_permissions_as_live_guards() {
    let transfer_env = default_env("transfer-profile-permission");
    transfer_env.replace_once_in_profile(
        "default",
        "allowed_transfers = []",
        r#"
[[risk.allowed_transfers]]
direction = "spot-to-usds-futures"
asset = "USDT"
max_amount = "10"
"#
        .trim(),
    );
    let transfer = transfer_env.json(command(&[
        "transfer",
        "create",
        "USDT",
        "--profile",
        "default",
        "--direction",
        "spot-to-usds-futures",
        "--amount",
        "1",
        "--json",
    ]));
    assert_eq!(transfer["risk"]["allowed"], false);
    assert_risk_finding(&transfer, "profile-permission-universal-transfer-disabled");

    let state_env = default_env("state-profile-permission");
    state_env.replace_once_in_profile("default", "usds_futures = true", "usds_futures = false");
    let state = state_env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    assert_eq!(state["risk"]["allowed"], false);
    assert_risk_finding(&state, "profile-permission-usds-futures-disabled");
}

#[test]
fn cancel_test_failure_does_not_consume_intent() {
    let env = default_env("cancel-flow");
    let cancel = env.json(command(&[
        "order",
        "cancel",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--client-order-id",
        "af-test",
        "--json",
    ]));
    assert_eq!(cancel["risk"]["allowed"], true);
    let cancel_id = cancel["intent"]["id"].as_str().expect("cancel intent id");

    let cancel_submit = env.json(command(&[
        "order",
        "submit",
        cancel_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(cancel_submit["response"]["request"]["method"], "DELETE");

    let cancel_test = env.output(command(&[
        "order",
        "submit",
        cancel_id,
        "--profile",
        "default",
        "--test",
        "--json",
    ]));
    assert!(
        !cancel_test.status.success(),
        "cancel intent should not have an exchange test mode"
    );
    let cancel_submit_after_test_failure = env.json(command(&[
        "order",
        "submit",
        cancel_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(
        cancel_submit_after_test_failure["response"]["request"]["method"],
        "DELETE"
    );
}

#[test]
fn invalid_and_risk_blocked_orders_are_rejected_at_the_right_boundary() {
    let env = default_env("risk-boundaries");
    let blocked = env.output(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "market",
        "--quantity",
        "1",
        "--valuation-price",
        "50000",
        "--json",
    ]));
    assert!(
        blocked.status.success(),
        "risk-blocked intent can be created"
    );
    let blocked_json: Value = serde_json::from_slice(&blocked.stdout).expect("blocked intent json");
    let blocked_id = blocked_json["intent"]["id"]
        .as_str()
        .expect("blocked intent id");
    let blocked_submit = env.output(command(&[
        "order",
        "submit",
        blocked_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !blocked_submit.status.success(),
        "risk-blocked intent should not be submitted even as dry-run"
    );

    let invalid_limit = env.output(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "limit",
        "--quantity",
        "1",
        "--price",
        "50000",
        "--valuation-price",
        "1",
        "--time-in-force",
        "gtc",
        "--json",
    ]));
    assert!(
        !invalid_limit.status.success(),
        "limit order must not accept a separate valuation price"
    );
}

#[test]
fn market_order_uses_valuation_only_for_risk_and_test_is_non_consuming() {
    let env = default_env("market-order");
    let market = env.json(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "market",
        "--quantity",
        "0.0001",
        "--valuation-price",
        "50000",
        "--json",
    ]));
    assert_eq!(market["risk"]["allowed"], true);
    let market_id = market["intent"]["id"].as_str().expect("market intent id");
    let market_submit = env.json(command(&[
        "order",
        "submit",
        market_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !market_submit["response"]["request"]["params"]
            .as_array()
            .expect("request params")
            .iter()
            .any(|param| param[0] == "price"),
        "market dry-run should not send an exchange price"
    );
    assert_eq!(
        market_submit["response"]["exchange_rules"]["status"],
        "not-checked"
    );
    assert_eq!(
        market_submit["response"]["exchange_rules"]["request"]["url"],
        "https://testnet.binance.vision/api/v3/exchangeInfo"
    );

    let test_failure = env.output(command(&[
        "order",
        "submit",
        market_id,
        "--profile",
        "default",
        "--test",
        "--json",
    ]));
    assert!(
        !test_failure.status.success(),
        "test submit without credentials should fail"
    );
    let after_failed_test = env.json(command(&[
        "order",
        "submit",
        market_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(after_failed_test["response"]["dry_run"], true);
}

#[test]
fn spot_limit_maker_order_dry_run_uses_post_only_exchange_shape() {
    let env = default_env("limit-maker-order");
    let order = env.json(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "limit-maker",
        "--quantity",
        "0.0001",
        "--price",
        "50000",
        "--json",
    ]));
    assert_eq!(order["risk"]["allowed"], true);
    let order_id = order["intent"]["id"].as_str().expect("order intent id");

    let submit = env.json(command(&[
        "order",
        "submit",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    let params = submit["response"]["request"]["params"]
        .as_array()
        .expect("request params");
    assert!(
        params
            .iter()
            .any(|param| param[0] == "type" && param[1] == "LIMIT_MAKER"),
        "dry-run should map limit-maker to Binance LIMIT_MAKER: {submit}"
    );
    assert!(
        !params.iter().any(|param| param[0] == "timeInForce"),
        "Binance LIMIT_MAKER dry-run must not send timeInForce: {submit}"
    );
}

#[test]
fn order_query_requires_exactly_one_order_identifier_before_credentials() {
    let env = default_env("order-query");
    let missing_target = env.output(command(&[
        "order",
        "query",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--json",
    ]));
    let missing_stderr = String::from_utf8_lossy(&missing_target.stderr);
    assert!(
        missing_stderr.contains("requires order id or client order id"),
        "missing query target should fail before credential loading; stderr={missing_stderr}"
    );

    let query = env.output(command(&[
        "order",
        "query",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--client-order-id",
        "af-test",
        "--json",
    ]));
    let query_stderr = String::from_utf8_lossy(&query.stderr);
    assert!(
        query_stderr.contains("BINANCE_API_KEY"),
        "valid query target should progress to credential loading; stderr={query_stderr}"
    );
}

#[test]
fn profile_and_command_boundaries_are_enforced() {
    let env = default_env("profile-boundaries");
    let order = create_limit_order(&env);
    let order_id = order["intent"]["id"].as_str().expect("order intent id");

    env.write_profile("other");
    let profile_mismatch = env.json(command(&[
        "risk",
        "check",
        order_id,
        "--profile",
        "other",
        "--json",
    ]));
    assert_eq!(profile_mismatch["allowed"], false);
    assert!(
        profile_mismatch["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "profile-mismatch")
    );

    let transfer = env.json(command(&[
        "transfer",
        "create",
        "USDT",
        "--profile",
        "default",
        "--direction",
        "spot-to-usds-futures",
        "--amount",
        "1",
        "--json",
    ]));
    assert_eq!(transfer["risk"]["allowed"], false);
    assert!(
        transfer["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "transfer-not-allowed")
    );
    let transfer_id = transfer["intent"]["id"]
        .as_str()
        .expect("transfer intent id");
    let wrong_submit = env.output(command(&[
        "order",
        "submit",
        transfer_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !wrong_submit.status.success(),
        "order submit must reject transfer intents"
    );
    let wrong_live_submit = env.output(command(&[
        "order",
        "submit",
        transfer_id,
        "--profile",
        "default",
        "--live",
        "--json",
    ]));
    assert!(
        !wrong_live_submit.status.success(),
        "wrong live submit must be rejected"
    );
    let correct_transfer_submit = env.output(command(&[
        "transfer",
        "submit",
        transfer_id,
        "--profile",
        "default",
        "--json",
    ]));
    let stderr = String::from_utf8_lossy(&correct_transfer_submit.stderr);
    assert!(
        stderr.contains("risk policy blocked intent submit"),
        "wrong live submit should not consume the transfer intent; stderr={stderr}"
    );
}

#[test]
fn transfer_history_requires_live_profile_before_credentials() {
    let env = default_env("transfer-history");
    let testnet_history = env.output(command(&[
        "transfer",
        "history",
        "--profile",
        "default",
        "--direction",
        "spot-to-usds-futures",
        "--json",
    ]));
    let testnet_stderr = String::from_utf8_lossy(&testnet_history.stderr);
    assert!(
        testnet_stderr.contains("uses Binance SAPI live account data"),
        "testnet transfer history should fail at environment guard; stderr={testnet_stderr}"
    );

    env.write_live_profile("live");
    let live_history = env.output(command(&[
        "transfer",
        "history",
        "--profile",
        "live",
        "--direction",
        "spot-to-usds-futures",
        "--json",
    ]));
    let live_stderr = String::from_utf8_lossy(&live_history.stderr);
    assert!(
        live_stderr.contains("BINANCE_API_KEY"),
        "live transfer history should progress to credential loading; stderr={live_stderr}"
    );
}

#[test]
fn futures_state_intent_is_policy_checked_and_dry_runs() {
    let env = default_env("futures-state");
    let intent = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    assert_eq!(intent["risk"]["allowed"], true);
    let intent_id = intent["intent"]["id"].as_str().expect("state intent id");

    let risk = env.json(command(&[
        "risk",
        "check",
        intent_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(risk["allowed"], true);

    let plan = env.json(command(&[
        "state",
        "submit",
        intent_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(plan["response"]["dry_run"], true);
    assert_eq!(plan["response"]["request"]["method"], "POST");
    assert_eq!(
        plan["response"]["request"]["url"],
        "https://testnet.binancefuture.com/fapi/v1/leverage"
    );
    assert!(
        plan["response"]["request"]["params"]
            .as_array()
            .expect("params")
            .iter()
            .any(|param| param[0] == "leverage" && param[1] == "2")
    );

    let margin = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "margin-type",
        "--symbol",
        "BTCUSDT",
        "--margin-type",
        "isolated",
        "--json",
    ]));
    let margin_plan = env.json(command(&[
        "state",
        "submit",
        margin["intent"]["id"].as_str().expect("margin intent id"),
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(
        margin_plan["response"]["request"]["url"],
        "https://testnet.binancefuture.com/fapi/v1/marginType"
    );
    assert!(
        margin_plan["response"]["request"]["params"]
            .as_array()
            .expect("params")
            .iter()
            .any(|param| param[0] == "marginType" && param[1] == "ISOLATED")
    );

    env.append_profile_toml(
        "default",
        r#"
[[risk.allowed_futures_state_changes]]
kind = "position-mode"
mode = "hedge"
"#,
    );
    let position_mode = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    assert_eq!(position_mode["risk"]["allowed"], true);
    assert!(
        position_mode["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-position-mode-account-wide")
    );
    let position_plan = env.json(command(&[
        "state",
        "submit",
        position_mode["intent"]["id"]
            .as_str()
            .expect("position mode intent id"),
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(
        position_plan["response"]["request"]["url"],
        "https://testnet.binancefuture.com/fapi/v1/positionSide/dual"
    );
    assert!(
        position_plan["response"]["request"]["params"]
            .as_array()
            .expect("params")
            .iter()
            .any(|param| param[0] == "dualSidePosition" && param[1] == "true")
    );
    let position_text = env.output(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
    ]));
    assert!(
        position_text.status.success(),
        "text state intent should succeed"
    );
    let stdout = String::from_utf8_lossy(&position_text.stdout);
    assert!(
        stdout.contains("risk findings:") && stdout.contains("futures-position-mode-account-wide"),
        "text state intent output should show account-wide risk findings; stdout={stdout}"
    );
    let submit_text = env.output(command(&[
        "state",
        "submit",
        position_mode["intent"]["id"]
            .as_str()
            .expect("position mode intent id"),
        "--profile",
        "default",
    ]));
    assert!(
        submit_text.status.success(),
        "text state submit should succeed"
    );
    let stdout = String::from_utf8_lossy(&submit_text.stdout);
    assert!(
        stdout.contains("risk findings:") && stdout.contains("futures-position-mode-account-wide"),
        "text state submit output should show account-wide risk findings; stdout={stdout}"
    );

    let audit = env.json(command(&["audit", "tail", "--limit", "10", "--json"]));
    assert!(
        audit
            .as_array()
            .expect("audit events")
            .iter()
            .any(|event| event["kind"] == "dry-run")
    );
}

#[test]
fn futures_state_policy_and_argument_boundaries_are_enforced() {
    let env = default_env("futures-state-boundaries");
    let excessive_leverage = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "3",
        "--json",
    ]));
    assert_eq!(excessive_leverage["risk"]["allowed"], false);
    assert!(
        excessive_leverage["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-leverage-too-high")
    );

    let out_of_range_leverage = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "126",
        "--json",
    ]));
    assert_eq!(out_of_range_leverage["risk"]["allowed"], false);
    assert!(
        out_of_range_leverage["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-leverage-out-of-range")
    );

    let not_allowed = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "margin-type",
        "--symbol",
        "ETHUSDT",
        "--margin-type",
        "isolated",
        "--json",
    ]));
    assert_eq!(not_allowed["risk"]["allowed"], false);
    assert!(
        not_allowed["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-state-change-not-allowed")
    );

    let cross_margin = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "margin-type",
        "--symbol",
        "BTCUSDT",
        "--margin-type",
        "cross",
        "--json",
    ]));
    assert_eq!(cross_margin["risk"]["allowed"], false);
    assert!(
        cross_margin["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-margin-type-not-allowed")
    );

    let position_mode_with_symbol = env.output(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--symbol",
        "BTCUSDT",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    assert!(
        !position_mode_with_symbol.status.success(),
        "position-mode is account-scoped and must reject symbol-scoped arguments"
    );

    let missing_position_policy = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    assert_eq!(missing_position_policy["risk"]["allowed"], false);
    assert!(
        missing_position_policy["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(
                |finding| finding["code"] == "futures-state-change-not-allowed"
                    && finding["message"]
                        .as_str()
                        .expect("message")
                        .contains("binance-futures-account")
            )
    );

    env.append_profile_toml(
        "default",
        r#"
[[risk.allowed_futures_state_changes]]
kind = "position-mode"
mode = "hedge"
"#,
    );
    let blocked_one_way = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "one-way",
        "--json",
    ]));
    assert_eq!(blocked_one_way["risk"]["allowed"], false);
    assert!(
        blocked_one_way["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-position-mode-not-allowed")
    );

    let allowed_hedge = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    assert_eq!(allowed_hedge["risk"]["allowed"], true);
    let hedge_plan = env.json(command(&[
        "state",
        "submit",
        allowed_hedge["intent"]["id"]
            .as_str()
            .expect("hedge position mode intent id"),
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        hedge_plan["response"]["request"]["params"]
            .as_array()
            .expect("params")
            .iter()
            .any(|param| param[0] == "dualSidePosition" && param[1] == "true")
    );

    env.replace_once_in_profile("default", "mode = \"hedge\"", "mode = \"one-way\"");
    let blocked_hedge = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    assert_eq!(blocked_hedge["risk"]["allowed"], false);
    assert!(
        blocked_hedge["risk"]["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "futures-position-mode-not-allowed")
    );

    let allowed_one_way = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "one-way",
        "--json",
    ]));
    assert_eq!(allowed_one_way["risk"]["allowed"], true);
    let one_way_plan = env.json(command(&[
        "state",
        "submit",
        allowed_one_way["intent"]["id"]
            .as_str()
            .expect("one-way position mode intent id"),
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        one_way_plan["response"]["request"]["params"]
            .as_array()
            .expect("params")
            .iter()
            .any(|param| param[0] == "dualSidePosition" && param[1] == "false")
    );
}

#[test]
fn duplicate_futures_state_policies_are_order_independent() {
    let env = default_env("futures-state-duplicate-policies");
    env.replace_once_in_profile("default", "max_leverage = 2", "max_leverage = 1");
    env.append_profile_toml(
        "default",
        r#"
[[risk.allowed_futures_state_changes]]
kind = "leverage"
symbol = "BTCUSDT"
max_leverage = 2
"#,
    );
    env.replace_once_in_profile(
        "default",
        "margin_type = \"isolated\"",
        "margin_type = \"cross\"",
    );
    env.append_profile_toml(
        "default",
        r#"
[[risk.allowed_futures_state_changes]]
kind = "margin-type"
symbol = "BTCUSDT"
margin_type = "isolated"
"#,
    );

    let leverage = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    assert_eq!(
        leverage["risk"]["allowed"], true,
        "a later matching leverage policy should allow the request"
    );

    let margin = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "margin-type",
        "--symbol",
        "BTCUSDT",
        "--margin-type",
        "isolated",
        "--json",
    ]));
    assert_eq!(
        margin["risk"]["allowed"], true,
        "a later matching margin policy should allow the request"
    );
}

#[test]
fn malformed_futures_state_policy_fails_closed_at_profile_parse() {
    let env = default_env("futures-state-profile-parse");
    env.replace_once_in_profile("default", "max_leverage = 2", "max_leverage_typo = 2");

    let output = env.output(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "malformed futures state policy must fail before creating an intent"
    );
    assert!(
        stderr.contains("max_leverage") || stderr.contains("unknown field"),
        "malformed futures state policy should fail before creating an intent; stderr={stderr}"
    );

    let scoped_position_env = default_env("futures-state-position-policy-parse");
    scoped_position_env.append_profile_toml(
        "default",
        r#"
[[risk.allowed_futures_state_changes]]
kind = "position-mode"
symbol = "BTCUSDT"
mode = "hedge"
"#,
    );
    let scoped_position = scoped_position_env.output(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "position-mode",
        "--position-mode",
        "hedge",
        "--json",
    ]));
    let stderr = String::from_utf8_lossy(&scoped_position.stderr);
    assert!(
        !scoped_position.status.success(),
        "position-mode policy must reject symbol-scoped fields"
    );
    assert!(
        stderr.contains("symbol") || stderr.contains("unknown field"),
        "position-mode policy with symbol should fail at profile parse; stderr={stderr}"
    );
}

#[test]
fn futures_state_submit_boundaries_do_not_consume_intents() {
    let env = default_env("futures-state-submit-boundaries");
    let state = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "2",
        "--json",
    ]));
    let state_id = state["intent"]["id"].as_str().expect("state intent id");

    let wrong_order_submit = env.output(command(&[
        "order",
        "submit",
        state_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !wrong_order_submit.status.success(),
        "order submit must reject futures state intents"
    );

    let wrong_transfer_submit = env.output(command(&[
        "transfer",
        "submit",
        state_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !wrong_transfer_submit.status.success(),
        "transfer submit must reject futures state intents"
    );

    let state_plan = env.json(command(&[
        "state",
        "submit",
        state_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(state_plan["response"]["dry_run"], true);

    let order = create_limit_order(&env);
    let order_id = order["intent"]["id"].as_str().expect("order intent id");
    let wrong_state_submit = env.output(command(&[
        "state",
        "submit",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert!(
        !wrong_state_submit.status.success(),
        "state submit must reject order intents"
    );

    let order_plan = env.json(command(&[
        "order",
        "submit",
        order_id,
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(order_plan["response"]["dry_run"], true);
}

#[test]
fn blocked_live_futures_state_submit_does_not_claim_intent() {
    let env = default_env("blocked-live-futures-state");
    let state = env.json(command(&[
        "state",
        "create",
        "--profile",
        "default",
        "--kind",
        "leverage",
        "--symbol",
        "BTCUSDT",
        "--leverage",
        "3",
        "--json",
    ]));
    let state_id = state["intent"]["id"].as_str().expect("state intent id");
    let path = state["path"].as_str().expect("intent path");

    let live_submit = env.output(command(&[
        "state",
        "submit",
        state_id,
        "--profile",
        "default",
        "--live",
        "--json",
    ]));

    assert!(
        !live_submit.status.success(),
        "risk-blocked live state submit should fail before credentials or network"
    );
    let stderr = String::from_utf8_lossy(&live_submit.stderr);
    assert!(
        stderr.contains("risk policy blocked intent submit"),
        "live state submit should fail at pre-claim risk check; stderr={stderr}"
    );
    let saved: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("intent should still be readable"))
            .expect("saved intent json");
    assert_eq!(saved["metadata"]["status"], "created");
}

#[test]
fn live_risk_uses_audit_backed_daily_order_limit() {
    let env = TestEnv::new("daily-limit");
    env.write_live_profile("default");
    env.append_live_order_audit("49");
    let order = create_limit_order(&env);
    let order_id = order["intent"]["id"].as_str().expect("order intent id");

    let risk = env.json(command(&[
        "risk",
        "check",
        order_id,
        "--profile",
        "default",
        "--live",
        "--json",
    ]));

    assert_eq!(risk["allowed"], false);
    assert!(
        risk["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "daily-order-notional-too-high")
    );

    let explain = env.json(command(&[
        "risk",
        "explain",
        "--profile",
        "default",
        "--json",
    ]));
    assert_eq!(explain["daily_order_notional_used_utc"], "49");

    let export = env.json(command(&["audit", "export", "--json"]));
    assert_eq!(export.as_array().expect("audit export").len(), 2);
}

#[test]
fn live_market_orders_are_blocked_until_notional_uses_exchange_data() {
    let env = TestEnv::new("live-market-notional");
    env.write_live_profile("default");
    let order = env.json(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
        "--market",
        "spot",
        "--side",
        "buy",
        "--kind",
        "market",
        "--quantity",
        "0.0001",
        "--valuation-price",
        "50000",
        "--json",
    ]));
    let order_id = order["intent"]["id"].as_str().expect("order intent id");

    let live_risk = env.json(command(&[
        "risk",
        "check",
        order_id,
        "--profile",
        "default",
        "--live",
        "--json",
    ]));

    assert_eq!(live_risk["allowed"], false);
    assert!(
        live_risk["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "live-market-order-notional-untrusted")
    );
}

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-finance"));
    command.args(args);
    command
}

fn default_env(name: &str) -> TestEnv {
    let env = TestEnv::new(name);
    env.write_profile("default");
    env
}

fn create_limit_order(env: &TestEnv) -> Value {
    env.json(command(&[
        "order",
        "create",
        "BTCUSDT",
        "--profile",
        "default",
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
    ]))
}

fn assert_risk_finding(value: &Value, code: &str) {
    assert!(
        value["risk"]["findings"]
            .as_array()
            .expect("risk findings")
            .iter()
            .any(|finding| finding["code"] == code),
        "expected risk finding {code}: {value}"
    );
}

struct TestEnv {
    root: std::path::PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-finance-{name}-{nanos}"));
        fs::create_dir_all(&root).expect("test root");
        Self { root }
    }

    fn write_profile(&self, name: &str) {
        let profile_dir = self.root.join("config/agent-finance/profiles");
        fs::create_dir_all(&profile_dir).expect("profile dir");
        let output = self.output(command(&["profile", "template", "--profile", name]));
        assert!(output.status.success(), "profile template should succeed");
        fs::write(profile_dir.join(format!("{name}.toml")), output.stdout).expect("profile write");
    }

    fn edit_profile(&self, name: &str, edit: impl FnOnce(String) -> String) {
        let path = self
            .root
            .join("config/agent-finance/profiles")
            .join(format!("{name}.toml"));
        let content = fs::read_to_string(&path).expect("profile read");
        fs::write(path, edit(content)).expect("profile write");
    }

    fn replace_once_in_profile(&self, name: &str, needle: &str, replacement: &str) {
        self.edit_profile(name, |content| {
            assert!(
                content.contains(needle),
                "profile template should contain {needle:?}"
            );
            content.replacen(needle, replacement, 1)
        });
    }

    fn append_profile_toml(&self, name: &str, toml: &str) {
        self.edit_profile(name, |mut content| {
            content.push('\n');
            content.push_str(toml.trim());
            content.push('\n');
            content
        });
    }

    fn write_live_profile(&self, name: &str) {
        let profile_dir = self.root.join("config/agent-finance/profiles");
        fs::create_dir_all(&profile_dir).expect("profile dir");
        let output = self.output(command(&["profile", "template", "--profile", name]));
        assert!(output.status.success(), "profile template should succeed");
        let content = String::from_utf8(output.stdout)
            .expect("profile template")
            .replace("environment = \"testnet\"", "environment = \"live\"")
            .replace("allow_live = false", "allow_live = true");
        fs::write(profile_dir.join(format!("{name}.toml")), content).expect("profile write");
    }

    fn append_live_order_audit(&self, order_notional: &str) {
        let audit_dir = self.root.join("data/agent-finance/audit");
        fs::create_dir_all(&audit_dir).expect("audit dir");
        let event = serde_json::json!({
            "timestamp_utc": Utc::now().to_rfc3339(),
            "profile": "default",
            "provider": "binance",
            "environment": "Live",
            "intent_id": "seed",
            "kind": "live-submit",
            "summary": "seed live order",
            "payload": {
                "order_notional_usdt": order_notional,
                "response": {"seed": true}
            }
        });
        fs::write(audit_dir.join("events.jsonl"), format!("{event}\n")).expect("audit write");
    }

    fn json(&self, command: Command) -> Value {
        let output = self.output(command);
        assert!(
            output.status.success(),
            "command failed\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("json stdout")
    }

    fn output(&self, mut command: Command) -> Output {
        let config_home = self.root.join("config");
        let data_home = self.root.join("data");
        command
            .env("XDG_CONFIG_HOME", config_home)
            .env("XDG_DATA_HOME", data_home)
            .output()
            .expect("agent-finance command should start")
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
