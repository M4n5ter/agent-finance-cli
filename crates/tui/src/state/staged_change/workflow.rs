use agent_finance_core::{
    intent::IntentStatus,
    submit::{SubmitIntentKind, SubmitMode},
};
use serde::Serialize;
use std::fmt;

#[cfg(test)]
use super::subject::StagedChangeRequest;
use super::subject::StagedChangeSubject;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StagedChange {
    pub(crate) id: String,
    pub(crate) default_mode: SubmitMode,
    pub(crate) state: StagedChangeState,
    pub(crate) subject: StagedChangeSubject,
}

impl StagedChange {
    #[cfg(test)]
    pub(crate) fn from_request(request: StagedChangeRequest, default_mode: SubmitMode) -> Self {
        Self {
            id: request.id,
            default_mode,
            state: StagedChangeState::Draft,
            subject: request.subject,
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> &StagedChangeState {
        &self.state
    }

    pub fn apply(&mut self, event: StagedChangeEvent) -> bool {
        if matches!(event, StagedChangeEvent::LiveIntentClaimed { .. })
            && self.default_mode != SubmitMode::Live
        {
            return false;
        }
        let Some(next) = self.state.next(event) else {
            return false;
        };
        self.state = next;
        true
    }

    pub(crate) fn disable_live(&mut self) -> bool {
        if self.default_mode != SubmitMode::Live || !self.state.can_disable_live() {
            return false;
        }
        self.default_mode = SubmitMode::DryRun;
        self.state = StagedChangeState::Abandoned;
        true
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged change events are consumed when staged change panels bind the change workflow"
    )
)]
pub enum StagedChangeEvent {
    ValidationStarted,
    ValidationReady,
    SubmitQueued,
    IntentCreated {
        intent_id: String,
    },
    NonConsumingFinished {
        intent_id: String,
        mode: SubmitMode,
    },
    PreflightFailed {
        intent_id: String,
        attempted_mode: SubmitMode,
    },
    LiveIntentClaimed {
        intent_id: String,
    },
    LiveSubmitSucceeded {
        intent_id: String,
    },
    LiveSubmitFailed {
        intent_id: String,
    },
    FailedBeforeIntent,
    Abandoned,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum StagedChangeState {
    Draft,
    Validating,
    Ready,
    SubmitQueued,
    IntentCreated {
        intent_id: String,
    },
    NonConsumingCompleted {
        intent_id: String,
        mode: NonConsumingMode,
    },
    PreflightFailed {
        intent_id: String,
        attempted_mode: SubmitMode,
    },
    LiveIntentClaimed {
        intent_id: String,
    },
    LiveSubmitted {
        intent_id: String,
    },
    FailedBeforeIntent,
    IntentFailed {
        intent_id: String,
    },
    Abandoned,
}

impl StagedChangeState {
    pub(crate) fn next(&self, event: StagedChangeEvent) -> Option<Self> {
        match (self, event) {
            (Self::Draft, StagedChangeEvent::ValidationStarted) => Some(Self::Validating),
            (Self::Draft, StagedChangeEvent::Abandoned) => Some(Self::Abandoned),
            (Self::Validating, StagedChangeEvent::ValidationReady) => Some(Self::Ready),
            (Self::Validating, StagedChangeEvent::FailedBeforeIntent) => {
                Some(Self::FailedBeforeIntent)
            }
            (Self::Validating, StagedChangeEvent::Abandoned) => Some(Self::Abandoned),
            (Self::Ready, StagedChangeEvent::SubmitQueued) => Some(Self::SubmitQueued),
            (Self::Ready, StagedChangeEvent::FailedBeforeIntent) => Some(Self::FailedBeforeIntent),
            (Self::Ready, StagedChangeEvent::Abandoned) => Some(Self::Abandoned),
            (Self::SubmitQueued, StagedChangeEvent::FailedBeforeIntent) => {
                Some(Self::FailedBeforeIntent)
            }
            (Self::SubmitQueued, StagedChangeEvent::Abandoned) => Some(Self::Abandoned),
            (Self::SubmitQueued, StagedChangeEvent::IntentCreated { intent_id }) => {
                Some(Self::IntentCreated { intent_id })
            }
            (
                Self::IntentCreated { intent_id },
                StagedChangeEvent::NonConsumingFinished {
                    intent_id: next_id,
                    mode,
                },
            ) if intent_id == &next_id => {
                let mode = NonConsumingMode::from_submit_mode(mode)?;
                Some(Self::NonConsumingCompleted {
                    intent_id: next_id,
                    mode,
                })
            }
            (
                Self::IntentCreated { intent_id },
                StagedChangeEvent::PreflightFailed {
                    intent_id: next_id,
                    attempted_mode,
                },
            ) if intent_id == &next_id => Some(Self::PreflightFailed {
                intent_id: next_id,
                attempted_mode,
            }),
            (
                Self::IntentCreated { intent_id }
                | Self::NonConsumingCompleted { intent_id, .. }
                | Self::PreflightFailed { intent_id, .. },
                StagedChangeEvent::LiveIntentClaimed { intent_id: next_id },
            ) if intent_id == &next_id => Some(Self::LiveIntentClaimed { intent_id: next_id }),
            (
                Self::NonConsumingCompleted { .. } | Self::PreflightFailed { .. },
                StagedChangeEvent::Abandoned,
            ) => Some(Self::Abandoned),
            (
                Self::LiveIntentClaimed { intent_id },
                StagedChangeEvent::LiveSubmitSucceeded { intent_id: next_id },
            ) if intent_id == &next_id => Some(Self::LiveSubmitted { intent_id: next_id }),
            (
                Self::LiveIntentClaimed { intent_id },
                StagedChangeEvent::LiveSubmitFailed { intent_id: next_id },
            ) if intent_id == &next_id => Some(Self::IntentFailed { intent_id: next_id }),
            _ => None,
        }
    }

