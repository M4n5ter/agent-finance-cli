use crate::config::{TuiConfig, TuiLaunch};
use crate::profile_snapshot::ProfileValidationSnapshot;
use crate::scheduler::Scheduler;
use crate::state::{
    Action, AppState, StagedChangeEvent, StagedExecution, StagedLocalCommitSubject,
    StagedSubmitRequest,
};

use super::request_symbol_load;
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
    execute_pending_staged_change(context.scheduler, state);
    apply_pending_market_data_requests(&mut context, state);
    apply_pending_provider_preferences(&mut context, state);
    persist_pending_config_save(&context, state);
}

fn apply_pending_market_data_requests(context: &mut PendingAppRequests<'_>, state: &mut AppState) {
    if state.take_pending_market_refresh() {
        request_refresh(context.scheduler, state, context.next_refresh_generation);
    }

    for kind in state.take_pending_symbol_data_refreshes() {
        request_symbol_load(
            context.scheduler,
            state,
            context.symbol_loads.runtime_mut(kind),
            kind,
            true,
        );
    }
}

fn execute_pending_staged_change(scheduler: &Scheduler, state: &mut AppState) {
    let Some(request) = state.take_pending_staged_execution() else {
        return;
    };
    match request.execution {
        StagedExecution::Submit { subject, mode } => request_pending_staged_submit(
            scheduler,
            state,
            StagedSubmitRequest {
                id: request.id,
                subject,
                mode,
            },
        ),
        StagedExecution::LocalCommit { subject } => {
            commit_pending_staged_local_change(state, request.id, subject);
        }
    }
}

