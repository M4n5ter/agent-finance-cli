use std::cell::Cell;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, RwLock};
use std::thread;

use agent_finance_core::ProfileStore;
use agent_finance_market::is_likely_crypto_pair;
use agent_finance_market::{
    args::{CryptoInstrument, HistorySession, Provider as MarketDataProvider},
    crypto_evidence_snapshot::{
        self, CryptoQuoteEvidenceSnapshot, CryptoQuoteEvidenceSnapshotRequest,
    },
    history_snapshot::{self, HistorySnapshot, HistorySnapshotRequest},
    research_snapshot::{self, ResearchContextSnapshot, ResearchContextSnapshotRequest},
    service::MarketRuntime,
    snapshot::{self, MarketSnapshot, PublicQuoteSnapshotRequest},
};
use agent_finance_trading::TradingRuntime;
use anyhow::{Result, anyhow};

use crate::account::{ACCOUNT_READ_PLAN, AccountReadError, AccountSnapshot};
use crate::chart::ChartHistoryRequest;
use crate::config::{EquityProvider, ProviderConfig, TuiLaunch};
use crate::profile_snapshot::{ProfileValidationSnapshot, TradingProfileSnapshot};
use crate::state::{StagedChangeEvent, StagedSubmitRequest};

mod write;

use write::{WriteCommand, spawn_write_worker};

#[derive(Debug)]
pub struct Scheduler {
    refresh_commands: Sender<RefreshCommand>,
    history_commands: Sender<HistoryCommand>,
    evidence_commands: Sender<EvidenceCommand>,
    research_commands: Sender<ResearchCommand>,
    account_commands: Sender<AccountCommand>,
    profile_validation_commands: Sender<ProfileValidationCommand>,
    write_commands: Sender<WriteCommand>,
    provider_policy: Arc<RwLock<TuiProviderPolicy>>,
    events: Receiver<SchedulerEvent>,
    disconnected_reported: Cell<bool>,
}

