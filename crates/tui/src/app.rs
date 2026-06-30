use std::io::{self, Stdout};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use agent_finance_market::is_likely_crypto_pair;

use crate::account_load::{AccountLoadMode, AccountLoadRuntime, request_account_load};
use crate::config::{TuiConfig, TuiLaunch};
use crate::dump::TuiDump;
use crate::input::{self, MouseDrag};
use crate::model::Panel;
use crate::profile_validation_load::{
    ProfileValidationLoadRuntime, request_profile_validation_load,
};
use crate::render;
use crate::scheduler::{Scheduler, SchedulerEvent, SymbolTaskKind};
use crate::state::{Action, AppState, SelectedSymbolLoad, SymbolSnapshot};
use crate::task_failure::TaskFailureSource;

mod pending;

use pending::{PendingAppRequests, drain_pending_app_requests};

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run(launch: TuiLaunch) -> Result<()> {
    let persisted_config = launch.load_config()?;
    let runtime_config = launch.runtime_config(persisted_config.clone());
    let mut state = AppState::from_config(runtime_config.clone());
    state.reduce(Action::Log("cockpit initialized".to_string()));

    if launch.dump_state.is_some() {
        return run_dump_state(&launch, runtime_config, state);
    }

    let mut terminal = TerminalGuard::enter().context("failed to initialize terminal UI")?;
    let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
    let mut next_refresh_generation = 1;
    let mut account_load = account_load_runtime(&launch);
    let mut profile_validation_load = ProfileValidationLoadRuntime::new();
    let mut symbol_loads = SymbolLoadRuntimes::new();
    request_refresh(&scheduler, &mut state, &mut next_refresh_generation);
    request_profile_validation_load(&scheduler, &mut state, &mut profile_validation_load);
    request_account_load(
        &scheduler,
        &mut state,
        &mut account_load,
        AccountLoadMode::Cached,
    );
    request_symbol_loads(&scheduler, &mut state, &mut symbol_loads, false);

    let result = run_loop(
        terminal.terminal_mut()?,
        &mut state,
        LoopContext {
            refresh_seconds: runtime_config.refresh.price_seconds,
            history_refresh_seconds: runtime_config.refresh.research_seconds,
            scheduler: &scheduler,
            next_refresh_generation: &mut next_refresh_generation,
            symbol_loads: &mut symbol_loads,
            account_load: &mut account_load,
            profile_validation_load: &mut profile_validation_load,
            launch: &launch,
            runtime_config: &runtime_config,
            persisted_config: &persisted_config,
        },
    );
    let next_config = result.as_ref().map(|()| {
        let config = state.export_config(&runtime_config);
        launch.persistence_config(
            config,
            &persisted_config,
            state.preserve_launch_profile_override(),
        )
    });
    let restore_result = terminal.leave();
    let persist_result = next_config
        .as_ref()
        .map_or(Ok(()), |config| launch.persist_config(config));

    result.and(restore_result).and(persist_result)
}

fn run_dump_state(
    launch: &TuiLaunch,
    runtime_config: crate::config::TuiConfig,
    mut state: AppState,
) -> Result<()> {
    let options = launch
        .dump_state
        .context("dump-state options were not configured")?;
    let scheduler = Scheduler::start(launch, runtime_config.providers.clone());
    let mut next_refresh_generation = 1;
    let mut account_load = account_load_runtime(launch);
    let mut profile_validation_load = ProfileValidationLoadRuntime::new();
    let mut symbol_loads = SymbolLoadRuntimes::new();
    let deadline = Instant::now() + Duration::from_secs(options.wait_seconds);

    request_refresh(&scheduler, &mut state, &mut next_refresh_generation);
    request_profile_validation_load(&scheduler, &mut state, &mut profile_validation_load);
    request_account_load(
        &scheduler,
        &mut state,
        &mut account_load,
        AccountLoadMode::Cached,
    );
    request_symbol_loads(&scheduler, &mut state, &mut symbol_loads, false);

    while Instant::now() < deadline {
        drain_scheduler_events(&scheduler, &mut state);
        request_profile_validation_load(&scheduler, &mut state, &mut profile_validation_load);
        request_account_load(
            &scheduler,
            &mut state,
            &mut account_load,
            AccountLoadMode::Cached,
        );
        request_symbol_loads(&scheduler, &mut state, &mut symbol_loads, false);
        if dump_is_ready(&state) {
            break;
        }
        thread::sleep(launch.tick_rate.min(Duration::from_millis(100)));
    }
    drain_scheduler_events(&scheduler, &mut state);

    let partial = !dump_is_ready(&state);
    let dump = TuiDump::from_state(&state, partial);
    if options.json {
        println!("{}", serde_json::to_string_pretty(&dump)?);
    } else {
        println!(
            "{} {} panes={} tasks={} partial={}",
            dump.selected_symbol.as_deref().unwrap_or("N/A"),
            dump.workspace,
            dump.panes.iter().filter(|pane| pane.visible).count(),
            dump.tasks.len(),
            dump.partial
        );
    }
    Ok(())
}

