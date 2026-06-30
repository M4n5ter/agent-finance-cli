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
    mode: AccountLoadMode,
) {
    let request = match prepare_account_load_request(state, runtime, mode) {
        AccountLoadPlan::Request(request) => request,
        AccountLoadPlan::Skip(reason) => {
            if mode == AccountLoadMode::UserRefresh
                && let Some(message) = reason.user_message()
            {
                state.reduce(Action::LogWarning(message));
            }
            return;
        }
    };

    if let Err(error) = scheduler.request_account(request.generation, request.profile) {
        state.reduce(Action::SchedulerFailed(error.to_string()));
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AccountLoadMode {
    Cached,
    UserRefresh,
}

fn prepare_account_load_request(
    state: &mut AppState,
    runtime: &mut AccountLoadRuntime,
    mode: AccountLoadMode,
) -> AccountLoadPlan {
    if !runtime.enabled {
        return AccountLoadPlan::Skip(AccountLoadSkip::Disabled);
    }
    if state.account_loading() {
        return AccountLoadPlan::Skip(AccountLoadSkip::Loading {
            profile: state.trading_profile.clone(),
        });
    }
    if state.scheduler_error.is_some() {
        return AccountLoadPlan::Skip(AccountLoadSkip::SchedulerFailed);
    }
    let Some(profile) = state.trading_profile.clone() else {
        return AccountLoadPlan::Skip(AccountLoadSkip::MissingProfile);
    };
    if mode == AccountLoadMode::Cached
        && state
            .account_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.profile == profile)
    {
        return AccountLoadPlan::Skip(AccountLoadSkip::AlreadyLoaded);
    }

    let generation = runtime.next_generation();
    state.reduce(Action::AccountStarted {
        generation,
        profile: profile.clone(),
    });
    AccountLoadPlan::Request(AccountLoadRequest {
        generation,
        profile,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum AccountLoadPlan {
    Request(AccountLoadRequest),
    Skip(AccountLoadSkip),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum AccountLoadSkip {
    Disabled,
    Loading { profile: Option<String> },
    SchedulerFailed,
    MissingProfile,
    AlreadyLoaded,
}

impl AccountLoadSkip {
    fn user_message(self) -> Option<String> {
        match self {
            Self::Disabled => Some(
                "account refresh unavailable: signed account loading is disabled for this launch"
                    .to_string(),
            ),
            Self::Loading { profile } => Some(format!(
                "{} account refresh is already loading",
                profile.as_deref().unwrap_or("account")
            )),
            Self::SchedulerFailed => Some("account refresh blocked: scheduler failed".to_string()),
            Self::MissingProfile => {
                Some("no trading profile selected for account refresh".to_string())
            }
            Self::AlreadyLoaded => None,
        }
    }
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

        let request =
            prepare_account_load_request(&mut state, &mut runtime, AccountLoadMode::Cached);

        assert_eq!(request, AccountLoadPlan::Skip(AccountLoadSkip::Disabled));
        assert!(!state.account_loading());
    }

    #[test]
    fn forced_account_load_reloads_current_profile_snapshot() {
        let mut state = AppState::from_config(TuiConfig {
            trading: TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.account_snapshot = Some(crate::account::AccountSnapshot::new(
            "mainnet".to_string(),
            agent_finance_core::Provider::Binance,
            agent_finance_core::Environment::Testnet,
            crate::profile_snapshot::test_trading_profile_snapshot(),
            Vec::new(),
            Vec::new(),
        ));
        let mut runtime = AccountLoadRuntime::new();

        let skipped =
            prepare_account_load_request(&mut state, &mut runtime, AccountLoadMode::Cached);
        let forced =
            prepare_account_load_request(&mut state, &mut runtime, AccountLoadMode::UserRefresh);

        assert_eq!(
            skipped,
            AccountLoadPlan::Skip(AccountLoadSkip::AlreadyLoaded)
        );
        assert_eq!(
            forced,
            AccountLoadPlan::Request(AccountLoadRequest {
                generation: 1,
                profile: "mainnet".to_string(),
            })
        );
        assert!(state.account_loading());
    }

    #[test]
    fn user_refresh_skip_reasons_are_user_visible() {
        assert_eq!(
            AccountLoadSkip::Disabled.user_message(),
            Some(
                "account refresh unavailable: signed account loading is disabled for this launch"
                    .to_string()
            )
        );
        assert_eq!(
            AccountLoadSkip::MissingProfile.user_message(),
            Some("no trading profile selected for account refresh".to_string())
        );
        assert_eq!(AccountLoadSkip::AlreadyLoaded.user_message(), None);
    }
}
