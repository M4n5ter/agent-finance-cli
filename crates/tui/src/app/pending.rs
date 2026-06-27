use crate::config::{TuiConfig, TuiLaunch};
use crate::scheduler::Scheduler;
use crate::state::{Action, AppState, StagedChangeEvent};

pub(super) struct PendingAppRequests<'a> {
    pub scheduler: &'a Scheduler,
    pub launch: &'a TuiLaunch,
    pub runtime_config: &'a TuiConfig,
    pub persisted_config: &'a TuiConfig,
}

pub(super) fn drain_pending_app_requests(context: PendingAppRequests<'_>, state: &mut AppState) {
    request_pending_staged_submit(context.scheduler, state);
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
    let config = context
        .launch
        .persistence_config(config, context.persisted_config);
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
    fn explicit_config_save_respects_no_persist() {
        let path = unique_temp_config_path("explicit-save-no-persist");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), true);
        let runtime_config = TuiConfig::default();
        let persisted_config = TuiConfig::default();
        let mut state = AppState::from_config(runtime_config.clone());

        state.reduce(Action::ResizeDockedColumns {
            left_ratio: 31,
            main_ratio: 42,
        });
        state.reduce(Action::RequestConfigSave);

        persist_pending_config_save(
            &PendingAppRequests {
                scheduler: &Scheduler::start(&launch, runtime_config.providers.clone()),
                launch: &launch,
                runtime_config: &runtime_config,
                persisted_config: &persisted_config,
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
