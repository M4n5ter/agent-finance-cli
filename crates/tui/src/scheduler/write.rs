use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use agent_finance_core::{Profile, SubmitMode};
use agent_finance_trading::TradingRuntime;
use anyhow::Result;

use crate::config::TuiLaunch;
use crate::staged_intent::{
    cancel_intent_from_review, futures_state_intent_from_review, generated_order_client_id,
    generated_transfer_client_id, order_intent_from_review, transfer_intent_from_review,
};
use crate::state::{
    CancelReview, FuturesStateReview, StagedChangeEvent, StagedSubmitRequest, StagedSubmitSubject,
    TransferReview,
};

use super::{SchedulerEvent, scheduler_runtime};

#[derive(Debug)]
pub(super) enum WriteCommand {
    SubmitStaged(StagedSubmitRequest),
}

pub(super) fn spawn_write_worker(
    launch: &TuiLaunch,
    commands: Receiver<WriteCommand>,
    events: Sender<SchedulerEvent>,
) {
    let runtime = TradingRuntime::with_http_policy(
        launch.timeout_seconds,
        launch.proxy.clone(),
        launch.no_proxy,
    );
    thread::Builder::new()
        .name("agent-finance-tui-write".to_string())
        .spawn(move || {
            let Some(tokio) = scheduler_runtime("write", &events) else {
                return;
            };

            while let Ok(command) = commands.recv() {
                if !handle_write_command(&tokio, &runtime, command, &events) {
                    break;
                }
            }
        })
        .unwrap_or_else(|error| panic!("failed to spawn TUI write scheduler thread: {error}"));
}

fn handle_write_command(
    tokio: &tokio::runtime::Runtime,
    runtime: &TradingRuntime,
    command: WriteCommand,
    events: &Sender<SchedulerEvent>,
) -> bool {
    match command {
        WriteCommand::SubmitStaged(request) => {
            handle_staged_submit(tokio, runtime, request, events)
        }
    }
}

fn handle_staged_submit(
    tokio: &tokio::runtime::Runtime,
    runtime: &TradingRuntime,
    request: StagedSubmitRequest,
    events: &Sender<SchedulerEvent>,
) -> bool {
    let created = create_staged_intent(runtime, &request);
    let (profile, intent_id) = match created {
        Ok((profile, intent_id)) => {
            if !send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::IntentCreated {
                    intent_id: intent_id.clone(),
                },
                Some(format!(
                    "created {} intent {intent_id}",
                    request.subject.kind_label()
                )),
            ) {
                return false;
            }
            (profile, intent_id)
        }
        Err(error) => {
            return send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::FailedBeforeIntent,
                Some(format!("{error:#}")),
            );
        }
    };

    let result = tokio.block_on(submit_staged_intent(
        runtime,
        &profile,
        &intent_id,
        request.mode,
        &request.subject,
    ));
    match (request.mode, result) {
        (SubmitMode::DryRun | SubmitMode::Test, Ok(_)) => send_staged_change_progress(
            events,
            &request.id,
            StagedChangeEvent::NonConsumingFinished {
                intent_id,
                mode: request.mode,
            },
            Some(format!("{} submit completed", request.mode)),
        ),
        (SubmitMode::DryRun | SubmitMode::Test, Err(error)) => send_staged_change_progress(
            events,
            &request.id,
            StagedChangeEvent::PreflightFailed {
                intent_id,
                attempted_mode: request.mode,
            },
            Some(error.to_string()),
        ),
        (SubmitMode::Live, Ok(_)) => {
            send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveIntentClaimed {
                    intent_id: intent_id.clone(),
                },
                None,
            ) && send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveSubmitSucceeded { intent_id },
                Some("live submit completed".to_string()),
            )
        }
        (SubmitMode::Live, Err(error)) if error.exchange_was_accepted() => {
            send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveIntentClaimed {
                    intent_id: intent_id.clone(),
                },
                None,
            ) && send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveSubmitSucceeded { intent_id },
                Some(format!(
                    "exchange accepted the live submit, but local finalization failed: {error}"
                )),
            )
        }
        (SubmitMode::Live, Err(error)) if error.exchange_was_attempted() => {
            send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveIntentClaimed {
                    intent_id: intent_id.clone(),
                },
                None,
            ) && send_staged_change_progress(
                events,
                &request.id,
                StagedChangeEvent::LiveSubmitFailed { intent_id },
                Some(error.to_string()),
            )
        }
        (SubmitMode::Live, Err(error)) => send_staged_change_progress(
            events,
            &request.id,
            StagedChangeEvent::PreflightFailed {
                intent_id,
                attempted_mode: SubmitMode::Live,
            },
            Some(error.to_string()),
        ),
    }
}

