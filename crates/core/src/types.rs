use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Binance,
}

impl fmt::Display for Provider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binance => formatter.write_str("binance"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Environment {
    Testnet,
    Live,
}

impl Environment {
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Testnet => formatter.write_str("testnet"),
            Self::Live => formatter.write_str("live"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Market {
    Spot,
    UsdsFutures,
}

impl Market {
    pub const fn required_profile_permission(self) -> ProfilePermission {
        match self {
            Self::Spot => ProfilePermission::SpotTrading,
            Self::UsdsFutures => ProfilePermission::UsdsFutures,
        }
    }
}

impl fmt::Display for Market {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spot => formatter.write_str("spot"),
            Self::UsdsFutures => formatter.write_str("usds-futures"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => formatter.write_str("buy"),
            Self::Sell => formatter.write_str("sell"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderKind {
    Market,
    Limit,
    #[serde(rename = "limit-maker")]
    PostOnlyLimit,
    StopLoss,
    TakeProfit,
}

impl fmt::Display for OrderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Market => formatter.write_str("market"),
            Self::Limit => formatter.write_str("limit"),
            Self::PostOnlyLimit => formatter.write_str("limit-maker"),
            Self::StopLoss => formatter.write_str("stop-loss"),
            Self::TakeProfit => formatter.write_str("take-profit"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gtc => formatter.write_str("GTC"),
            Self::Ioc => formatter.write_str("IOC"),
            Self::Fok => formatter.write_str("FOK"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PositionSide {
    Both,
    Long,
    Short,
}

impl fmt::Display for PositionSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Both => formatter.write_str("BOTH"),
            Self::Long => formatter.write_str("LONG"),
            Self::Short => formatter.write_str("SHORT"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarginType {
    Cross,
    Isolated,
}

impl fmt::Display for MarginType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cross => formatter.write_str("CROSSED"),
            Self::Isolated => formatter.write_str("ISOLATED"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FuturesStateChangeKind {
    Leverage,
    MarginType,
    PositionMode,
}

impl fmt::Display for FuturesStateChangeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leverage => formatter.write_str("leverage"),
            Self::MarginType => formatter.write_str("margin-type"),
            Self::PositionMode => formatter.write_str("position-mode"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PositionMode {
    OneWay,
    Hedge,
}

impl fmt::Display for PositionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OneWay => formatter.write_str("one-way"),
            Self::Hedge => formatter.write_str("hedge"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferDirection {
    SpotToUsdsFutures,
    UsdsFuturesToSpot,
}

impl fmt::Display for TransferDirection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpotToUsdsFutures => formatter.write_str("spot-to-usds-futures"),
            Self::UsdsFuturesToSpot => formatter.write_str("usds-futures-to-spot"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecimalValue(#[serde(with = "rust_decimal::serde::str")] pub Decimal);

impl DecimalValue {
    pub fn new(value: Decimal) -> Self {
        Self(value)
    }

    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }

    pub fn checked_mul(&self, other: &Self) -> Option<Self> {
        self.0.checked_mul(other.0).map(Self)
    }

    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
}

impl fmt::Display for DecimalValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.normalize())
    }
}

impl FromStr for DecimalValue {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let decimal = Decimal::from_str(value.trim())
            .map_err(|_| anyhow!("invalid decimal value: {value}"))?;
        if decimal <= Decimal::ZERO {
            return Err(anyhow!("decimal value must be positive: {value}"));
        }
        Ok(Self(decimal))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderIntent {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub market: Market,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: DecimalValue,
    pub spec: OrderSpec,
    pub reduce_only: bool,
    pub position_side: Option<PositionSide>,
    pub client_order_id: String,
}

impl OrderIntent {
    pub fn notional_usdt(&self) -> Option<DecimalValue> {
        self.quantity.checked_mul(self.spec.notional_price())
    }

    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        ProfilePermissionSet::one(self.market.required_profile_permission())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OrderSpec {
    Market {
        valuation_price: DecimalValue,
    },
    Limit {
        price: DecimalValue,
        time_in_force: TimeInForce,
    },
    #[serde(rename = "limit-maker")]
    PostOnlyLimit {
        price: DecimalValue,
    },
    StopLoss {
        stop_price: DecimalValue,
    },
    TakeProfit {
        stop_price: DecimalValue,
    },
}

impl OrderSpec {
    pub fn new(
        market: Market,
        kind: OrderKind,
        price: Option<DecimalValue>,
        valuation_price: Option<DecimalValue>,
        stop_price: Option<DecimalValue>,
        time_in_force: Option<TimeInForce>,
    ) -> Result<Self> {
        match kind {
            OrderKind::Market => {
                reject_present("price", price.as_ref())?;
                reject_present("stop price", stop_price.as_ref())?;
                if time_in_force.is_some() {
                    return Err(anyhow!("market order does not accept time in force"));
                }
                Ok(Self::Market {
                    valuation_price: valuation_price
                        .ok_or_else(|| anyhow!("market order requires valuation price"))?,
                })
            }
            OrderKind::Limit => {
                reject_present("valuation price", valuation_price.as_ref())?;
                reject_present("stop price", stop_price.as_ref())?;
                Ok(Self::Limit {
                    price: price.ok_or_else(|| anyhow!("limit order requires price"))?,
                    time_in_force: time_in_force
                        .ok_or_else(|| anyhow!("limit order requires time in force"))?,
                })
            }
            OrderKind::PostOnlyLimit if market == Market::UsdsFutures => Err(anyhow!(
                "{kind} is not supported for usds-futures yet; use spot post-only limit orders"
            )),
            OrderKind::PostOnlyLimit => {
                reject_present("valuation price", valuation_price.as_ref())?;
                reject_present("stop price", stop_price.as_ref())?;
                if time_in_force.is_some() {
                    return Err(anyhow!("limit-maker order does not accept time in force"));
                }
                Ok(Self::PostOnlyLimit {
                    price: price.ok_or_else(|| anyhow!("limit-maker order requires price"))?,
                })
            }
            OrderKind::StopLoss | OrderKind::TakeProfit if market == Market::UsdsFutures => {
                Err(anyhow!(
                    "{kind} is not supported for usds-futures yet; use a provider-specific order model once futures conditional orders are modeled"
                ))
            }
            OrderKind::StopLoss => {
                reject_present("price", price.as_ref())?;
                reject_present("valuation price", valuation_price.as_ref())?;
                if time_in_force.is_some() {
                    return Err(anyhow!("stop-loss order does not accept time in force"));
                }
                Ok(Self::StopLoss {
                    stop_price: stop_price
                        .ok_or_else(|| anyhow!("stop-loss order requires stop price"))?,
                })
            }
            OrderKind::TakeProfit => {
                reject_present("price", price.as_ref())?;
                reject_present("valuation price", valuation_price.as_ref())?;
                if time_in_force.is_some() {
                    return Err(anyhow!("take-profit order does not accept time in force"));
                }
                Ok(Self::TakeProfit {
                    stop_price: stop_price
                        .ok_or_else(|| anyhow!("take-profit order requires stop price"))?,
                })
            }
        }
    }

    pub const fn kind(&self) -> OrderKind {
        match self {
            Self::Market { .. } => OrderKind::Market,
            Self::Limit { .. } => OrderKind::Limit,
            Self::PostOnlyLimit { .. } => OrderKind::PostOnlyLimit,
            Self::StopLoss { .. } => OrderKind::StopLoss,
            Self::TakeProfit { .. } => OrderKind::TakeProfit,
        }
    }

    pub const fn notional_price(&self) -> &DecimalValue {
        match self {
            Self::Market { valuation_price } => valuation_price,
            Self::Limit { price, .. } => price,
            Self::PostOnlyLimit { price } => price,
            Self::StopLoss { stop_price } | Self::TakeProfit { stop_price } => stop_price,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelIntent {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub market: Market,
    pub symbol: String,
    pub target: OrderIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OrderIdentifier {
    OrderId { order_id: String },
    ClientOrderId { client_order_id: String },
}

impl OrderIdentifier {
    pub fn new(order_id: Option<String>, client_order_id: Option<String>) -> Result<Self> {
        match (order_id, client_order_id) {
            (Some(order_id), None) => Ok(Self::OrderId { order_id }),
            (None, Some(client_order_id)) => Ok(Self::ClientOrderId { client_order_id }),
            (Some(_), Some(_)) => Err(anyhow!(
                "order identifier accepts exactly one of order id or client order id"
            )),
            (None, None) => Err(anyhow!(
                "order identifier requires order id or client order id"
            )),
        }
    }
}

impl CancelIntent {
    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        ProfilePermissionSet::one(self.market.required_profile_permission())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferIntent {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub direction: TransferDirection,
    pub asset: String,
    pub amount: DecimalValue,
    pub client_transfer_id: String,
}

impl TransferIntent {
    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        ProfilePermissionSet::one(ProfilePermission::UniversalTransfer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuturesStateIntent {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub change: FuturesStateChange,
}

impl FuturesStateIntent {
    pub fn change_kind(&self) -> FuturesStateChangeKind {
        self.change.kind()
    }

    pub fn scope_label(&self) -> String {
        self.change.scope_label()
    }

    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        ProfilePermissionSet::one(ProfilePermission::UsdsFutures)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FuturesStateChange {
    Leverage {
        symbol: String,
        leverage: u8,
    },
    MarginType {
        symbol: String,
        margin_type: MarginType,
    },
    PositionMode {
        mode: PositionMode,
    },
}

impl FuturesStateChange {
    pub const fn kind(&self) -> FuturesStateChangeKind {
        match self {
            Self::Leverage { .. } => FuturesStateChangeKind::Leverage,
            Self::MarginType { .. } => FuturesStateChangeKind::MarginType,
            Self::PositionMode { .. } => FuturesStateChangeKind::PositionMode,
        }
    }

    pub fn scope_label(&self) -> String {
        match self {
            Self::Leverage { symbol, .. } | Self::MarginType { symbol, .. } => {
                symbol.to_ascii_uppercase()
            }
            Self::PositionMode { .. } => "binance-futures-account".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub environment: Environment,
    pub api_key_env: String,
    pub api_secret_env: String,
    pub spot_base_url: Option<String>,
    pub usds_futures_base_url: Option<String>,
    pub sapi_base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountSnapshotKind {
    ApiPermissions,
    SpotBalances,
    UsdsFuturesPositions,
}

impl fmt::Display for AccountSnapshotKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiPermissions => formatter.write_str("api-permissions"),
            Self::SpotBalances => formatter.write_str("spot-balances"),
            Self::UsdsFuturesPositions => formatter.write_str("usds-futures-positions"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub kind: AccountSnapshotKind,
    pub payload: serde_json::Value,
}

impl AccountSnapshot {
    pub fn new(
        profile: impl Into<String>,
        provider: Provider,
        environment: Environment,
        kind: AccountSnapshotKind,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            profile: profile.into(),
            provider,
            environment,
            kind,
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub ok: bool,
    pub required: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl DiagnosticCheck {
    pub fn new(name: &'static str, ok: bool, required: bool, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            ok,
            required,
            message: message.into(),
            payload: None,
        }
    }

    pub fn optional(name: &'static str, ok: bool, message: impl Into<String>) -> Self {
        Self::new(name, ok, false, message)
    }

    pub fn required(name: &'static str, ok: bool, message: impl Into<String>) -> Self {
        Self::new(name, ok, true, message)
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfilePermission {
    SpotTrading,
    UsdsFutures,
    UniversalTransfer,
}

impl ProfilePermission {
    pub const ALL: [Self; 3] = [
        Self::SpotTrading,
        Self::UsdsFutures,
        Self::UniversalTransfer,
    ];

    pub const fn field_name(self) -> &'static str {
        match self {
            Self::SpotTrading => "spot_trading",
            Self::UsdsFutures => "usds_futures",
            Self::UniversalTransfer => "universal_transfer",
        }
    }

    pub const fn disabled_code(self) -> &'static str {
        match self {
            Self::SpotTrading => "profile-permission-spot-trading-disabled",
            Self::UsdsFutures => "profile-permission-usds-futures-disabled",
            Self::UniversalTransfer => "profile-permission-universal-transfer-disabled",
        }
    }

    pub const fn policy_check_name(self) -> &'static str {
        match self {
            Self::SpotTrading => "profile-permission-spot-trading",
            Self::UsdsFutures => "profile-permission-usds-futures",
            Self::UniversalTransfer => "profile-permission-universal-transfer",
        }
    }

    pub const fn policy_reason(self) -> &'static str {
        match self {
            Self::SpotTrading => "risk.allowed_symbols includes spot markets",
            Self::UsdsFutures => {
                "risk.allowed_symbols or risk.allowed_futures_state_changes includes USD-M futures"
            }
            Self::UniversalTransfer => "risk.allowed_transfers is not empty",
        }
    }
}

impl fmt::Display for ProfilePermission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.field_name())
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ProfilePermissionSet {
    permissions: BTreeSet<ProfilePermission>,
}

impl ProfilePermissionSet {
    pub fn one(permission: ProfilePermission) -> Self {
        let mut set = Self::default();
        set.insert(permission);
        set
    }

    pub fn insert(&mut self, permission: ProfilePermission) {
        self.permissions.insert(permission);
    }

    pub fn contains(&self, permission: ProfilePermission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn iter(&self) -> impl Iterator<Item = ProfilePermission> + '_ {
        self.permissions.iter().copied()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ProfilePermissions {
    #[serde(default)]
    pub spot_trading: bool,
    #[serde(default)]
    pub usds_futures: bool,
    #[serde(default)]
    pub universal_transfer: bool,
}

impl ProfilePermissions {
    pub fn allows(self, permission: ProfilePermission) -> bool {
        match permission {
            ProfilePermission::SpotTrading => self.spot_trading,
            ProfilePermission::UsdsFutures => self.usds_futures,
            ProfilePermission::UniversalTransfer => self.universal_transfer,
        }
    }

    pub fn declared_profile_permissions(self) -> ProfilePermissionSet {
        let mut set = ProfilePermissionSet::default();
        for permission in ProfilePermission::ALL {
            if self.allows(permission) {
                set.insert(permission);
            }
        }
        set
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPolicy {
    pub allow_live: bool,
    #[serde(default)]
    pub max_daily_order_notional_usdt: Option<DecimalValue>,
    #[serde(default)]
    pub allowed_symbols: BTreeMap<String, SymbolPolicy>,
    #[serde(default)]
    pub allowed_transfers: Vec<TransferPolicy>,
    #[serde(default)]
    pub allowed_futures_state_changes: Vec<FuturesStatePolicy>,
}

impl RiskPolicy {
    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        let mut permissions = ProfilePermissionSet::default();
        for symbol in self.allowed_symbols.values() {
            for market in &symbol.markets {
                permissions.insert(market.required_profile_permission());
            }
        }
        if !self.allowed_transfers.is_empty() {
            permissions.insert(ProfilePermission::UniversalTransfer);
        }
        if !self.allowed_futures_state_changes.is_empty() {
            permissions.insert(ProfilePermission::UsdsFutures);
        }
        permissions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPolicy {
    #[serde(default)]
    pub markets: Vec<Market>,
    #[serde(default)]
    pub order_kinds: Vec<OrderKind>,
    pub max_order_notional_usdt: DecimalValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPolicy {
    pub direction: TransferDirection,
    pub asset: String,
    pub max_amount: DecimalValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FuturesStatePolicy {
    Leverage {
        symbol: String,
        max_leverage: u8,
    },
    MarginType {
        symbol: String,
        margin_type: MarginType,
    },
    PositionMode {
        mode: PositionMode,
    },
}

impl FuturesStatePolicy {
    pub fn matches_change_scope(&self, change: &FuturesStateChange) -> bool {
        match (self, change) {
            (
                Self::Leverage { symbol, .. },
                FuturesStateChange::Leverage {
                    symbol: intent_symbol,
                    ..
                },
            )
            | (
                Self::MarginType { symbol, .. },
                FuturesStateChange::MarginType {
                    symbol: intent_symbol,
                    ..
                },
            ) => symbol.eq_ignore_ascii_case(intent_symbol),
            (Self::PositionMode { .. }, FuturesStateChange::PositionMode { .. }) => true,
            _ => false,
        }
    }

    pub fn allows_change(&self, change: &FuturesStateChange) -> bool {
        if !self.matches_change_scope(change) {
            return false;
        }
        match (self, change) {
            (
                Self::Leverage { max_leverage, .. },
                FuturesStateChange::Leverage { leverage, .. },
            ) => leverage <= max_leverage,
            (
                Self::MarginType { margin_type, .. },
                FuturesStateChange::MarginType {
                    margin_type: requested,
                    ..
                },
            ) => requested == margin_type,
            (
                Self::PositionMode { mode, .. },
                FuturesStateChange::PositionMode { mode: requested },
            ) => requested == mode,
            _ => false,
        }
    }

    pub fn max_leverage(&self) -> Option<u8> {
        match self {
            Self::Leverage { max_leverage, .. } => Some(*max_leverage),
            Self::MarginType { .. } | Self::PositionMode { .. } => None,
        }
    }
}

impl fmt::Display for FuturesStatePolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leverage {
                symbol,
                max_leverage,
            } => write!(
                formatter,
                "leverage:{}<= {}",
                symbol.to_ascii_uppercase(),
                max_leverage
            ),
            Self::MarginType {
                symbol,
                margin_type,
            } => write!(
                formatter,
                "margin-type:{}={}",
                symbol.to_ascii_uppercase(),
                margin_type
            ),
            Self::PositionMode { mode } => write!(formatter, "position-mode={mode}"),
        }
    }
}

impl fmt::Display for TransferPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}<= {}",
            self.direction,
            self.asset.to_ascii_uppercase(),
            self.max_amount
        )
    }
}

fn reject_present<T>(name: &str, value: Option<&T>) -> Result<()> {
    if value.is_some() {
        return Err(anyhow!("{name} is not valid for this order kind"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn account_snapshot_keeps_provider_payload_inside_typed_envelope() {
        let snapshot = AccountSnapshot::new(
            "default",
            Provider::Binance,
            Environment::Live,
            AccountSnapshotKind::ApiPermissions,
            json!({"enableReading": true}),
        );

        let value = serde_json::to_value(snapshot).expect("account snapshot json");

        assert_eq!(value["profile"], "default");
        assert_eq!(value["provider"], "binance");
        assert_eq!(value["environment"], "live");
        assert_eq!(value["kind"], "api-permissions");
        assert_eq!(value["payload"]["enableReading"], true);
    }
}