impl Scheduler {
    pub fn start(launch: &TuiLaunch, providers: ProviderConfig) -> Self {
        let (refresh_commands, refresh_command_rx) = mpsc::channel();
        let (history_commands, history_command_rx) = mpsc::channel();
        let (evidence_commands, evidence_command_rx) = mpsc::channel();
        let (research_commands, research_command_rx) = mpsc::channel();
        let (account_commands, account_command_rx) = mpsc::channel();
        let (profile_validation_commands, profile_validation_command_rx) = mpsc::channel();
        let (write_commands, write_command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let runtime = MarketRuntime::new(
            launch.proxy.as_deref(),
            launch.no_proxy,
            launch.timeout_seconds,
            &launch.timezone,
        );
        let policy = Arc::new(RwLock::new(TuiProviderPolicy::from(providers)));

        let refresh_policy = Arc::clone(&policy);
        spawn_scheduler_worker(
            "refresh",
            runtime.clone(),
            refresh_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| match provider_policy_snapshot(&refresh_policy) {
                Ok(policy) => handle_refresh_command(tokio, runtime, command, &policy),
                Err(error) => SchedulerEvent::RefreshFailed {
                    generation: command.generation,
                    error: error.to_string(),
                },
            },
        );
        let history_policy = Arc::clone(&policy);
        spawn_scheduler_worker(
            "history",
            runtime.clone(),
            history_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| match provider_policy_snapshot(&history_policy) {
                Ok(policy) => handle_history_command(tokio, runtime, command, &policy),
                Err(error) => SchedulerEvent::HistoryFailed {
                    generation: command.generation,
                    symbol: command.symbol,
                    error: error.to_string(),
                },
            },
        );
        let evidence_policy = Arc::clone(&policy);
        spawn_scheduler_worker(
            "evidence",
            runtime.clone(),
            evidence_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| match provider_policy_snapshot(&evidence_policy) {
                Ok(policy) => handle_evidence_command(tokio, runtime, command, &policy),
                Err(error) => SchedulerEvent::EvidenceFailed {
                    generation: command.generation,
                    symbol: command.symbol,
                    error: error.to_string(),
                },
            },
        );
        spawn_scheduler_worker(
            "research",
            runtime,
            research_command_rx,
            event_tx.clone(),
            handle_research_command,
        );
        spawn_account_worker(launch, account_command_rx, event_tx.clone());
        spawn_profile_validation_worker(profile_validation_command_rx, event_tx.clone());
        spawn_write_worker(launch, write_command_rx, event_tx);

        Self {
            refresh_commands,
            history_commands,
            evidence_commands,
            research_commands,
            account_commands,
            profile_validation_commands,
            write_commands,
            provider_policy: policy,
            events,
            disconnected_reported: Cell::new(false),
        }
    }

    pub fn request_refresh(&self, generation: u64, symbols: Vec<String>) -> Result<()> {
        self.refresh_commands
            .send(RefreshCommand {
                generation,
                symbols,
            })
            .map_err(|error| anyhow!("failed to request TUI refresh: {error}"))
    }

    pub fn request_history(
        &self,
        generation: u64,
        symbol: String,
        request: ChartHistoryRequest,
    ) -> Result<()> {
        self.history_commands
            .send(HistoryCommand {
                generation,
                symbol,
                request,
            })
            .map_err(|error| anyhow!("failed to request TUI history: {error}"))
    }

    pub fn request_evidence(&self, generation: u64, symbol: String) -> Result<()> {
        self.evidence_commands
            .send(EvidenceCommand { generation, symbol })
            .map_err(|error| anyhow!("failed to request TUI evidence: {error}"))
    }

    pub fn request_research(&self, generation: u64, symbol: String) -> Result<()> {
        self.research_commands
            .send(ResearchCommand { generation, symbol })
            .map_err(|error| anyhow!("failed to request TUI research: {error}"))
    }

    pub fn request_account(&self, generation: u64, profile: String) -> Result<()> {
        self.account_commands
            .send(AccountCommand {
                generation,
                profile,
            })
            .map_err(|error| anyhow!("failed to request TUI account snapshot: {error}"))
    }

    pub fn request_profile_validation(&self, generation: u64, profile: String) -> Result<()> {
        self.profile_validation_commands
            .send(ProfileValidationCommand {
                generation,
                profile,
            })
            .map_err(|error| anyhow!("failed to request TUI profile validation: {error}"))
    }

    pub fn request_staged_submit(&self, request: StagedSubmitRequest) -> Result<()> {
        self.write_commands
            .send(WriteCommand::SubmitStaged(request))
            .map_err(|error| anyhow!("failed to request TUI staged submit: {error}"))
    }

    pub fn update_provider_policy(&self, providers: ProviderConfig) -> Result<()> {
        let mut policy = self
            .provider_policy
            .write()
            .map_err(|_| anyhow!("failed to update TUI provider policy: lock poisoned"))?;
        *policy = TuiProviderPolicy::from(providers);
        Ok(())
    }

    pub fn try_recv(&self) -> Option<SchedulerEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) if !self.disconnected_reported.replace(true) => Some(
                SchedulerEvent::Fatal("scheduler worker stopped".to_string()),
            ),
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

fn provider_policy_snapshot(policy: &Arc<RwLock<TuiProviderPolicy>>) -> Result<TuiProviderPolicy> {
    policy
        .read()
        .map(|policy| policy.clone())
        .map_err(|_| anyhow!("failed to read TUI provider policy: lock poisoned"))
}

#[derive(Debug)]
struct RefreshCommand {
    generation: u64,
    symbols: Vec<String>,
}

#[derive(Debug)]
struct HistoryCommand {
    generation: u64,
    symbol: String,
    request: ChartHistoryRequest,
}

#[derive(Debug)]
struct EvidenceCommand {
    generation: u64,
    symbol: String,
}

#[derive(Debug)]
struct ResearchCommand {
    generation: u64,
    symbol: String,
}

#[derive(Debug)]
struct AccountCommand {
    generation: u64,
    profile: String,
}

#[derive(Debug)]
struct ProfileValidationCommand {
    generation: u64,
    profile: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SymbolTaskKind {
    History,
    Evidence,
    Research,
}

impl SymbolTaskKind {
    pub const ALL: [Self; 3] = [Self::History, Self::Evidence, Self::Research];
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerEvent {
    Snapshot {
        generation: u64,
        snapshot: MarketSnapshot,
    },
    RefreshFailed {
        generation: u64,
        error: String,
    },
    History {
        generation: u64,
        snapshot: HistorySnapshot,
    },
    HistoryFailed {
        generation: u64,
        symbol: String,
        error: String,
    },
    Evidence {
        generation: u64,
        snapshot: CryptoQuoteEvidenceSnapshot,
    },
    EvidenceFailed {
        generation: u64,
        symbol: String,
        error: String,
    },
    Research {
        generation: u64,
        snapshot: ResearchContextSnapshot,
    },
    Account {
        generation: u64,
        snapshot: AccountSnapshot,
    },
    AccountFailed {
        generation: u64,
        profile: String,
        error: String,
    },
    ProfileValidation {
        generation: u64,
        snapshot: ProfileValidationSnapshot,
    },
    ProfileValidationFailed {
        generation: u64,
        profile: String,
        error: String,
    },
    StagedChangeProgress {
        id: String,
        event: StagedChangeEvent,
        message: Option<String>,
    },
    Fatal(String),
}

fn spawn_scheduler_worker<C, F>(
    name: &'static str,
    runtime: MarketRuntime,
    commands: Receiver<C>,
    events: Sender<SchedulerEvent>,
    handle: F,
) where
    C: Send + 'static,
    F: Fn(&tokio::runtime::Runtime, &MarketRuntime, C) -> SchedulerEvent + Send + 'static,
{
    thread::Builder::new()
        .name(format!("agent-finance-tui-{name}"))
        .spawn(move || {
            let Some(tokio) = scheduler_runtime(name, &events) else {
                return;
            };

            while let Ok(command) = commands.recv() {
                if events.send(handle(&tokio, &runtime, command)).is_err() {
                    break;
                }
            }
        })
        .unwrap_or_else(|error| panic!("failed to spawn TUI {name} scheduler thread: {error}"));
}

fn spawn_account_worker(
    launch: &TuiLaunch,
    commands: Receiver<AccountCommand>,
    events: Sender<SchedulerEvent>,
) {
    let runtime = TradingRuntime::with_http_policy(
        launch.timeout_seconds,
        launch.proxy.clone(),
        launch.no_proxy,
    );
    thread::Builder::new()
        .name("agent-finance-tui-account".to_string())
        .spawn(move || {
            let Some(tokio) = scheduler_runtime("account", &events) else {
                return;
            };

            while let Ok(command) = commands.recv() {
                if events
                    .send(handle_account_command(&tokio, &runtime, command))
                    .is_err()
                {
                    break;
                }
            }
        })
        .unwrap_or_else(|error| panic!("failed to spawn TUI account scheduler thread: {error}"));
}

fn spawn_profile_validation_worker(
    commands: Receiver<ProfileValidationCommand>,
    events: Sender<SchedulerEvent>,
) {
    thread::Builder::new()
        .name("agent-finance-tui-profile-validation".to_string())
        .spawn(move || {
            while let Ok(command) = commands.recv() {
                if events
                    .send(handle_profile_validation_command(command))
                    .is_err()
                {
                    break;
                }
            }
        })
        .unwrap_or_else(|error| {
            panic!("failed to spawn TUI profile validation scheduler thread: {error}")
        });
}

fn handle_refresh_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &MarketRuntime,
    command: RefreshCommand,
    policy: &TuiProviderPolicy,
) -> SchedulerEvent {
    let RefreshCommand {
        generation,
        symbols,
    } = command;
    match tokio.block_on(fetch_snapshot(runtime, symbols, policy)) {
        Ok(snapshot) => SchedulerEvent::Snapshot {
            generation,
            snapshot,
        },
        Err(error) => SchedulerEvent::RefreshFailed {
            generation,
            error: error.to_string(),
        },
    }
}

fn handle_history_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &MarketRuntime,
    command: HistoryCommand,
    policy: &TuiProviderPolicy,
) -> SchedulerEvent {
    let HistoryCommand {
        generation,
        symbol,
        request,
    } = command;
    match tokio.block_on(fetch_history(runtime, symbol.clone(), policy, request)) {
        Ok(snapshot) => SchedulerEvent::History {
            generation,
            snapshot,
        },
        Err(error) => SchedulerEvent::HistoryFailed {
            generation,
            symbol,
            error: error.to_string(),
        },
    }
}

fn handle_evidence_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &MarketRuntime,
    command: EvidenceCommand,
    policy: &TuiProviderPolicy,
) -> SchedulerEvent {
    let EvidenceCommand { generation, symbol } = command;
    match tokio.block_on(fetch_evidence(runtime, symbol.clone(), policy)) {
        Ok(snapshot) => SchedulerEvent::Evidence {
            generation,
            snapshot,
        },
        Err(error) => SchedulerEvent::EvidenceFailed {
            generation,
            symbol,
            error: error.to_string(),
        },
    }
}

