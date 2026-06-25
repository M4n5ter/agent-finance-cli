use std::cell::Cell;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use agent_finance_market::{
    args::{CryptoInstrument, CryptoProvider},
    crypto_evidence_snapshot::{
        self, CryptoQuoteEvidenceSnapshot, CryptoQuoteEvidenceSnapshotRequest,
    },
    history_snapshot::{self, HistorySnapshot, HistorySnapshotRequest},
    research_snapshot::{self, ResearchContextSnapshot, ResearchContextSnapshotRequest},
    service::MarketRuntime,
    snapshot::{self, MarketSnapshot, PublicQuoteSnapshotRequest},
};
use anyhow::{Result, anyhow};

use crate::config::TuiLaunch;

#[derive(Debug)]
pub struct Scheduler {
    refresh_commands: Sender<RefreshCommand>,
    history_commands: Sender<HistoryCommand>,
    evidence_commands: Sender<EvidenceCommand>,
    research_commands: Sender<ResearchCommand>,
    events: Receiver<SchedulerEvent>,
    disconnected_reported: Cell<bool>,
}

impl Scheduler {
    pub fn start(launch: &TuiLaunch) -> Self {
        let (refresh_commands, refresh_command_rx) = mpsc::channel();
        let (history_commands, history_command_rx) = mpsc::channel();
        let (evidence_commands, evidence_command_rx) = mpsc::channel();
        let (research_commands, research_command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let runtime = MarketRuntime::new(
            launch.proxy.as_deref(),
            launch.no_proxy,
            launch.timeout_seconds,
            &launch.timezone,
        );

        spawn_scheduler_worker(
            "refresh",
            runtime.clone(),
            refresh_command_rx,
            event_tx.clone(),
            handle_refresh_command,
        );
        spawn_scheduler_worker(
            "history",
            runtime.clone(),
            history_command_rx,
            event_tx.clone(),
            handle_history_command,
        );
        spawn_scheduler_worker(
            "evidence",
            runtime.clone(),
            evidence_command_rx,
            event_tx.clone(),
            handle_evidence_command,
        );
        spawn_scheduler_worker(
            "research",
            runtime,
            research_command_rx,
            event_tx,
            handle_research_command,
        );

        Self {
            refresh_commands,
            history_commands,
            evidence_commands,
            research_commands,
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

fn handle_refresh_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &MarketRuntime,
    command: RefreshCommand,
) -> SchedulerEvent {
    let RefreshCommand {
        generation,
        symbols,
    } = command;
    match tokio.block_on(fetch_snapshot(runtime, symbols)) {
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
) -> SchedulerEvent {
    let HistoryCommand { generation, symbol } = command;
    match tokio.block_on(fetch_history(runtime, symbol.clone())) {
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
) -> SchedulerEvent {
    let EvidenceCommand { generation, symbol } = command;
    match tokio.block_on(fetch_evidence(runtime, symbol.clone())) {
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

async fn fetch_snapshot(runtime: &MarketRuntime, symbols: Vec<String>) -> Result<MarketSnapshot> {
    snapshot::fetch_public_quote_snapshot(runtime, PublicQuoteSnapshotRequest { symbols }).await
}

async fn fetch_history(runtime: &MarketRuntime, symbol: String) -> Result<HistorySnapshot> {
    history_snapshot::fetch_history_snapshot(
        runtime,
        HistorySnapshotRequest {
            symbol,
            interval: "1d".to_string(),
            range: "6mo".to_string(),
            limit: 90,
        },
    )
    .await
}

async fn fetch_evidence(
    runtime: &MarketRuntime,
    symbol: String,
) -> Result<CryptoQuoteEvidenceSnapshot> {
    crypto_evidence_snapshot::fetch_crypto_quote_evidence_snapshot(
        runtime,
        CryptoQuoteEvidenceSnapshotRequest {
            symbol,
            provider: CryptoProvider::Auto,
            instrument: CryptoInstrument::Auto,
        },
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
