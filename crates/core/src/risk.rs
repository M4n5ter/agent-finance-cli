use std::fmt;

use serde::{Deserialize, Serialize};

use crate::profile::Profile;
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskDecision {
    pub allowed: bool,
    pub findings: Vec<RiskFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFinding {
    pub severity: RiskSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilePermissionPolicyCheck {
    pub name: &'static str,
    pub ok: bool,
    pub required: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskSeverity {
    Info,
    Block,
}

impl fmt::Display for RiskSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => formatter.write_str("info"),
            Self::Block => formatter.write_str("block"),
        }
    }
}

impl RiskDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            findings: Vec::new(),
        }
    }

    fn push_info(&mut self, code: &str, message: impl Into<String>) {
        self.findings.push(RiskFinding {
            severity: RiskSeverity::Info,
            code: code.to_string(),
            message: message.into(),
        });
    }

    fn push_block(&mut self, code: &str, message: impl Into<String>) {
        self.allowed = false;
        self.findings.push(RiskFinding {
            severity: RiskSeverity::Block,
            code: code.to_string(),
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderRuntimeRisk {
    pub daily_order_notional_used_utc: Option<DecimalValue>,
}

pub fn check_order_intent(profile: &Profile, intent: &OrderIntent, live: bool) -> RiskDecision {
    let mut decision = RiskDecision::allow();
    check_profile(
        &mut decision,
        profile,
        &intent.profile,
        intent.provider,
        intent.environment,
        live,
    );

    let symbol_key = intent.symbol.to_ascii_uppercase();
    check_required_permissions(
        &mut decision,
        profile,
        intent.required_profile_permissions(),
    );
    let Some(symbol_policy) =
        check_symbol_market(&mut decision, profile, &symbol_key, intent.market)
    else {
        return decision;
    };
    let kind = intent.spec.kind();
    if !symbol_policy.order_kinds.contains(&kind) {
        decision.push_block(
            "order-kind-not-allowed",
            format!("order kind {kind} is not allowed for {symbol_key}"),
        );
    }
    if live && matches!(intent.spec, OrderSpec::Market { .. }) {
        decision.push_block(
            "live-market-order-notional-untrusted",
            "live market orders are blocked until risk notional can be derived from fresh exchange data instead of local valuation_price",
        );
    }
    let Some(notional) = intent.notional_usdt().map(|value| value.0) else {
        decision.push_block("order-notional-overflow", "order notional overflowed");
        return decision;
    };
    if notional > symbol_policy.max_order_notional_usdt.0 {
        decision.push_block(
            "order-notional-too-high",
            format!(
                "order notional {notional} exceeds max {}",
                symbol_policy.max_order_notional_usdt
            ),
        );
    }
    decision
}

pub fn check_order_intent_with_runtime(
    profile: &Profile,
    intent: &OrderIntent,
    live: bool,
    runtime: &OrderRuntimeRisk,
) -> RiskDecision {
    let mut decision = check_order_intent(profile, intent, live);
    if live {
        apply_daily_order_limit(&mut decision, profile, intent, runtime);
    }
    decision
}

pub fn check_cancel_intent(profile: &Profile, intent: &CancelIntent, live: bool) -> RiskDecision {
    let mut decision = RiskDecision::allow();
    check_profile(
        &mut decision,
        profile,
        &intent.profile,
        intent.provider,
        intent.environment,
        live,
    );
    check_symbol_market(
        &mut decision,
        profile,
        &intent.symbol.to_ascii_uppercase(),
        intent.market,
    );
    check_required_permissions(
        &mut decision,
        profile,
        intent.required_profile_permissions(),
    );
    decision
}

fn apply_daily_order_limit(
    decision: &mut RiskDecision,
    profile: &Profile,
    intent: &OrderIntent,
    runtime: &OrderRuntimeRisk,
) {
    let Some(max_daily) = &profile.risk.max_daily_order_notional_usdt else {
        return;
    };
    let Some(used) = &runtime.daily_order_notional_used_utc else {
        decision.push_block(
            "daily-order-notional-runtime-missing",
            "daily order notional runtime usage is required for live order checks",
        );
        return;
    };
    let Some(current_order) = intent.notional_usdt() else {
        decision.push_block("order-notional-overflow", "order notional overflowed");
        return;
    };
    let Some(used_after) = used.checked_add(&current_order) else {
        decision.push_block(
            "daily-order-notional-overflow",
            "daily order notional overflowed",
        );
        return;
    };
    if used_after.0 > max_daily.0 {
        decision.push_block(
            "daily-order-notional-too-high",
            format!(
                "daily live order notional would become {used_after}, exceeding max {max_daily}"
            ),
        );
    }
}

pub fn check_transfer_intent(
    profile: &Profile,
    intent: &TransferIntent,
    live: bool,
) -> RiskDecision {
    let mut decision = RiskDecision::allow();
    check_profile(
        &mut decision,
        profile,
        &intent.profile,
        intent.provider,
        intent.environment,
        live,
    );
    if live && intent.environment != Environment::Live {
        decision.push_block(
            "transfer-live-environment-required",
            "signed transfer is a live-only capability; use a live profile after reviewing the intent",
        );
    }
    check_required_permissions(
        &mut decision,
        profile,
        intent.required_profile_permissions(),
    );
    let asset = intent.asset.to_ascii_uppercase();
    let Some(policy) = profile.risk.allowed_transfers.iter().find(|policy| {
        policy.direction == intent.direction && policy.asset.to_ascii_uppercase() == asset
    }) else {
        decision.push_block(
            "transfer-not-allowed",
            format!(
                "transfer {} {} is not allowed by profile risk.allowed_transfers",
                intent.direction, asset
            ),
        );
        return decision;
    };
    if intent.amount.0 > policy.max_amount.0 {
        decision.push_block(
            "transfer-amount-too-high",
            format!(
                "transfer amount {} exceeds max {} for {} {}",
                intent.amount, policy.max_amount, intent.direction, asset
            ),
        );
    }
    decision
}

pub fn check_futures_state_intent(
    profile: &Profile,
    intent: &FuturesStateIntent,
    live: bool,
) -> RiskDecision {
    let mut decision = RiskDecision::allow();
    check_profile(
        &mut decision,
        profile,
        &intent.profile,
        intent.provider,
        intent.environment,
        live,
    );
    if live && intent.environment != Environment::Live {
        decision.push_block(
            "state-live-environment-required",
            "signed futures state changes require a live profile after reviewing the intent",
        );
    }
    if let FuturesStateChange::Leverage { leverage, .. } = intent.change
        && !(1..=125).contains(&leverage)
    {
        decision.push_block(
            "futures-leverage-out-of-range",
            format!("requested leverage {leverage} is outside Binance USD-M range 1..=125"),
        );
    }
    check_required_permissions(
        &mut decision,
        profile,
        intent.required_profile_permissions(),
    );
    if matches!(intent.change, FuturesStateChange::PositionMode { .. }) {
        decision.push_info(
            "futures-position-mode-account-wide",
            "Binance position mode changes every symbol and UM/CM share dualSidePosition; Binance rejects the change if either side has open orders or open positions",
        );
    }
    let matching_policies = matching_futures_state_policies(profile, intent);
    if matching_policies.is_empty() {
        decision.push_block(
            "futures-state-change-not-allowed",
            format!(
                "futures state change {} for {} is not allowed by profile risk.allowed_futures_state_changes",
                intent.change_kind(),
                intent.scope_label()
            ),
        );
        return decision;
    };
    match intent.change {
        FuturesStateChange::Leverage { leverage, .. } => {
            let max_allowed = matching_policies
                .iter()
                .filter_map(|policy| policy.max_leverage())
                .max()
                .expect("leverage policies must match leverage intents");
            if leverage > max_allowed {
                decision.push_block(
                    "futures-leverage-too-high",
                    format!("requested leverage {leverage} exceeds max {max_allowed}"),
                );
            }
        }
        FuturesStateChange::MarginType { margin_type, .. } => {
            if !matching_policies
                .iter()
                .any(|policy| policy.allows_change(&intent.change))
            {
                decision.push_block(
                    "futures-margin-type-not-allowed",
                    format!(
                        "requested margin type {margin_type} is not allowed by matching policies"
                    ),
                );
            }
        }
        FuturesStateChange::PositionMode { mode } => {
            if !matching_policies
                .iter()
                .any(|policy| policy.allows_change(&intent.change))
            {
                decision.push_block(
                    "futures-position-mode-not-allowed",
                    format!("requested position mode {mode} is not allowed by matching policies"),
                );
            }
        }
    }
    decision
}

fn check_required_permissions(
    decision: &mut RiskDecision,
    profile: &Profile,
    permissions: ProfilePermissionSet,
) {
    for permission in permissions.iter() {
        if !profile.permissions.allows(permission) {
            decision.push_block(
                permission.disabled_code(),
                format!("profile permissions.{} is false", permission.field_name()),
            );
        }
    }
}

pub fn check_profile_permission_policy(profile: &Profile) -> Vec<ProfilePermissionPolicyCheck> {
    let required = profile.risk.required_profile_permissions();
    ProfilePermission::ALL
        .into_iter()
        .map(|permission| {
            profile_permission_policy_check(
                permission,
                required.contains(permission),
                profile.permissions.allows(permission),
            )
        })
        .collect()
}

fn profile_permission_policy_check(
    permission: ProfilePermission,
    required: bool,
    declared: bool,
) -> ProfilePermissionPolicyCheck {
    let ok = !required || declared;
    let message = match (required, declared) {
        (false, true) => "permission is declared but not required by risk policy".to_string(),
        (false, false) => "permission is not declared and not required by risk policy".to_string(),
        (true, true) => "permission is declared and required by risk policy".to_string(),
        (true, false) => {
            format!(
                "permission is required by risk policy but not declared; {}",
                permission.policy_reason()
            )
        }
    };
    ProfilePermissionPolicyCheck {
        name: permission.policy_check_name(),
        ok,
        required,
        message,
    }
}

fn matching_futures_state_policies<'a>(
    profile: &'a Profile,
    intent: &FuturesStateIntent,
) -> Vec<&'a FuturesStatePolicy> {
    profile
        .risk
        .allowed_futures_state_changes
        .iter()
        .filter(|policy| policy.matches_change_scope(&intent.change))
        .collect()
}

fn check_profile(
    decision: &mut RiskDecision,
    profile: &Profile,
    intent_profile: &str,
    provider: Provider,
    environment: Environment,
    live: bool,
) {
    if profile.name != intent_profile {
        decision.push_block(
            "profile-mismatch",
            format!(
                "intent profile '{}' does not match selected profile '{}'",
                intent_profile, profile.name
            ),
        );
    }
    if live && !profile.risk.allow_live {
        decision.push_block("live-disabled", "profile risk.allow_live is false");
    }
    if profile.provider.provider != provider {
        decision.push_block(
            "provider-mismatch",
            "intent provider does not match profile",
        );
    }
    if profile.provider.environment != environment {
        decision.push_block(
            "environment-mismatch",
            "intent environment does not match profile",
        );
    }
}

fn check_symbol_market<'a>(
    decision: &mut RiskDecision,
    profile: &'a Profile,
    symbol_key: &str,
    market: Market,
) -> Option<&'a SymbolPolicy> {
    let Some(symbol_policy) = profile.risk.allowed_symbols.get(symbol_key) else {
        decision.push_block(
            "symbol-not-allowed",
            format!("symbol {symbol_key} is not in profile risk.allowed_symbols"),
        );
        return None;
    };
    if !symbol_policy.markets.contains(&market) {
        decision.push_block(
            "market-not-allowed",
            format!("market {market} is not allowed for {symbol_key}"),
        );
    }
    Some(symbol_policy)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rust_decimal::Decimal;

    use super::*;

    #[test]
    fn profile_permission_policy_checks_are_core_owned() {
        let mut profile = profile();
        profile.permissions.spot_trading = false;

        let checks = check_profile_permission_policy(&profile);
        let spot = check(&checks, "profile-permission-spot-trading");
        assert!(spot.required);
        assert!(!spot.ok);
        assert!(spot.message.contains("risk.allowed_symbols"));
    }

    #[test]
    fn missing_symbol_does_not_hide_missing_profile_permission() {
        let mut profile = profile();
        profile.permissions.spot_trading = false;
        let mut intent = order_intent();
        intent.symbol = "ETHUSDT".to_string();

        let decision = check_order_intent(&profile, &intent, false);

        assert!(!decision.allowed);
        assert_finding(&decision, "profile-permission-spot-trading-disabled");
        assert_finding(&decision, "symbol-not-allowed");
    }

    fn check<'a>(
        checks: &'a [ProfilePermissionPolicyCheck],
        name: &str,
    ) -> &'a ProfilePermissionPolicyCheck {
        checks.iter().find(|check| check.name == name).expect(name)
    }

    fn assert_finding(decision: &RiskDecision, code: &str) {
        assert!(
            decision.findings.iter().any(|finding| finding.code == code),
            "expected finding {code}: {:?}",
            decision.findings
        );
    }

    fn profile() -> Profile {
        Profile {
            name: "test".to_string(),
            provider: ProviderConfig {
                provider: Provider::Binance,
                environment: Environment::Testnet,
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
                allow_live: false,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::from([(
                    "BTCUSDT".to_string(),
                    SymbolPolicy {
                        markets: vec![Market::Spot],
                        order_kinds: vec![OrderKind::Limit],
                        max_order_notional_usdt: decimal("10"),
                    },
                )]),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: Vec::new(),
            },
        }
    }

    fn order_intent() -> OrderIntent {
        OrderIntent {
            profile: "test".to_string(),
            provider: Provider::Binance,
            environment: Environment::Testnet,
            market: Market::Spot,
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            quantity: decimal("0.0001"),
            spec: OrderSpec::Limit {
                price: decimal("50000"),
                time_in_force: TimeInForce::Gtc,
            },
            reduce_only: false,
            position_side: None,
            client_order_id: "af-test".to_string(),
        }
    }

    fn decimal(value: &str) -> DecimalValue {
        DecimalValue(value.parse::<Decimal>().expect("decimal"))
    }
}