fn handle_research_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &MarketRuntime,
    command: ResearchCommand,
) -> SchedulerEvent {
    let ResearchCommand { generation, symbol } = command;
    SchedulerEvent::Research {
        generation,
        snapshot: tokio.block_on(fetch_research(runtime, symbol)),
    }
}

fn handle_account_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &TradingRuntime,
    command: AccountCommand,
) -> SchedulerEvent {
    let AccountCommand {
        generation,
        profile,
    } = command;
    match tokio.block_on(fetch_account(runtime, profile.clone())) {
        Ok(snapshot) => SchedulerEvent::Account {
            generation,
            snapshot,
        },
        Err(error) => SchedulerEvent::AccountFailed {
            generation,
            profile,
            error: error.to_string(),
        },
    }
}

fn handle_profile_validation_command(command: ProfileValidationCommand) -> SchedulerEvent {
    let ProfileValidationCommand {
        generation,
        profile,
    } = command;
    match load_profile_validation_snapshot(&profile) {
        Ok(snapshot) => SchedulerEvent::ProfileValidation {
            generation,
            snapshot,
        },
        Err(error) => SchedulerEvent::ProfileValidationFailed {
            generation,
            profile,
            error: error.to_string(),
        },
    }
}

fn load_profile_validation_snapshot(profile: &str) -> Result<ProfileValidationSnapshot> {
    let store = ProfileStore::from_default_dir()?;
    let loaded = store.load_report(profile)?;
    Ok(ProfileValidationSnapshot::from_loaded(loaded))
}

