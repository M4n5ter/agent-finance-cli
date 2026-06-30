pub(crate) const INTENT_REVIEW_SUMMARY_ROWS: u16 = 4;
pub(crate) const INTENT_REVIEW_ACTION_ROW: usize = INTENT_REVIEW_SUMMARY_ROWS as usize - 1;

const TABLE_HEADER_ROWS: usize = 1;
const READY_ACTIONS: &[(&str, IntentReviewAction)] = &[
    ("[execute]", IntentReviewAction::ExecuteSelected),
    ("[close]", IntentReviewAction::CloseSelected),
];
const CLOSE_ONLY_ACTIONS: &[(&str, IntentReviewAction)] =
    &[("[close]", IntentReviewAction::CloseSelected)];
const NO_ACTIONS: &[(&str, IntentReviewAction)] = &[];

pub(crate) type IntentReviewActionLine = crate::action_line_view::ActionLine<IntentReviewAction>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IntentReviewAction {
    ExecuteSelected,
    CloseSelected,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IntentReviewActionState {
    None,
    Ready,
    Waiting,
    CloseOnly,
}

pub(crate) fn action_state_for_status(
    status: Option<crate::state::StagedChangeQueueStatus>,
) -> IntentReviewActionState {
    match status {
        None => IntentReviewActionState::None,
        Some(crate::state::StagedChangeQueueStatus::Ready) => IntentReviewActionState::Ready,
        Some(crate::state::StagedChangeQueueStatus::Running) => IntentReviewActionState::Waiting,
        Some(
            crate::state::StagedChangeQueueStatus::Draft
            | crate::state::StagedChangeQueueStatus::Done
            | crate::state::StagedChangeQueueStatus::Failed
            | crate::state::StagedChangeQueueStatus::Closed,
        ) => IntentReviewActionState::CloseOnly,
    }
}

pub(crate) fn staged_change_index_at_content_row(
    visible_len: usize,
    content_row: usize,
) -> Option<usize> {
    let index = content_row.checked_sub(staged_change_content_row(0))?;
    (index < visible_len).then_some(index)
}

pub(crate) const fn staged_change_content_row(index: usize) -> usize {
    first_staged_change_content_row() + index
}

const fn first_staged_change_content_row() -> usize {
    INTENT_REVIEW_SUMMARY_ROWS as usize + TABLE_HEADER_ROWS
}

pub(crate) fn action_line(
    hidden: usize,
    width: u16,
    state: IntentReviewActionState,
) -> IntentReviewActionLine {
    let mut hint = hint_for_state(state);
    if hidden > 0 {
        hint.push_str(&format!("  +{hidden} hidden staged change(s)"));
    }
    crate::action_line_view::right_aligned_action_line(width, &hint, 2, actions_for_state(state))
}

fn hint_for_state(state: IntentReviewActionState) -> String {
    match state {
        IntentReviewActionState::None => "no staged change selected".to_string(),
        IntentReviewActionState::Ready => crate::hints::intent_review_panel_hint(),
        IntentReviewActionState::Waiting => "waiting for worker result".to_string(),
        IntentReviewActionState::CloseOnly => "review selected change or close".to_string(),
    }
}

fn actions_for_state(
    state: IntentReviewActionState,
) -> &'static [(&'static str, IntentReviewAction)] {
    match state {
        IntentReviewActionState::Ready => READY_ACTIONS,
        IntentReviewActionState::CloseOnly => CLOSE_ONLY_ACTIONS,
        IntentReviewActionState::None | IntentReviewActionState::Waiting => NO_ACTIONS,
    }
}

pub(crate) fn action_at_content_cell(
    hidden: usize,
    width: u16,
    state: IntentReviewActionState,
    content_row: usize,
    content_column: u16,
) -> Option<IntentReviewAction> {
    if content_row != INTENT_REVIEW_ACTION_ROW {
        return None;
    }
    action_line(hidden, width, state)
        .action_at(content_column)
        .map(|span| span.action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_row_maps_to_staged_change_index_below_summary_and_header() {
        assert_eq!(staged_change_index_at_content_row(2, 4), None);
        assert_eq!(staged_change_index_at_content_row(2, 5), Some(0));
        assert_eq!(staged_change_index_at_content_row(2, 6), Some(1));
        assert_eq!(staged_change_index_at_content_row(2, 7), None);
    }

    #[test]
    fn action_line_maps_visible_buttons_to_actions() {
        let line = action_line(3, 120, IntentReviewActionState::Ready);
        let execute = line
            .actions
            .iter()
            .find(|span| span.action == IntentReviewAction::ExecuteSelected)
            .expect("execute action");
        let close = line
            .actions
            .iter()
            .find(|span| span.action == IntentReviewAction::CloseSelected)
            .expect("close action");

        assert_eq!(line.action_text(execute), "[execute]");
        assert_eq!(
            action_at_content_cell(
                3,
                120,
                IntentReviewActionState::Ready,
                INTENT_REVIEW_ACTION_ROW,
                execute.start
            ),
            Some(IntentReviewAction::ExecuteSelected)
        );
        assert_eq!(line.action_text(close), "[close]");
        assert_eq!(
            action_at_content_cell(
                3,
                120,
                IntentReviewActionState::Ready,
                INTENT_REVIEW_ACTION_ROW,
                close.end - 1
            ),
            Some(IntentReviewAction::CloseSelected)
        );
        assert_eq!(
            action_at_content_cell(3, 120, IntentReviewActionState::Ready, 0, execute.start),
            None
        );
    }

    #[test]
    fn narrow_action_line_keeps_actions_when_the_hint_must_shrink() {
        let line = action_line(0, 40, IntentReviewActionState::Ready);

        assert!(unicode_width::UnicodeWidthStr::width(line.text.as_str()) <= 40);
        assert_eq!(line.actions.len(), 2);
        assert!(line.text.contains("[execute]"));
        assert!(line.text.contains("[close]"));
    }

    #[test]
    fn action_line_matches_selected_change_lifecycle() {
        let ready = action_line(0, 80, IntentReviewActionState::Ready);
        assert!(ready.text.contains("[execute]"));
        assert!(ready.text.contains("[close]"));

        let waiting = action_line(0, 80, IntentReviewActionState::Waiting);
        assert!(waiting.text.contains("waiting for worker result"));
        assert!(waiting.actions.is_empty());

        let close_only = action_line(0, 80, IntentReviewActionState::CloseOnly);
        assert!(!close_only.text.contains("[execute]"));
        assert!(close_only.text.contains("[close]"));

        let none = action_line(0, 80, IntentReviewActionState::None);
        assert!(none.actions.is_empty());
    }
}
