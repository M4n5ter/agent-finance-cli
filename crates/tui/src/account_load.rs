use crate::scheduler::Scheduler;
use crate::state::{Action, AppState};

#[derive(Debug, Clone)]
pub(crate) struct AccountLoadRuntime {
    next_generation: u64,
    enabled: bool,
}

impl AccountLoadRuntime {
    pub(crate) const fn new() -> Self {
        Self {
            next_generation: 1,
            enabled: true,
        }
    }

    pub(crate) const fn disabled() -> Self {
        Self {
            next_generation: 1,
            enabled: false,
        }
    }

    fn next_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        generation
    }
}

pub(crate) fn request_account_load(
    scheduler: &Scheduler,
    state: &mut AppState,
    runtime: &mut AccountLoadRuntime,
    force: bool,
) {
    let Some(request) = prepare_account_load_request(state, runtime, force) else {
        return;
    };

    if let Err(error) = scheduler.request_account(request.generation, request.profile) {
        state.reduce(Action::SchedulerFailed(error.to_string()));
    }
}

fn prepare_account_load_request(
    state: &mut AppState,
    runtime: &mut AccountLoadRuntime,
    force: bool,
) -> Option<AccountLoadRequest> {
    if !runtime.enabled {
        return None;
    }
    if state.account_loading() || state.scheduler_error.is_some() {
        return None;
    }
    let profile = state.trading_profile.clone()?;
    if !force
        && state
            .account_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.profile == profile)
    {
        return None;
    }

    let generation = runtime.next_generation();
    state.reduce(Action::AccountStarted {
        generation,
        profile: profile.clone(),
    });
    Some(AccountLoadRequest {
        generation,
        profile,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct AccountLoadRequest {
    generation: u64,
    profile: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{TradingConfig, TuiConfig};

    #[test]
    fn disabled_account_load_runtime_does_not_enqueue_profile_reads() {
        let mut state = AppState::from_config(TuiConfig {
            trading: TradingConfig {
                default_profile: Some("smoke".to_string()),
            },
            ..TuiConfig::default()
        });
        let mut runtime = AccountLoadRuntime::disabled();

        let request = prepare_account_load_request(&mut state, &mut runtime, false);

        assert!(request.is_none());
        assert!(!state.account_loading());
    }
}