fn scheduler_runtime(
    worker_name: &str,
    events: &Sender<SchedulerEvent>,
) -> Option<tokio::runtime::Runtime> {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            let _ = events.send(SchedulerEvent::Fatal(format!(
                "failed to start {worker_name} scheduler runtime: {error}"
            )));
            None
        }
    }
}

async fn fetch_snapshot(
    runtime: &MarketRuntime,
    symbols: Vec<String>,
    policy: &TuiProviderPolicy,
) -> Result<MarketSnapshot> {
    snapshot::fetch_public_quote_snapshot(runtime, policy.public_quote_request(symbols)).await
}

async fn fetch_history(
    runtime: &MarketRuntime,
    symbol: String,
    policy: &TuiProviderPolicy,
    request: ChartHistoryRequest,
) -> Result<HistorySnapshot> {
    history_snapshot::fetch_history_snapshot(runtime, policy.history_request(symbol, request)).await
}

async fn fetch_evidence(
    runtime: &MarketRuntime,
    symbol: String,
    policy: &TuiProviderPolicy,
) -> Result<CryptoQuoteEvidenceSnapshot> {
    crypto_evidence_snapshot::fetch_crypto_quote_evidence_snapshot(
        runtime,
        policy.crypto_evidence_request(symbol),
    )
    .await
}

async fn fetch_research(runtime: &MarketRuntime, symbol: String) -> ResearchContextSnapshot {
    research_snapshot::fetch_research_context_snapshot(
        runtime,
        ResearchContextSnapshotRequest {
            symbol,
            news_count: 5,
            prediction_count: 5,
            refresh: false,
            cache_ttl_seconds: 900,
        },
    )
    .await
}

async fn fetch_account(runtime: &TradingRuntime, profile_name: String) -> Result<AccountSnapshot> {
    let profile = runtime.load_profile(&profile_name)?;
    let mut reads = Vec::new();
    let mut errors = Vec::new();

    for plan in ACCOUNT_READ_PLAN {
        if plan.live_only() && !profile.provider.environment.is_live() {
            errors.push(AccountReadError::from_plan(
                &plan,
                format!(
                    "{} uses Binance live SAPI account data and is skipped for {} profiles",
                    plan.kind(),
                    profile.provider.environment
                ),
            ));
            continue;
        }

        match runtime.run_signed_read(&profile, plan.request()).await {
            Ok(snapshot) => reads.push(snapshot),
            Err(error) => errors.push(AccountReadError::from_plan(&plan, error.to_string())),
        }
    }

    Ok(AccountSnapshot::new(
        profile.name.clone(),
        profile.provider.provider,
        profile.provider.environment,
        TradingProfileSnapshot::from(&profile),
        reads,
        errors,
    ))
}