fn run_loop(
    terminal: &mut TuiTerminal,
    state: &mut AppState,
    context: LoopContext<'_>,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();
    let mut mouse_drag = MouseDrag::default();
    let refresh_interval = context.refresh_seconds.max(2);
    let history_refresh_interval = context.history_refresh_seconds.max(60);
    loop {
        terminal.draw(|frame| render::render(frame, state))?;

        drain_scheduler_events(context.scheduler, state);
        request_profile_validation_load(context.scheduler, state, context.profile_validation_load);
        request_account_load(
            context.scheduler,
            state,
            context.account_load,
            AccountLoadMode::Cached,
        );
        request_symbol_loads(context.scheduler, state, context.symbol_loads, false);

        let timeout = context
            .launch
            .tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_default();
        if event::poll(timeout)? {
            let mut handled_input = false;
            match event::read()? {
                Event::Key(key) if input::should_quit(state, key) => break,
                Event::Key(key) => {
                    if let Some(action) = input::key_action(state, key) {
                        state.reduce(action);
                        handled_input = true;
                    }
                }
                Event::Mouse(mouse) => {
                    input::handle_mouse_event(
                        terminal.size()?.into(),
                        state,
                        &mut mouse_drag,
                        mouse,
                    );
                    handled_input = true;
                }
                _ => {}
            }
            if handled_input {
                drain_pending_app_requests(
                    PendingAppRequests {
                        scheduler: context.scheduler,
                        launch: context.launch,
                        runtime_config: context.runtime_config,
                        persisted_config: context.persisted_config,
                        next_refresh_generation: context.next_refresh_generation,
                        symbol_loads: context.symbol_loads,
                    },
                    state,
                );
                let account_load_mode = if state.take_pending_account_refresh() {
                    AccountLoadMode::UserRefresh
                } else {
                    AccountLoadMode::Cached
                };
                request_account_load(
                    context.scheduler,
                    state,
                    context.account_load,
                    account_load_mode,
                );
                request_profile_validation_load(
                    context.scheduler,
                    state,
                    context.profile_validation_load,
                );
                request_symbol_loads(context.scheduler, state, context.symbol_loads, false);
            }
        }

        if last_tick.elapsed() >= context.launch.tick_rate {
            last_tick = Instant::now();
        }

        if last_refresh.elapsed().as_secs() >= refresh_interval {
            request_refresh(context.scheduler, state, context.next_refresh_generation);
            last_refresh = Instant::now();
        }

        request_due_symbol_loads(
            context.scheduler,
            state,
            context.symbol_loads,
            history_refresh_interval,
        );
    }
    Ok(())
}

fn drain_scheduler_events(scheduler: &Scheduler, state: &mut AppState) {
    while let Some(event) = scheduler.try_recv() {
        apply_scheduler_event(state, event);
    }
}

fn account_load_runtime(launch: &TuiLaunch) -> AccountLoadRuntime {
    if launch.account_load {
        AccountLoadRuntime::new()
    } else {
        AccountLoadRuntime::disabled()
    }
}

