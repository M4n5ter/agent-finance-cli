use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
pub struct CapabilitiesArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
    /// Print the default profile path.
    Path(ProfileNameArgs),
    /// Print a starter TOML profile template.
    Template(ProfileNameArgs),
    /// Explain one configured profile without reading secrets.
    Explain(ProfileNameArgs),
    /// Check profile shape, env vars, and Binance signed permissions when possible.
    Doctor(ProfileNameArgs),
}

#[derive(Parser, Debug)]
pub struct ProfileNameArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct AccountArgs {
    #[command(subcommand)]
    pub command: AccountCommand,
}

#[derive(Subcommand, Debug)]
pub enum AccountCommand {
    /// Print Binance API key permissions.
    Permissions(SignedProfileArgs),
    /// Print Binance spot balances.
    Balances(SignedProfileArgs),
    /// Print Binance USD-M positions.
    Positions(SignedProfileArgs),
}

#[derive(Parser, Debug)]
pub struct SignedProfileArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct OrderArgs {
    #[command(subcommand)]
    pub command: OrderCommand,
}

#[derive(Subcommand, Debug)]
pub enum OrderCommand {
    /// Create and persist an order intent.
    Intent(OrderIntentArgs),
    /// Create and persist a cancel intent.
    CancelIntent(OrderCancelIntentArgs),
    /// Submit an existing intent as dry-run, exchange test, or live write.
    Submit(OrderSubmitArgs),
    /// Query one order by exchange order id or client order id.
    Query(OrderQueryArgs),
    /// Query open orders.
    Open(OrderOpenArgs),
}

#[derive(Parser, Debug)]
pub struct OrderIntentArgs {
    pub symbol: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub market: TradingMarket,

    #[arg(long, value_enum)]
    pub side: TradingSide,

    #[arg(long, value_enum)]
    pub kind: TradingOrderKind,

    #[arg(long)]
    pub quantity: String,

    #[arg(long)]
    pub price: Option<String>,

    #[arg(long)]
    pub valuation_price: Option<String>,

    #[arg(long)]
    pub stop_price: Option<String>,

    #[arg(long, value_enum)]
    pub time_in_force: Option<TradingTimeInForce>,

    #[arg(long)]
    pub reduce_only: bool,

    #[arg(long, value_enum)]
    pub position_side: Option<TradingPositionSide>,

