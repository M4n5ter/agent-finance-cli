mod collection;
mod subject;
mod workflow;

#[cfg(test)]
mod tests;

pub(crate) use collection::{
    CloseStagedChangeResult, OpenStagedChangeResult, QueueExecutionResult, StagedChanges,
    TransitionResult, VISIBLE_REVIEW_LIMIT,
};
pub use subject::{
    CancelReview, FuturesStateReview, OrderTicketReview, ProfileRiskReview, StagedChangeRequest,
    StagedChangeSubject, StagedExecution, StagedExecutionRequest, StagedLocalCommitSubject,
    StagedSubmitRequest, StagedSubmitSubject, TransferReview, TypedConfirmation,
};
#[cfg(test)]
pub use subject::{ProfileRiskChange, StagedChangeKind};
pub(crate) use workflow::StagedChangeQueueStatus;
#[cfg(test)]
pub use workflow::StagedChangeStage;
pub use workflow::{StagedChangeEvent, StagedChangeView};

#[cfg(test)]
pub(crate) use workflow::{StagedChange, StagedChangeState};
