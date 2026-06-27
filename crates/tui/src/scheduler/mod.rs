use std::cell::Cell;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use agent_finance_market::{
    args::CryptoInstrument,
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
use crate::config::{EquityProvider, ProviderConfig, TuiLaunch};
use crate::state::{StagedChangeEvent, StagedOrderSubmitRequest};

mod write;

use write::{WriteCommand, spawn_write_worker};

#[derive(Debug)]
pub struct Scheduler {
    refresh_commands: Sender<RefreshCommand>,
    history_commands: Sender<HistoryCommand>,
    evidence_commands: Sender<EvidenceCommand>,
    research_commands: Sender<ResearchCommand>,
    account_commands: Sender<AccountCommand>,
    write_commands: Sender<WriteCommand>,
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
        let (write_commands, write_command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let runtime = MarketRuntime::new(
            launch.proxy.as_deref(),
            launch.no_proxy,
            launch.timeout_seconds,
            &launch.timezone,
        );
        let policy = TuiProviderPolicy::from(providers);

        let refresh_policy = policy.clone();
        spawn_scheduler_worker(
            "refresh",
            runtime.clone(),
            refresh_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| {
                handle_refresh_command(tokio, runtime, command, &refresh_policy)
            },
        );
        let history_policy = policy.clone();
        spawn_scheduler_worker(
            "history",
            runtime.clone(),
            history_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| {
                handle_history_command(tokio, runtime, command, &history_policy)
            },
        );
        let evidence_policy = policy;
        spawn_scheduler_worker(
            "evidence",
            runtime.clone(),
            evidence_command_rx,
            event_tx.clone(),
            move |tokio, runtime, command| {
                handle_evidence_command(tokio, runtime, command, &evidence_policy)
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
        spawn_write_worker(launch, write_command_rx, event_tx);

        Self {
            refresh_commands,
            history_commands,
            evidence_commands,
            research_commands,
            account_commands,
            write_commands,
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

    pub fn request_symbol_task(
        &self,
        kind: SymbolTaskKind,
        generation: u64,
        symbol: String,
    ) -> Result<()> {
        let result = match kind {
            SymbolTaskKind::History => self
                .history_commands
                .send(HistoryCommand { generation, symbol })
                .map_err(|error| error.to_string()),
            SymbolTaskKind::Evidence => self
                .evidence_commands
                .send(EvidenceCommand { generation, symbol })
                .map_err(|error| error.to_string()),
            SymbolTaskKind::Research => self
                .research_commands
                .send(ResearchCommand { generation, symbol })
                .map_err(|error| error.to_string()),
        };

        result.map_err(|error| anyhow!("failed to request TUI {}: {error}", kind.label()))
    }

    pub fn request_account(&self, generation: u64, profile: String) -> Result<()> {
        self.account_commands
            .send(AccountCommand {
                generation,
                profile,
            })
            .map_err(|error| anyhow!("failed to request TUI account snapshot: {error}"))
    }

    pub fn request_staged_order_submit(&self, request: StagedOrderSubmitRequest) -> Result<()> {
        self.write_commands
            .send(WriteCommand::SubmitStagedOrder(request))
            .map_err(|error| anyhow!("failed to request TUI staged order submit: {error}"))
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

#[derive(Debug)]
struct RefreshCommand {
    generation: u64,
    symbols: Vec<String>,
}

#[derive(Debug)]
struct HistoryCommand {
    generation: u64,
    symbol: String,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SymbolTaskKind {
    History,
    Evidence,
    Research,
}

impl SymbolTaskKind {
    pub const ALL: [Self; 3] = [Self::History, Self::Evidence, Self::Research];

    const fn label(self) -> &'static str {
        match self {
            Self::History => "history",
            Self::Evidence => "evidence",
            Self::Research => "research",
        }
    }
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
    let HistoryCommand { generation, symbol } = command;
    match tokio.block_on(fetch_history(runtime, symbol.clone(), policy)) {
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
) -> Result<HistorySnapshot> {
    history_snapshot::fetch_history_snapshot(runtime, policy.history_request(symbol)).await
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
        profile.name,
        profile.provider.provider,
        profile.provider.environment,
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

    fn history_request(&self, symbol: String) -> HistorySnapshotRequest {
        HistorySnapshotRequest {
            symbol,
            provider: self.equity.provider(),
            crypto_provider: self.crypto,
            interval: "1d".to_string(),
            range: "6mo".to_string(),
            limit: 90,
        }
    }

    fn crypto_evidence_request(&self, symbol: String) -> CryptoQuoteEvidenceSnapshotRequest {
        CryptoQuoteEvidenceSnapshotRequest {
            symbol,
            provider: self.crypto,
            instrument: CryptoInstrument::Auto,
        }
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
        let history = policy.history_request("CRDO".to_string());
        let evidence = policy.crypto_evidence_request("BTCUSDT".to_string());

        assert_eq!(quote.equity_provider, MarketDataProvider::Robinhood);
        assert_eq!(quote.crypto_provider, CryptoProvider::Okx);
        assert_eq!(history.provider, MarketDataProvider::Robinhood);
        assert_eq!(history.crypto_provider, CryptoProvider::Okx);
        assert_eq!(evidence.provider, CryptoProvider::Okx);
    }
}