#[derive(Debug, Clone)]
struct TuiProviderPolicy {
    equity: EquityProvider,
    crypto: agent_finance_market::args::CryptoProvider,
}

impl From<ProviderConfig> for TuiProviderPolicy {
    fn from(config: ProviderConfig) -> Self {
        Self {
            equity: config.equity,
            crypto: config.crypto,
        }
    }
}

impl TuiProviderPolicy {
    fn public_quote_request(&self, symbols: Vec<String>) -> PublicQuoteSnapshotRequest {
        PublicQuoteSnapshotRequest {
            symbols,
            equity_provider: self.equity.provider(),
            crypto_provider: self.crypto,
        }
    }

    fn history_request(
        &self,
        symbol: String,
        request: ChartHistoryRequest,
    ) -> HistorySnapshotRequest {
        let request = self.adapt_history_request(&symbol, request);
        HistorySnapshotRequest {
            symbol,
            provider: request.provider,
            crypto_provider: self.crypto,
            session: request.session,
            interval: request.interval,
            range: request.range,
            limit: request.limit,
        }
    }

    fn crypto_evidence_request(&self, symbol: String) -> CryptoQuoteEvidenceSnapshotRequest {
        CryptoQuoteEvidenceSnapshotRequest {
            symbol,
            provider: self.crypto,
            instrument: CryptoInstrument::Auto,
        }
    }