    #[arg(long, default_value_t = 300)]
    pub ttl_seconds: i64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct OrderSubmitArgs {
    pub intent_id: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    /// Submit to exchange test endpoint where available.
    #[arg(long)]
    pub test: bool,

    /// Execute a real live write when profile policy allows it.
    #[arg(long)]
    pub live: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct OrderCancelIntentArgs {
    pub symbol: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub market: TradingMarket,

    #[arg(long)]
    pub order_id: Option<String>,

    #[arg(long)]
    pub client_order_id: Option<String>,

    #[arg(long, default_value_t = 300)]
    pub ttl_seconds: i64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct OrderOpenArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub market: TradingMarket,

    #[arg(long)]
    pub symbol: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct OrderQueryArgs {
    pub symbol: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub market: TradingMarket,

    #[arg(long)]
    pub order_id: Option<String>,

    #[arg(long)]
    pub client_order_id: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct TransferArgs {
    #[command(subcommand)]
    pub command: TransferCommand,
}

#[derive(Subcommand, Debug)]
pub enum TransferCommand {
    /// Create and persist an internal transfer intent.
    Intent(TransferIntentArgs),
    /// Submit an existing internal transfer intent.
    Submit(TransferSubmitArgs),
    /// Query Binance user universal transfer history.
    History(TransferHistoryArgs),
}

#[derive(Parser, Debug)]
pub struct TransferIntentArgs {
    pub asset: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub direction: TradingTransferDirection,

    #[arg(long)]
    pub amount: String,

    #[arg(long, default_value_t = 300)]
    pub ttl_seconds: i64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct TransferSubmitArgs {
    pub intent_id: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    /// Execute a real live transfer when profile policy allows it.
    #[arg(long)]
    pub live: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct TransferHistoryArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub direction: TradingTransferDirection,

    #[arg(long, default_value_t = 1)]
    pub current: usize,

    #[arg(long, default_value_t = 10)]
    pub size: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct StateArgs {
    #[command(subcommand)]
    pub command: StateCommand,
}

#[derive(Subcommand, Debug)]
pub enum StateCommand {
    /// Create and persist a USD-M futures state-change intent.
    Intent(StateIntentArgs),
    /// Submit an existing state-change intent as dry-run or live write.
    Submit(StateSubmitArgs),
}

#[derive(Parser, Debug)]
pub struct StateIntentArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long, value_enum)]
    pub kind: TradingFuturesStateChangeKind,

    #[arg(long)]
    pub symbol: Option<String>,

    #[arg(long)]
    pub leverage: Option<u8>,

    #[arg(long, value_enum)]
    pub margin_type: Option<TradingMarginType>,

    #[arg(long, default_value_t = 300)]
    pub ttl_seconds: i64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct StateSubmitArgs {
    pub intent_id: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    /// Execute a real live state change when profile policy allows it.
    #[arg(long)]
    pub live: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct RiskArgs {
    #[command(subcommand)]
    pub command: RiskCommand,
}

#[derive(Subcommand, Debug)]
pub enum RiskCommand {
    /// Check a persisted intent against a profile.
    Check(RiskCheckArgs),
    /// Explain a profile risk policy and runtime audit usage.
    Explain(RiskExplainArgs),
}

#[derive(Parser, Debug)]
pub struct RiskCheckArgs {
    pub intent_id: String,

    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub live: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct RiskExplainArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct AuditArgs {
    #[command(subcommand)]
    pub command: AuditCommand,
}

#[derive(Subcommand, Debug)]
pub enum AuditCommand {
    /// Print recent audit events.
    Tail(AuditTailArgs),
    /// Export audit events as JSON array or JSONL.
    Export(AuditExportArgs),
}

#[derive(Parser, Debug)]
pub struct AuditTailArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct AuditExportArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingMarket {
    Spot,
    UsdsFutures,
}

impl From<TradingMarket> for agent_finance_core::Market {
    fn from(value: TradingMarket) -> Self {
        match value {
            TradingMarket::Spot => Self::Spot,
            TradingMarket::UsdsFutures => Self::UsdsFutures,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingSide {
    Buy,
    Sell,
}

impl From<TradingSide> for agent_finance_core::OrderSide {
    fn from(value: TradingSide) -> Self {
        match value {
            TradingSide::Buy => Self::Buy,
            TradingSide::Sell => Self::Sell,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingOrderKind {
    Market,
    Limit,
    LimitMaker,
    StopLoss,
    TakeProfit,
}

impl From<TradingOrderKind> for agent_finance_core::OrderKind {
    fn from(value: TradingOrderKind) -> Self {
        match value {
            TradingOrderKind::Market => Self::Market,
            TradingOrderKind::Limit => Self::Limit,
            TradingOrderKind::LimitMaker => Self::PostOnlyLimit,
            TradingOrderKind::StopLoss => Self::StopLoss,
            TradingOrderKind::TakeProfit => Self::TakeProfit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingTimeInForce {
    Gtc,
    Ioc,
    Fok,
}

impl From<TradingTimeInForce> for agent_finance_core::TimeInForce {
    fn from(value: TradingTimeInForce) -> Self {
        match value {
            TradingTimeInForce::Gtc => Self::Gtc,
            TradingTimeInForce::Ioc => Self::Ioc,
            TradingTimeInForce::Fok => Self::Fok,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingPositionSide {
    Both,
    Long,
    Short,
}

impl From<TradingPositionSide> for agent_finance_core::PositionSide {
    fn from(value: TradingPositionSide) -> Self {
        match value {
            TradingPositionSide::Both => Self::Both,
            TradingPositionSide::Long => Self::Long,
            TradingPositionSide::Short => Self::Short,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingTransferDirection {
    SpotToUsdsFutures,
    UsdsFuturesToSpot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingFuturesStateChangeKind {
    Leverage,
    MarginType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TradingMarginType {
    Cross,
    Isolated,
}

impl From<TradingMarginType> for agent_finance_core::MarginType {
    fn from(value: TradingMarginType) -> Self {
        match value {
            TradingMarginType::Cross => Self::Cross,
            TradingMarginType::Isolated => Self::Isolated,
        }
    }
}

impl From<TradingTransferDirection> for agent_finance_core::TransferDirection {
    fn from(value: TradingTransferDirection) -> Self {
        match value {
            TradingTransferDirection::SpotToUsdsFutures => Self::SpotToUsdsFutures,
            TradingTransferDirection::UsdsFuturesToSpot => Self::UsdsFuturesToSpot,
        }
    }
}
