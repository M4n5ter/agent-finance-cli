use crate::config::{TuiConfig, TuiLaunch};
use crate::scheduler::Scheduler;
use crate::state::{Action, AppState, StagedChangeEvent};

use super::{SymbolLoadRuntimes, request_provider_backed_symbol_loads, request_refresh};

pub(super) struct PendingAppRequests<'a> {
    pub scheduler: &'a Scheduler,
    pub launch: &'a TuiLaunch,
    pub runtime_config: &'a TuiConfig,
    pub persisted_config: &'a TuiConfig,
    pub next_refresh_generation: &'a mut u64,
    pub symbol_loads: &'a mut SymbolLoadRuntimes,
}

pub(super) fn drain_pending_app_requests(
    mut context: PendingAppRequests<'_>,
    state: &mut AppState,
) {
    request_pending_staged_submit(context.scheduler, state);
    apply_pending_provider_preferences(&mut context, state);
    persist_pending_config_save(&context, state);
}

fn request_pending_staged_submit(scheduler: &Scheduler, state: &mut AppState) {
    let Some(request) = state.take_pending_staged_submit() else {
        return;
    };
    let id = request.id.clone();
    match scheduler.request_staged_submit(request) {
        Ok(()) => {}
        Err(error) => {
            state.reduce(Action::ApplyStagedChangeEvent {
                id,
                event: StagedChangeEvent::FailedBeforeIntent,
            });
            state.reduce(Action::Log(error.to_string()));
        }
    }
}

fn apply_pending_provider_preferences(context: &mut PendingAppRequests<'_>, state: &mut AppState) {
    let Some(providers) = state.take_pending_provider_preferences_update() else {
        return;
    };

    match context.scheduler.update_provider_policy(providers) {
        Ok(()) => {
            state.invalidate_provider_backed_loads();
            request_refresh(context.scheduler, state, context.next_refresh_generation);
            request_provider_backed_symbol_loads(context.scheduler, state, context.symbol_loads);
        }
        Err(error) => state.reduce(Action::SchedulerFailed(error.to_string())),
    }
}

fn persist_pending_config_save(context: &PendingAppRequests<'_>, state: &mut AppState) {
    if !state.take_pending_config_save() {
        return;
    }
    if context.launch.no_persist {
        state.reduce(Action::ConfigSaveFailed(
            "config persistence is disabled for this launch".to_string(),
        ));
        return;
    }

    let config = state.export_config(context.runtime_config);
    let config = context.launch.persistence_config(
        config,
        context.persisted_config,
        state.preserve_launch_profile_override(),
    );
    match context.launch.persist_config(&config) {
        Ok(()) => state.reduce(Action::ConfigSaved),
        Err(error) => state.reduce(Action::ConfigSaveFailed(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn explicit_config_save_persists_runtime_config_and_clears_dirty() {
        let path = unique_temp_config_path("explicit-save");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), false);
        let runtime_config = TuiConfig::default();
        let persisted_config = TuiConfig::default();
        let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
        let mut state = AppState::from_config(runtime_config.clone());
        let mut next_refresh_generation = 1;
        let mut symbol_loads = SymbolLoadRuntimes::new();

        state.reduce(Action::ResizeDockedColumns {
            left_ratio: 31,
            main_ratio: 42,
        });
        state.reduce(Action::RequestConfigSave);

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
        let loaded = launch.load_config().expect("load saved config");
        let _ = fs::remove_file(path);

        assert!(state.config_changes.is_empty());
        assert_eq!(loaded.layout.left_ratio, 31);
        assert_eq!(loaded.layout.main_ratio, 42);
    }

    #[test]
    fn explicit_config_save_persists_user_edited_profile_under_launch_profile_override() {
        let path = unique_temp_config_path("explicit-save-profile");
        let persisted_config = TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("paper".to_string()),
            },
            ..TuiConfig::default()
        };
        persisted_config
            .save_to(&path)
            .expect("write persisted config");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), false)
            .with_profile(Some("runtime".to_string()));
        let runtime_config = launch.runtime_config(persisted_config.clone());
        let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
        let mut state = AppState::from_config(runtime_config.clone());
        let mut next_refresh_generation = 1;
        let mut symbol_loads = SymbolLoadRuntimes::new();

        state.reduce(Action::Execute(crate::command::ActionId::OpenFloating(
            crate::model::FloatingKind::TradingProfile,
        )));
        for _ in 0.."runtime".len() {
            state.reduce(Action::EditTradingProfileQuery(
                tui_input::InputRequest::DeletePrevChar,
            ));
        }
        for character in "mainnet".chars() {
            state.reduce(Action::EditTradingProfileQuery(
                tui_input::InputRequest::InsertChar(character),
            ));
        }
        state.reduce(Action::AcceptTradingProfile);
        state.reduce(Action::RequestConfigSave);

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
        let loaded = launch.load_config().expect("load saved config");
        let exit_config = launch.persistence_config(
            state.export_config(&runtime_config),
            &persisted_config,
            state.preserve_launch_profile_override(),
        );
        let _ = fs::remove_file(path);

        assert!(state.config_changes.is_empty());
        assert_eq!(loaded.trading.default_profile.as_deref(), Some("mainnet"));
        assert_eq!(
            exit_config.trading.default_profile.as_deref(),
            Some("mainnet")
        );
    }

    #[test]
    fn explicit_config_save_respects_no_persist() {
        let path = unique_temp_config_path("explicit-save-no-persist");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), true);
        let runtime_config = TuiConfig::default();
        let persisted_config = TuiConfig::default();
        let mut state = AppState::from_config(runtime_config.clone());
        let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
        let mut next_refresh_generation = 1;
        let mut symbol_loads = SymbolLoadRuntimes::new();

        state.reduce(Action::ResizeDockedColumns {
            left_ratio: 31,
            main_ratio: 42,
        });
        state.reduce(Action::RequestConfigSave);

        persist_pending_config_save(
            &PendingAppRequests {
                scheduler: &scheduler,
                launch: &launch,
                runtime_config: &runtime_config,
                persisted_config: &persisted_config,
                next_refresh_generation: &mut next_refresh_generation,
                symbol_loads: &mut symbol_loads,
            },
            &mut state,
        );

        assert_eq!(state.config_changes, ["layout"]);
        assert!(!path.exists());
    }

    fn unique_temp_config_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-tui-app-{name}-{nanos}.toml"))
    }
}