    pub(crate) fn stage(&self) -> StagedChangeStage {
        match self {
            Self::Draft => StagedChangeStage::Draft,
            Self::Validating => StagedChangeStage::Validating,
            Self::Ready => StagedChangeStage::Ready,
            Self::SubmitQueued => StagedChangeStage::SubmitQueued,
            Self::IntentCreated { .. } => StagedChangeStage::IntentCreated,
            Self::NonConsumingCompleted {
                mode: NonConsumingMode::DryRun,
                ..
            } => StagedChangeStage::DryRunCompleted,
            Self::NonConsumingCompleted {
                mode: NonConsumingMode::Test,
                ..
            } => StagedChangeStage::TestCompleted,
            Self::PreflightFailed {
                attempted_mode: SubmitMode::DryRun,
                ..
            } => StagedChangeStage::DryRunFailed,
            Self::PreflightFailed {
                attempted_mode: SubmitMode::Test,
                ..
            } => StagedChangeStage::TestFailed,
            Self::PreflightFailed {
                attempted_mode: SubmitMode::Live,
                ..
            } => StagedChangeStage::LivePreflightFailed,
            Self::LiveIntentClaimed { .. } => StagedChangeStage::LiveIntentClaimed,
            Self::LiveSubmitted { .. } => StagedChangeStage::LiveSubmitted,
            Self::FailedBeforeIntent => StagedChangeStage::FailedBeforeIntent,
            Self::IntentFailed { .. } => StagedChangeStage::IntentFailed,
            Self::Abandoned => StagedChangeStage::Abandoned,
        }
    }

    pub(crate) fn mode(&self, default_mode: SubmitMode) -> SubmitMode {
        match self {
            Self::NonConsumingCompleted { mode, .. } => mode.submit_mode(),
            Self::PreflightFailed { attempted_mode, .. } => *attempted_mode,
            Self::LiveIntentClaimed { .. }
            | Self::LiveSubmitted { .. }
            | Self::IntentFailed { .. } => SubmitMode::Live,
            _ => default_mode,
        }
    }

