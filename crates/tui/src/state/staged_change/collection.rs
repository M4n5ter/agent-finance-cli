use agent_finance_core::submit::SubmitMode;

use super::subject::{StagedChangeRequest, StagedSubmitRequest};
use super::workflow::{
    StagedChange, StagedChangeEvent, StagedChangeStage, StagedChangeState, StagedChangeView,
};

pub(crate) const VISIBLE_REVIEW_LIMIT: usize = 8;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StagedChanges {
    changes: Vec<StagedChange>,
    selected: usize,
}

impl StagedChanges {
    pub(crate) fn views(&self) -> Vec<StagedChangeView> {
        let selected = self.normalized_selected();
        self.changes
            .iter()
            .enumerate()
            .map(|(index, change)| StagedChangeView::from_selected(change, index == selected))
            .collect()
    }

    pub(crate) fn review_views(&self) -> Vec<StagedChangeView> {
        self.views()
            .into_iter()
            .take(VISIBLE_REVIEW_LIMIT)
            .collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.changes.len()
    }

    pub(crate) fn selected_submit_request(&mut self) -> QueueSubmitResult {
        self.normalize_selection();
        let Some(change) = self.changes.get(self.selected) else {
            return QueueSubmitResult::Missing;
        };
        submit_request_for(change)
    }

    pub(crate) fn queue_submit_request(
        &mut self,
        expected: &StagedSubmitRequest,
    ) -> QueueSubmitResult {
        let Some(change) = self
            .changes
            .iter_mut()
            .find(|change| change.id == expected.id)
        else {
            return QueueSubmitResult::Missing;
        };
        let QueueSubmitResult::Queued(request) = submit_request_for(change) else {
            return QueueSubmitResult::Rejected {
                current: format!("{:?}", change.state),
            };
        };
        if &request != expected {
            return QueueSubmitResult::Rejected {
                current: format!("{:?}", change.state),
            };
        }
        if change.apply(StagedChangeEvent::SubmitQueued) {
            QueueSubmitResult::Queued(request)
        } else {
            QueueSubmitResult::Rejected {
                current: format!("{:?}", change.state),
            }
        }
    }

    pub(crate) fn open(
        &mut self,
        request: StagedChangeRequest,
        mode: SubmitMode,
    ) -> OpenStagedChangeResult {
        self.open_with(request, mode, StagedChangeState::Draft)
    }

    pub(crate) fn open_ready(
        &mut self,
        request: StagedChangeRequest,
        mode: SubmitMode,
    ) -> OpenStagedChangeResult {
        self.open_with(request, mode, StagedChangeState::Ready)
    }

    fn open_with(
        &mut self,
        request: StagedChangeRequest,
        mode: SubmitMode,
        state: StagedChangeState,
    ) -> OpenStagedChangeResult {
        if self
            .changes
            .iter()
            .any(|change| change.id == request.id && !change.state.accepts_replacement())
        {
            return OpenStagedChangeResult::Rejected;
        }

        self.changes
            .retain(|change| change.id != request.id || !change.state.accepts_replacement());
        self.changes.push(StagedChange {
            id: request.id,
            default_mode: mode,
            state,
            subject: request.subject,
        });
        self.selected = self.visible_len().saturating_sub(1);
        OpenStagedChangeResult::Opened
    }

    pub(crate) fn apply(&mut self, id: &str, event: StagedChangeEvent) -> TransitionResult {
        let Some(change) = self.changes.iter_mut().find(|change| change.id == id) else {
            return TransitionResult::Missing;
        };
        let previous = format!("{:?}", change.state);
        if change.apply(event.clone()) {
            TransitionResult::Applied
        } else {
            TransitionResult::Rejected {
                current: previous,
                event,
            }
        }
    }

    pub(crate) fn disable_live(&mut self) -> usize {
        let mut disabled = 0;
        for change in &mut self.changes {
            if change.disable_live() {
                disabled += 1;
            }
        }
        self.normalize_selection();
        disabled
    }

    pub(crate) fn close(&mut self, id: &str) -> CloseStagedChangeResult {
        let Some(index) = self.changes.iter().position(|change| change.id == id) else {
            return CloseStagedChangeResult::Missing;
        };
        let current = &self.changes[index].state;
        if current.blocks_close() {
            return CloseStagedChangeResult::Rejected {
                current: format!("{:?}", current),
            };
        }

        self.changes.remove(index);
        self.selected = self
            .selected
            .saturating_sub(usize::from(index <= self.selected));
        self.normalize_selection();
        CloseStagedChangeResult::Closed
    }

    pub(crate) fn move_selection(&mut self, direction: isize) {
        if self.changes.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = shift_index(self.normalized_selected(), self.visible_len(), direction);
    }

    pub(crate) fn close_selected(&mut self) -> CloseStagedChangeResult {
        self.normalize_selection();
        let Some(id) = self
            .changes
            .get(self.selected)
            .map(|change| change.id.clone())
        else {
            return CloseStagedChangeResult::Missing;
        };
        self.close(&id)
    }

    fn normalize_selection(&mut self) {
        if self.changes.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.normalized_selected();
        }
    }

    fn visible_len(&self) -> usize {
        self.changes.len().min(VISIBLE_REVIEW_LIMIT)
    }

    fn normalized_selected(&self) -> usize {
        self.selected.min(self.visible_len().saturating_sub(1))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OpenStagedChangeResult {
    Opened,
    Rejected,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CloseStagedChangeResult {
    Closed,
    Missing,
    Rejected { current: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QueueSubmitResult {
    Queued(StagedSubmitRequest),
    Missing,
    Rejected { current: String },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum TransitionResult {
    Applied,
    Missing,
    Rejected {
        current: String,
        event: StagedChangeEvent,
    },
}

fn shift_index(index: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    (index as isize + direction).rem_euclid(len) as usize
}

fn submit_request_for(change: &StagedChange) -> QueueSubmitResult {
    if change.state.stage() != StagedChangeStage::Ready {
        return QueueSubmitResult::Rejected {
            current: format!("{:?}", change.state),
        };
    }
    change
        .subject
        .submit_request(change.id.clone(), change.state.mode(change.default_mode))
        .map_or(QueueSubmitResult::Missing, QueueSubmitResult::Queued)
}