    fn adapt_history_request(
        &self,
        symbol: &str,
        request: ChartHistoryRequest,
    ) -> ProviderHistoryRequest {
        if is_likely_crypto_pair(symbol) {
            return ProviderHistoryRequest {
                provider: MarketDataProvider::Auto,
                session: HistorySession::Regular,
                interval: request.interval,
                range: request.range,
                limit: request.limit,
            };
        }
        let provider = self.equity.provider();
        match provider {
            MarketDataProvider::Robinhood => ProviderHistoryRequest {
                provider,
                session: request.session,
                interval: adapt_robinhood_interval(&request.interval).to_string(),
                range: request.range,
                limit: request.limit,
            },
            MarketDataProvider::Stooq => ProviderHistoryRequest {
                provider,
                session: HistorySession::Regular,
                interval: adapt_stooq_interval(&request.interval).to_string(),
                limit: adapt_stooq_limit(&request.range, request.limit),
                range: request.range,
            },
            provider => ProviderHistoryRequest {
                provider,
                session: request.session,
                interval: request.interval,
                range: request.range,
                limit: request.limit,
            },
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ProviderHistoryRequest {
    provider: MarketDataProvider,
    session: HistorySession,
    interval: String,
    range: String,
    limit: usize,
}

fn adapt_robinhood_interval(interval: &str) -> &str {
    match interval {
        "1m" => "5m",
        "5m" | "10m" | "1h" | "1d" | "1w" => interval,
        _ => "1d",
    }
}

fn adapt_stooq_interval(interval: &str) -> &str {
    match interval {
        "1w" => "1w",
        "1mo" => "1mo",
        _ => "1d",
    }
}

fn adapt_stooq_limit(range: &str, fallback: usize) -> usize {
    match range {
        "1d" => 1,
        "5d" => 5,
        "1mo" => 31,
        "3mo" => 66,
        "6mo" => 132,
        "1y" => 252,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use agent_finance_market::args::{CryptoProvider, Provider as MarketDataProvider};

    use super::*;

    #[test]
    fn request_builders_carry_tui_provider_preferences_into_market_requests() {
        let policy = TuiProviderPolicy::from(ProviderConfig {
            equity: EquityProvider::Robinhood,
            crypto: CryptoProvider::Okx,
        });

        let quote = policy.public_quote_request(vec!["CRDO".to_string(), "BTCUSDT".to_string()]);
        let history = policy.history_request(
            "CRDO".to_string(),
            crate::chart::ChartPreset::FiveDays.request_for("CRDO"),
        );
        let evidence = policy.crypto_evidence_request("BTCUSDT".to_string());

        assert_eq!(quote.equity_provider, MarketDataProvider::Robinhood);
        assert_eq!(quote.crypto_provider, CryptoProvider::Okx);
        assert_eq!(history.provider, MarketDataProvider::Robinhood);
        assert_eq!(history.session, HistorySession::Extended);
        assert_eq!(history.interval, "5m");
        assert_eq!(history.crypto_provider, CryptoProvider::Okx);
        assert_eq!(evidence.provider, CryptoProvider::Okx);

        let regular_history = policy.history_request(
            "CRDO".to_string(),
            crate::chart::ChartPreset::OneMonth.request_for("CRDO"),
        );
        assert_eq!(regular_history.provider, MarketDataProvider::Robinhood);
        assert_eq!(regular_history.interval, "1d");
    }

    #[test]
    fn history_request_adapts_precision_to_provider_capabilities() {
        let robinhood = TuiProviderPolicy::from(ProviderConfig {
            equity: EquityProvider::Robinhood,
            crypto: CryptoProvider::Auto,
        });
        let robinhood_day = robinhood.history_request(
            "CRDO".to_string(),
            crate::chart::ChartPreset::OneDay.request_for("CRDO"),
        );
        assert_eq!(robinhood_day.provider, MarketDataProvider::Robinhood);
        assert_eq!(robinhood_day.session, HistorySession::Extended);
        assert_eq!(robinhood_day.interval, "5m");

        let stooq = TuiProviderPolicy::from(ProviderConfig {
            equity: EquityProvider::Stooq,
            crypto: CryptoProvider::Auto,
        });
        let stooq_day = stooq.history_request(
            "CRDO".to_string(),
            crate::chart::ChartPreset::OneDay.request_for("CRDO"),
        );
        assert_eq!(stooq_day.provider, MarketDataProvider::Stooq);
        assert_eq!(stooq_day.session, HistorySession::Regular);
        assert_eq!(stooq_day.interval, "1d");
        assert_eq!(stooq_day.limit, 1);

        let stooq_auto = stooq.history_request(
            "CRDO".to_string(),
            crate::chart::ChartPreset::Auto.request_for("CRDO"),
        );
        assert_eq!(stooq_auto.interval, "1d");
        assert_eq!(stooq_auto.limit, 5);
    }

    #[test]
    fn history_request_does_not_apply_equity_provider_adapters_to_crypto_symbols() {
        let policy = TuiProviderPolicy::from(ProviderConfig {
            equity: EquityProvider::Robinhood,
            crypto: CryptoProvider::Binance,
        });
        let mut chart = crate::chart::ChartState::new(crate::chart::ChartPreset::OneDay);
        assert!(chart.set_interval(crate::chart::ChartInterval::FifteenMinutes));

        let history = policy.history_request(
            "BTCUSDT".to_string(),
            chart.request_for_provider("BTCUSDT", MarketDataProvider::Robinhood),
        );

        assert_eq!(history.provider, MarketDataProvider::Auto);
        assert_eq!(history.crypto_provider, CryptoProvider::Binance);
        assert_eq!(history.session, HistorySession::Regular);
        assert_eq!(history.interval, "15m");
        assert_eq!(history.limit, 96);
    }

    #[test]
    fn scheduler_provider_policy_can_be_updated_for_later_requests() {
        let launch = TuiLaunch::new(Vec::new(), None, true);
        let scheduler = Scheduler::start(&launch, ProviderConfig::default());

        scheduler
            .update_provider_policy(ProviderConfig {
                equity: EquityProvider::Robinhood,
                crypto: CryptoProvider::Okx,
            })
            .expect("update provider policy");

        let policy = provider_policy_snapshot(&scheduler.provider_policy).expect("policy");
        let quote = policy.public_quote_request(vec!["CRDO".to_string()]);
        let evidence = policy.crypto_evidence_request("BTCUSDT".to_string());

        assert_eq!(quote.equity_provider, MarketDataProvider::Robinhood);
        assert_eq!(quote.crypto_provider, CryptoProvider::Okx);
        assert_eq!(evidence.provider, CryptoProvider::Okx);
    }
}