    pub(crate) fn intent_status(&self) -> Option<IntentStatus> {
        match self {
            Self::LiveSubmitted { .. } => Some(IntentStatus::Submitted),
            Self::IntentFailed { .. } => Some(IntentStatus::Failed),
            _ => None,
        }
    }

    pub(crate) fn intent_id(&self) -> Option<&str> {
        match self {
            Self::IntentCreated { intent_id }
            | Self::NonConsumingCompleted { intent_id, .. }
            | Self::PreflightFailed { intent_id, .. }
            | Self::LiveIntentClaimed { intent_id }
            | Self::LiveSubmitted { intent_id }
            | Self::IntentFailed { intent_id } => Some(intent_id),
            _ => None,
        }
    }

    pub(crate) fn accepts_replacement(&self) -> bool {
        matches!(
            self,
            Self::Draft | Self::FailedBeforeIntent | Self::IntentFailed { .. } | Self::Abandoned
        )
    }

    pub(crate) fn blocks_close(&self) -> bool {
        matches!(
            self,
            Self::SubmitQueued | Self::IntentCreated { .. } | Self::LiveIntentClaimed { .. }
        )
    }

    pub(crate) fn can_disable_live(&self) -> bool {
        !matches!(
            self,
            Self::LiveIntentClaimed { .. } | Self::LiveSubmitted { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum NonConsumingMode {
    DryRun,
    Test,
}

impl NonConsumingMode {
    fn from_submit_mode(mode: SubmitMode) -> Option<Self> {
        match mode {
            SubmitMode::DryRun => Some(Self::DryRun),
            SubmitMode::Test => Some(Self::Test),
            SubmitMode::Live => None,
        }
    }

    fn submit_mode(self) -> SubmitMode {
        match self {
            Self::DryRun => SubmitMode::DryRun,
            Self::Test => SubmitMode::Test,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StagedChangeStage {
    Draft,
    Validating,
    Ready,
    SubmitQueued,
    IntentCreated,
    DryRunCompleted,
    TestCompleted,
    DryRunFailed,
    TestFailed,
    LivePreflightFailed,
    LiveIntentClaimed,
    LiveSubmitted,
    FailedBeforeIntent,
    IntentFailed,
    Abandoned,
}

impl StagedChangeStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Validating => "validating",
            Self::Ready => "ready",
            Self::SubmitQueued => "submit-queued",
            Self::IntentCreated => "intent-created",
            Self::DryRunCompleted => "dry-run-completed",
            Self::TestCompleted => "test-completed",
            Self::DryRunFailed => "dry-run-failed",
            Self::TestFailed => "test-failed",
            Self::LivePreflightFailed => "live-preflight-failed",
            Self::LiveIntentClaimed => "live-intent-claimed",
            Self::LiveSubmitted => "live-submitted",
            Self::FailedBeforeIntent => "failed-before-intent",
            Self::IntentFailed => "intent-failed",
            Self::Abandoned => "abandoned",
        }
    }
}

impl fmt::Display for StagedChangeStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StagedChangeView {
    pub id: String,
    pub selected: bool,
    pub intent_kind: SubmitIntentKind,
    pub stage: StagedChangeStage,
    pub mode: SubmitMode,
    pub intent_id: Option<String>,
    pub intent_status: Option<IntentStatus>,
    pub summary: String,
    pub subject: StagedChangeSubject,
}

impl From<&StagedChange> for StagedChangeView {
    fn from(change: &StagedChange) -> Self {
        Self::from_selected(change, false)
    }
}

impl StagedChangeView {
    pub(crate) fn from_selected(change: &StagedChange, selected: bool) -> Self {
        Self {
            id: change.id.clone(),
            selected,
            intent_kind: change.subject.intent_kind(),
            stage: change.state.stage(),
            mode: change.state.mode(change.default_mode),
            intent_id: change.state.intent_id().map(ToString::to_string),
            intent_status: change.state.intent_status(),
            summary: change.subject.summary(),
            subject: change.subject.clone(),
        }
    }
}