fn send_staged_change_progress(
    events: &Sender<SchedulerEvent>,
    id: &str,
    event: StagedChangeEvent,
    message: Option<String>,
) -> bool {
    events
        .send(SchedulerEvent::StagedChangeProgress {
            id: id.to_string(),
            event,
            message,
        })
        .is_ok()
}

fn create_staged_order_intent(
    runtime: &TradingRuntime,
    review: &crate::state::OrderTicketReview,
    mode: SubmitMode,
) -> Result<(Profile, String)> {
    let profile = runtime.load_profile(&review.profile)?;
    let intent = order_intent_from_review(&profile, review, generated_order_client_id());
    let risk =
        runtime.check_order_with_runtime_limits(&profile, &intent, mode == SubmitMode::Live)?;
    let envelope = agent_finance_core::create_order_intent(intent, 300)?;
    runtime.save_intent_with_audit(
        &profile,
        &envelope,
        &risk,
        format!("created TUI order intent {}", envelope.id),
    )?;
    Ok((profile, envelope.id))
}

fn create_staged_intent(
    runtime: &TradingRuntime,
    request: &StagedSubmitRequest,
) -> Result<(Profile, String)> {
    match &request.subject {
        StagedSubmitSubject::OrderTicket(review) => {
            create_staged_order_intent(runtime, review, request.mode)
        }
        StagedSubmitSubject::Cancel(review) => {
            create_staged_cancel_intent(runtime, review, request.mode)
        }
        StagedSubmitSubject::Transfer(review) => {
            create_staged_transfer_intent(runtime, review, request.mode)
        }
        StagedSubmitSubject::FuturesState(review) => {
            create_staged_futures_state_intent(runtime, review, request.mode)
        }
        #[cfg(test)]
        StagedSubmitSubject::Text { .. } => unreachable!("text changes are never submitted"),
    }
}

fn create_staged_cancel_intent(
    runtime: &TradingRuntime,
    review: &CancelReview,
    mode: SubmitMode,
) -> Result<(Profile, String)> {
    let profile = runtime.load_profile(&review.profile)?;
    let intent = cancel_intent_from_review(&profile, review);
    let risk = agent_finance_core::check_cancel_intent(&profile, &intent, mode == SubmitMode::Live);
    let envelope = agent_finance_core::create_cancel_intent(intent, 300)?;
    runtime.save_intent_with_audit(
        &profile,
        &envelope,
        &risk,
        format!("created TUI cancel intent {}", envelope.id),
    )?;
    Ok((profile, envelope.id))
}

fn create_staged_transfer_intent(
    runtime: &TradingRuntime,
    review: &TransferReview,
    mode: SubmitMode,
) -> Result<(Profile, String)> {
    let profile = runtime.load_profile(&review.profile)?;
    let intent = transfer_intent_from_review(&profile, review, generated_transfer_client_id());
    let risk =
        agent_finance_core::check_transfer_intent(&profile, &intent, mode == SubmitMode::Live);
    let envelope = agent_finance_core::create_transfer_intent(intent, 300)?;
    runtime.save_intent_with_audit(
        &profile,
        &envelope,
        &risk,
        format!("created TUI transfer intent {}", envelope.id),
    )?;
    Ok((profile, envelope.id))
}

fn create_staged_futures_state_intent(
    runtime: &TradingRuntime,
    review: &FuturesStateReview,
    mode: SubmitMode,
) -> Result<(Profile, String)> {
    let profile = runtime.load_profile(&review.profile)?;
    let intent = futures_state_intent_from_review(&profile, review);
    let risk =
        agent_finance_core::check_futures_state_intent(&profile, &intent, mode == SubmitMode::Live);
    let envelope = agent_finance_core::create_futures_state_intent(intent, 300)?;
    runtime.save_intent_with_audit(
        &profile,
        &envelope,
        &risk,
        format!("created TUI futures state intent {}", envelope.id),
    )?;
    Ok((profile, envelope.id))
}

async fn submit_staged_intent(
    runtime: &TradingRuntime,
    profile: &Profile,
    intent_id: &str,
    mode: SubmitMode,
    subject: &StagedSubmitSubject,
) -> std::result::Result<agent_finance_core::SubmitSnapshot, agent_finance_trading::SubmitFailure> {
    runtime
        .submit_typed_intent_classified(profile, intent_id, subject.intent_kind(), mode)
        .await
}