fn apply_scheduler_event(state: &mut AppState, event: SchedulerEvent) {
    match event {
        SchedulerEvent::Snapshot {
            generation,
            snapshot,
        } => state.reduce(Action::SnapshotLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::RefreshFailed { generation, error } => {
            state.reduce(Action::RefreshFailed { generation, error })
        }
        SchedulerEvent::History {
            generation,
            snapshot,
        } => state.reduce(Action::HistoryLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::HistoryFailed {
            generation,
            symbol,
            error,
        } => state.reduce(Action::HistoryFailed {
            generation,
            symbol,
            error,
        }),
        SchedulerEvent::Evidence {
            generation,
            snapshot,
        } => state.reduce(Action::EvidenceLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::EvidenceFailed {
            generation,
            symbol,
            error,
        } => state.reduce(Action::EvidenceFailed {
            generation,
            symbol,
            error,
        }),
        SchedulerEvent::Research {
            generation,
            snapshot,
        } => state.reduce(Action::ResearchLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::Account {
            generation,
            snapshot,
        } => state.reduce(Action::AccountLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::AccountFailed {
            generation,
            profile,
            error,
        } => state.reduce(Action::AccountFailed {
            generation,
            profile,
            error,
        }),
        SchedulerEvent::ProfileValidation {
            generation,
            snapshot,
        } => state.reduce(Action::ProfileValidationLoaded {
            generation,
            snapshot,
        }),
        SchedulerEvent::ProfileValidationFailed {
            generation,
            profile,
            error,
        } => state.reduce(Action::ProfileValidationFailed {
            generation,
            profile,
            error,
        }),
        SchedulerEvent::StagedChangeProgress { id, event, message } => {
            state.reduce(Action::ApplyStagedChangeEvent { id, event });
            if let Some(message) = message {
                state.reduce(Action::Log(message));
            }
        }
        SchedulerEvent::Fatal(error) => state.reduce(Action::SchedulerFailed(error)),
    }
}

fn dump_is_ready(state: &AppState) -> bool {
    if state.scheduler_error.is_some() {
        return true;
    }
    if state.refresh_loading() {
        return false;
    }
    if state.account_loading() {
        return false;
    }
    if state.profile_validation_loading() {
        return false;
    }
    if state.market_snapshot.is_none() && !state.task_failures.has_source(TaskFailureSource::Quotes)
    {
        return false;
    }

    state.visible_panels().into_iter().all(|panel| match panel {
        Panel::History => !state.history.loading(),
        Panel::Evidence => !state.evidence.loading(),
        Panel::Polymarket | Panel::Research => !state.research.loading(),
        Panel::Account => !state.account_loading(),
        Panel::Watchlist
        | Panel::Quote
        | Panel::OrderTicket
        | Panel::TransferTicket
        | Panel::FuturesState
        | Panel::OpenOrders
        | Panel::IntentReview
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog
        | Panel::Settings
        | Panel::ProfileRisk => true,
    })
}

struct LoopContext<'a> {
    refresh_seconds: u64,
    history_refresh_seconds: u64,
    scheduler: &'a Scheduler,
    next_refresh_generation: &'a mut u64,
    symbol_loads: &'a mut SymbolLoadRuntimes,
    account_load: &'a mut AccountLoadRuntime,
    profile_validation_load: &'a mut ProfileValidationLoadRuntime,
    launch: &'a TuiLaunch,
    runtime_config: &'a TuiConfig,
    persisted_config: &'a TuiConfig,
}

fn request_refresh(scheduler: &Scheduler, state: &mut AppState, next_generation: &mut u64) {
    let Some(request) = prepare_refresh_request(state, next_generation) else {
        return;
    };

    if let Err(error) = scheduler.request_refresh(request.generation, request.symbols) {
        state.reduce(Action::SchedulerFailed(error.to_string()));
    }
}

fn request_symbol_loads(
    scheduler: &Scheduler,
    state: &mut AppState,
    runtimes: &mut SymbolLoadRuntimes,
    force: bool,
) {
    for kind in SymbolTaskKind::ALL {
        request_symbol_load(scheduler, state, runtimes.runtime_mut(kind), kind, force);
    }
}

fn request_provider_backed_symbol_loads(
    scheduler: &Scheduler,
    state: &mut AppState,
    runtimes: &mut SymbolLoadRuntimes,
) {
    for kind in [SymbolTaskKind::History, SymbolTaskKind::Evidence] {
        request_symbol_load(scheduler, state, runtimes.runtime_mut(kind), kind, true);
    }
}

fn request_due_symbol_loads(
    scheduler: &Scheduler,
    state: &mut AppState,
    runtimes: &mut SymbolLoadRuntimes,
    refresh_interval: u64,
) {
    for kind in SymbolTaskKind::ALL {
        let runtime = runtimes.runtime_mut(kind);
        if runtime.last_refresh.elapsed().as_secs() >= refresh_interval {
            request_symbol_load(scheduler, state, runtime, kind, true);
            runtime.last_refresh = Instant::now();
        }
    }
}

fn request_symbol_load(
    scheduler: &Scheduler,
    state: &mut AppState,
    runtime: &mut SymbolLoadRuntime,
    kind: SymbolTaskKind,
    force: bool,
) {
    let Some(request) = prepare_symbol_task_request(state, runtime, kind, force) else {
        return;
    };

    if let Err(error) = scheduler.request_symbol_task(kind, request.generation, request.symbol) {
        state.reduce(Action::SchedulerFailed(error.to_string()));
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RefreshRequest {
    generation: u64,
    symbols: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SymbolLoadRequest {
    generation: u64,
    symbol: String,
}

#[derive(Debug, Clone)]
struct SymbolLoadRuntime {
    next_generation: u64,
    last_refresh: Instant,
}

impl SymbolLoadRuntime {
    fn new() -> Self {
        Self {
            next_generation: 1,
            last_refresh: Instant::now(),
        }
    }

    fn next_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        generation
    }
}

#[derive(Debug, Clone)]
struct SymbolLoadRuntimes {
    history: SymbolLoadRuntime,
    evidence: SymbolLoadRuntime,
    research: SymbolLoadRuntime,
}

impl SymbolLoadRuntimes {
    fn new() -> Self {
        Self {
            history: SymbolLoadRuntime::new(),
            evidence: SymbolLoadRuntime::new(),
            research: SymbolLoadRuntime::new(),
        }
    }

    fn runtime_mut(&mut self, kind: SymbolTaskKind) -> &mut SymbolLoadRuntime {
        match kind {
            SymbolTaskKind::History => &mut self.history,
            SymbolTaskKind::Evidence => &mut self.evidence,
            SymbolTaskKind::Research => &mut self.research,
        }
    }
}

fn prepare_refresh_request(
    state: &mut AppState,
    next_generation: &mut u64,
) -> Option<RefreshRequest> {
    if state.refresh_loading() || state.scheduler_error.is_some() {
        return None;
    }

    let generation = *next_generation;
    *next_generation = next_generation.saturating_add(1);
    state.reduce(Action::RefreshStarted(generation));
    Some(RefreshRequest {
        generation,
        symbols: state.watchlist.clone(),
    })
}

fn prepare_symbol_task_request(
    state: &mut AppState,
    runtime: &mut SymbolLoadRuntime,
    kind: SymbolTaskKind,
    force: bool,
) -> Option<SymbolLoadRequest> {
    let request = match kind {
        SymbolTaskKind::History => {
            prepare_symbol_load_request(state, &state.history, runtime, force, |_| true)
        }
        SymbolTaskKind::Evidence => prepare_symbol_load_request(
            state,
            &state.evidence,
            runtime,
            force,
            is_likely_crypto_pair,
        ),
        SymbolTaskKind::Research => {
            prepare_symbol_load_request(state, &state.research, runtime, force, |_| true)
        }
    }?;
    state.reduce(start_symbol_task_action(kind, &request));
    Some(request)
}

fn start_symbol_task_action(kind: SymbolTaskKind, request: &SymbolLoadRequest) -> Action {
    match kind {
        SymbolTaskKind::History => Action::HistoryStarted {
            generation: request.generation,
            symbol: request.symbol.clone(),
        },
        SymbolTaskKind::Evidence => Action::EvidenceStarted {
            generation: request.generation,
            symbol: request.symbol.clone(),
        },
        SymbolTaskKind::Research => Action::ResearchStarted {
            generation: request.generation,
            symbol: request.symbol.clone(),
        },
    }
}

fn prepare_symbol_load_request<T>(
    state: &AppState,
    load: &SelectedSymbolLoad<T>,
    runtime: &mut SymbolLoadRuntime,
    force: bool,
    allow_symbol: impl Fn(&str) -> bool,
) -> Option<SymbolLoadRequest>
where
    T: SymbolSnapshot,
{
    if load.loading() || state.scheduler_error.is_some() {
        return None;
    }

    let symbol = state.selected_symbol()?.to_string();
    if !allow_symbol(&symbol) {
        return None;
    }

    if load.has_selected_snapshot(&symbol) && !force {
        return None;
    }

    let generation = runtime.next_generation();
    Some(SymbolLoadRequest { generation, symbol })
}

struct TerminalGuard {
    terminal: Option<TuiTerminal>,
    raw_mode: bool,
    alternate_screen: bool,
    mouse_capture: bool,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        let mut guard = Self {
            terminal: None,
            raw_mode: false,
            alternate_screen: false,
            mouse_capture: false,
        };

        enable_raw_mode()?;
        guard.raw_mode = true;

        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen) {
            let _ = guard.cleanup();
            return Err(error.into());
        }
        guard.alternate_screen = true;

        if let Err(error) = execute!(io::stdout(), EnableMouseCapture) {
            let _ = guard.cleanup();
            return Err(error.into());
        }
        guard.mouse_capture = true;

        let backend = CrosstermBackend::new(io::stdout());
        match Terminal::new(backend) {
            Ok(terminal) => {
                guard.terminal = Some(terminal);
                Ok(guard)
            }
            Err(error) => {
                let _ = guard.cleanup();
                Err(error.into())
            }
        }
    }

    fn terminal_mut(&mut self) -> Result<&mut TuiTerminal> {
        self.terminal
            .as_mut()
            .context("terminal UI was not initialized")
    }

    fn leave(&mut self) -> Result<()> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<()> {
        let mut first_error = None;

        if self.mouse_capture {
            let result = if let Some(terminal) = self.terminal.as_mut() {
                execute!(terminal.backend_mut(), DisableMouseCapture)
            } else {
                execute!(io::stdout(), DisableMouseCapture)
            };
            if let Err(error) = result {
                first_error.get_or_insert_with(|| anyhow::Error::from(error));
            }
            self.mouse_capture = false;
        }

        if self.alternate_screen {
            let result = if let Some(terminal) = self.terminal.as_mut() {
                execute!(terminal.backend_mut(), LeaveAlternateScreen)
            } else {
                execute!(io::stdout(), LeaveAlternateScreen)
            };
            if let Err(error) = result {
                first_error.get_or_insert_with(|| anyhow::Error::from(error));
            }
            self.alternate_screen = false;
        }

        if let Some(terminal) = self.terminal.as_mut()
            && let Err(error) = terminal.show_cursor()
        {
            first_error.get_or_insert_with(|| anyhow::Error::from(error));
        }

        if self.raw_mode {
            if let Err(error) = disable_raw_mode() {
                first_error.get_or_insert_with(|| anyhow::Error::from(error));
            }
            self.raw_mode = false;
        }

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
    use agent_finance_market::history_snapshot::HistorySnapshot;
    use agent_finance_market::research_snapshot::ResearchContextSnapshot;
    use agent_finance_market::snapshot::MarketSnapshot;

    #[test]
    fn refresh_request_does_not_enqueue_while_previous_refresh_is_in_flight() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        let mut next_generation = 1;

        let first = prepare_refresh_request(&mut state, &mut next_generation);
        assert_eq!(
            first,
            Some(RefreshRequest {
                generation: 1,
                symbols: state.watchlist.clone(),
            })
        );
        assert!(state.refresh_loading());
        assert_eq!(next_generation, 2);

        let second = prepare_refresh_request(&mut state, &mut next_generation);
        assert_eq!(second, None);
        assert_eq!(next_generation, 2);

        state.reduce(Action::SnapshotLoaded {
            generation: 1,
            snapshot: market_snapshot(),
        });
        assert!(state.market_snapshot.is_some());
        assert!(!state.refresh_loading());
    }

    #[test]
    fn refresh_request_does_not_enqueue_after_scheduler_fatal_failure() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        let mut next_generation = 1;

        state.reduce(Action::SchedulerFailed("scheduler failed".to_string()));

        assert_eq!(
            prepare_refresh_request(&mut state, &mut next_generation),
            None
        );
        assert_eq!(next_generation, 1);
    }

    #[test]
    fn dump_readiness_ignores_hidden_workspace_panel_loads() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            workspace: crate::config::WorkspaceConfig {
                current: crate::model::WorkspaceKind::Market,
            },
            ..crate::config::TuiConfig::default()
        });

        state.reduce(Action::RefreshStarted(1));
        state.reduce(Action::SnapshotLoaded {
            generation: 1,
            snapshot: market_snapshot(),
        });
        state.reduce(Action::ResearchStarted {
            generation: 2,
            symbol: "AAPL".to_string(),
        });

        assert!(dump_is_ready(&state));
    }

    #[test]
    fn history_request_enqueues_on_symbol_change_and_skips_same_symbol() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "AAPL".to_string(),
            })
        );
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            None
        );

        state.reduce(Action::HistoryLoaded {
            generation: 1,
            snapshot: history_snapshot("AAPL"),
        });
        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "CRDO".to_string(),
            })
        );
    }

    #[test]
    fn history_request_does_not_enqueue_while_in_flight_or_after_scheduler_failure() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        let mut runtime = SymbolLoadRuntime::new();

        assert!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false)
                .is_some()
        );
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, true),
            None
        );
        assert_eq!(runtime.next_generation, 2);

        state.reduce(Action::SchedulerFailed("scheduler failed".to_string()));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, true),
            None
        );
    }

    #[test]
    fn provider_preference_update_invalidates_in_flight_provider_backed_loads() {
        let launch = crate::config::TuiLaunch::new(Vec::new(), None, true);
        let runtime_config = crate::config::TuiConfig {
            watchlist: vec!["BTCUSDT".to_string()],
            ..crate::config::TuiConfig::default()
        };
        let persisted_config = runtime_config.clone();
        let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
        let mut state = AppState::from_config(runtime_config.clone());
        let mut next_refresh_generation = 1;
        let mut symbol_loads = SymbolLoadRuntimes::new();

        assert!(prepare_refresh_request(&mut state, &mut next_refresh_generation).is_some());
        for kind in SymbolTaskKind::ALL {
            assert!(
                prepare_symbol_task_request(
                    &mut state,
                    symbol_loads.runtime_mut(kind),
                    kind,
                    false
                )
                .is_some(),
                "{kind:?} should start"
            );
        }
        state.reduce(Action::AdjustSelectedSetting(1));

        drain_pending_app_requests(
            PendingAppRequests {
                scheduler: &scheduler,
                launch: &launch,
                runtime_config: &runtime_config,
                persisted_config: &persisted_config,
                next_refresh_generation: &mut next_refresh_generation,
                symbol_loads: &mut symbol_loads,
            },
            &mut state,
        );

        assert_eq!(next_refresh_generation, 3);
        assert!(state.refresh_loading());
        assert!(state.history.loading());
        assert!(state.evidence.loading());
        assert!(state.research.loading());
        assert_eq!(symbol_loads.history.next_generation, 3);
        assert_eq!(symbol_loads.evidence.next_generation, 3);
        assert_eq!(symbol_loads.research.next_generation, 2);

        state.reduce(Action::SnapshotLoaded {
            generation: 1,
            snapshot: market_snapshot(),
        });
        state.reduce(Action::HistoryLoaded {
            generation: 1,
            snapshot: history_snapshot("BTCUSDT"),
        });
        state.reduce(Action::EvidenceLoaded {
            generation: 1,
            snapshot: evidence_snapshot("BTCUSDT"),
        });
        state.reduce(Action::ResearchLoaded {
            generation: 1,
            snapshot: research_snapshot("BTCUSDT"),
        });

        assert!(state.market_snapshot.is_none());
        assert!(state.history.selected_snapshot("BTCUSDT").is_none());
        assert!(state.evidence.selected_snapshot("BTCUSDT").is_none());
        assert!(state.research.selected_snapshot("BTCUSDT").is_some());
    }

    #[test]
    fn failed_history_request_does_not_count_as_loaded() {
        let mut state = AppState::from_config(crate::config::TuiConfig::default());
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "AAPL".to_string(),
            })
        );
        state.reduce(Action::HistoryFailed {
            generation: 1,
            symbol: "AAPL".to_string(),
            error: "network".to_string(),
        });

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "AAPL".to_string(),
            })
        );
    }

    #[test]
    fn history_request_follows_selected_symbol_after_previous_in_flight_request_finishes() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "AAPL".to_string(),
            })
        );
        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            None
        );

        state.reduce(Action::HistoryLoaded {
            generation: 1,
            snapshot: history_snapshot("AAPL"),
        });
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::History, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "CRDO".to_string(),
            })
        );
    }

    #[test]
    fn evidence_request_follows_selected_symbol_after_previous_in_flight_request_finishes() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "BTCUSDT".to_string(),
            })
        );
        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            None
        );

        state.reduce(Action::EvidenceLoaded {
            generation: 1,
            snapshot: evidence_snapshot("BTCUSDT"),
        });
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "ETHUSDT".to_string(),
            })
        );
    }

    #[test]
    fn failed_evidence_request_does_not_count_as_loaded() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["BTCUSDT".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "BTCUSDT".to_string(),
            })
        );
        state.reduce(Action::EvidenceFailed {
            generation: 1,
            symbol: "BTCUSDT".to_string(),
            error: "network".to_string(),
        });

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "BTCUSDT".to_string(),
            })
        );
    }

    #[test]
    fn research_request_follows_selected_symbol_after_previous_in_flight_request_finishes() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Research, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "AAPL".to_string(),
            })
        );
        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Research, false),
            None
        );

        state.reduce(Action::ResearchLoaded {
            generation: 1,
            snapshot: research_snapshot("AAPL"),
        });
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Research, false),
            Some(SymbolLoadRequest {
                generation: 2,
                symbol: "CRDO".to_string(),
            })
        );
    }

    #[test]
    fn evidence_request_only_enqueues_for_crypto_pair_shapes() {
        let mut state = AppState::from_config(crate::config::TuiConfig {
            watchlist: vec!["AAPL".to_string(), "BTCUSDT".to_string()],
            ..crate::config::TuiConfig::default()
        });
        let mut runtime = SymbolLoadRuntime::new();

        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            None
        );
        assert_eq!(runtime.next_generation, 1);

        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        assert_eq!(
            prepare_symbol_task_request(&mut state, &mut runtime, SymbolTaskKind::Evidence, false),
            Some(SymbolLoadRequest {
                generation: 1,
                symbol: "BTCUSDT".to_string(),
            })
        );
    }

    fn history_snapshot(symbol: &str) -> HistorySnapshot {
        HistorySnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            provider: "test".to_string(),
            interval: "1d".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            latest_close: Some(100.0),
            latest_time: Some("2026-06-25".to_string()),
            return_pct: Some(1.0),
            volume: Some(10_000.0),
            bars: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn evidence_snapshot(symbol: &str) -> CryptoQuoteEvidenceSnapshot {
        CryptoQuoteEvidenceSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            instrument: "spot".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            ok_providers: 1,
            total_providers: 1,
            providers: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn research_snapshot(symbol: &str) -> ResearchContextSnapshot {
        ResearchContextSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            news: Vec::new(),
            prediction_markets: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn market_snapshot() -> MarketSnapshot {
        MarketSnapshot {
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            quotes: Vec::new(),
            errors: Vec::new(),
        }
    }
}