fn request_pending_staged_submit(
    scheduler: &Scheduler,
    state: &mut AppState,
    request: crate::state::StagedSubmitRequest,
) {
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

fn commit_pending_staged_local_change(
    state: &mut AppState,
    id: String,
    subject: StagedLocalCommitSubject,
) {
    match subject {
        StagedLocalCommitSubject::ProfileRisk(review) => {
            let committed = profile_store_for_review_path(&review.path).and_then(|store| {
                let expected_path = store.path(&review.profile);
                if expected_path != review.path {
                    anyhow::bail!(
                        "profile review path {} does not match profile store path {}",
                        review.path.display(),
                        expected_path.display()
                    );
                }
                store
                    .plan_replace_unchanged(&review.expected_content_hash, &review.next_profile)
                    .and_then(|plan| store.commit_write_plan(plan))
            });
            match committed {
                Ok(report) => {
                    let backup = report
                        .backup_path
                        .as_ref()
                        .map(|path| format!(" with backup {}", path.display()))
                        .unwrap_or_default();
                    state.reduce(Action::ProfileRiskCommitSucceeded {
                        id,
                        snapshot: ProfileValidationSnapshot::from_profile(
                            &review.next_profile,
                            report.path.clone(),
                        ),
                        message: format!(
                            "committed profile risk change for {} to {}{}",
                            report.profile,
                            report.path.display(),
                            backup
                        ),
                    });
                }
                Err(error) => state.reduce(Action::ProfileRiskCommitFailed {
                    id,
                    error: format!("failed to commit profile risk change: {error:#}"),
                }),
            }
        }
    }
}

fn profile_store_for_review_path(
    path: &std::path::Path,
) -> anyhow::Result<agent_finance_core::ProfileStore> {
    path.parent()
        .map(agent_finance_core::ProfileStore::from_root)
        .ok_or_else(|| anyhow::anyhow!("profile path {} has no parent directory", path.display()))
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
    use crate::profile_snapshot::{
        ProfileValidationSnapshot, ProfileValidationState, test_profile,
    };

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

    #[test]
    fn pending_market_data_requests_enqueue_scheduler_loads() {
        let launch = TuiLaunch::new(Vec::new(), None, true);
        let runtime_config = TuiConfig {
            watchlist: vec!["BTCUSDT".to_string()],
            ..TuiConfig::default()
        };
        let persisted_config = runtime_config.clone();
        let scheduler = Scheduler::start(&launch, runtime_config.providers.clone());
        let mut state = AppState::from_config(runtime_config.clone());
        let mut next_refresh_generation = 1;
        let mut symbol_loads = SymbolLoadRuntimes::new();

        state.reduce(Action::Execute(
            crate::command::ActionId::RefreshMarketSnapshot,
        ));
        state.reduce(Action::Execute(
            crate::command::ActionId::RefreshSelectedHistory,
        ));
        state.reduce(Action::Execute(
            crate::command::ActionId::RefreshSelectedEvidence,
        ));
        state.reduce(Action::Execute(
            crate::command::ActionId::RefreshSelectedResearch,
        ));

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

        assert_eq!(next_refresh_generation, 2);
        assert!(state.refresh_loading());
        assert!(state.history.loading());
        assert!(state.evidence.loading());
        assert!(state.research.loading());
    }

    #[test]
    fn pending_profile_risk_local_commit_writes_profile_backup_and_refreshes_validation() {
        let root = unique_temp_profile_dir("profile-risk-commit");
        fs::create_dir_all(&root).expect("create temp profile dir");
        let store = agent_finance_core::ProfileStore::from_root(&root);
        let profile = test_profile("mainnet");
        store.write(&profile).expect("write source profile");
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(&profile, store.path("mainnet")),
        });
        state.reduce(Action::Execute(
            crate::command::ActionId::StageProfileLiveToggle,
        ));
        state.reduce(Action::ExecuteStagedChange);
        state.reduce(Action::ConfirmStagedExecution);

        commit_pending_staged_local_change_for_test(&mut state);

        let updated = store.load("mainnet").expect("load updated profile");
        assert!(!updated.risk.allow_live);
        let backups = fs::read_dir(&root)
            .expect("read profile dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".bak-"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            state.staged_change_views()[0].stage,
            crate::state::StagedChangeStage::LocalCommitted
        );
        assert!(matches!(
            &state.profile_validation,
            ProfileValidationState::Ready {
                profile,
                profile_config,
                ..
            } if profile == "mainnet" && !profile_config.risk.allow_live
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pending_profile_risk_local_commit_rejects_profile_changed_after_staging() {
        let root = unique_temp_profile_dir("profile-risk-stale");
        fs::create_dir_all(&root).expect("create temp profile dir");
        let store = agent_finance_core::ProfileStore::from_root(&root);
        let profile = test_profile("mainnet");
        store.write(&profile).expect("write source profile");
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(&profile, store.path("mainnet")),
        });
        state.reduce(Action::Execute(
            crate::command::ActionId::StageProfileLiveToggle,
        ));
        state.reduce(Action::ExecuteStagedChange);
        state.reduce(Action::ConfirmStagedExecution);
        let mut external = profile.clone();
        external.permissions.spot_trading = false;
        store.write(&external).expect("external profile edit");

        commit_pending_staged_local_change_for_test(&mut state);

        let loaded = store.load("mainnet").expect("load current profile");
        assert!(!loaded.permissions.spot_trading);
        assert!(loaded.risk.allow_live);
        assert_eq!(
            state.staged_change_views()[0].stage,
            crate::state::StagedChangeStage::LocalCommitFailed
        );
        assert!(state.task_log.iter().any(|entry| {
            entry
                .message
                .contains("changed after validation; revalidate before replacing it")
        }));
        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_config_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-tui-app-{name}-{nanos}.toml"))
    }

    fn unique_temp_profile_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-tui-app-{name}-{nanos}"))
    }

    fn commit_pending_staged_local_change_for_test(state: &mut AppState) {
        let request = state
            .take_pending_staged_execution()
            .expect("pending staged execution");
        let StagedExecution::LocalCommit { subject } = request.execution else {
            panic!("expected local commit execution");
        };
        commit_pending_staged_local_change(state, request.id, subject);
    }
}
