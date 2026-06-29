pub(crate) const INTENT_REVIEW_SUMMARY_ROWS: u16 = 2;

const TABLE_HEADER_ROWS: usize = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct IntentReviewActionLine {
    pub text: String,
    pub actions: Vec<IntentReviewActionSpan>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct IntentReviewActionSpan {
    pub start: u16,
    pub end: u16,
    pub action: IntentReviewAction,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IntentReviewAction {
    ExecuteSelected,
    CloseSelected,
}

pub(crate) fn staged_change_index_at_content_row(
    visible_len: usize,
    content_row: usize,
) -> Option<usize> {
    let first_change_row = INTENT_REVIEW_SUMMARY_ROWS as usize + TABLE_HEADER_ROWS;
    let index = content_row.checked_sub(first_change_row)?;
    (index < visible_len).then_some(index)
}

pub(crate) fn action_line(hidden: usize, width: u16) -> IntentReviewActionLine {
    let mut line = IntentReviewActionLine {
        text: crate::hints::intent_review_panel_hint(),
        actions: Vec::new(),
    };
    line.truncate_to_width(width);
    line.push_visible_text(width, "  ");
    line.push_visible_action(width, "[execute]", IntentReviewAction::ExecuteSelected);
    line.push_visible_text(width, "  ");
    line.push_visible_action(width, "[close]", IntentReviewAction::CloseSelected);
    if hidden > 0 {
        line.push_visible_text(width, &format!("  +{hidden} hidden staged change(s)"));
    }
    line
}

pub(crate) fn action_at_content_cell(
    hidden: usize,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<IntentReviewAction> {
    if content_row != 1 {
        return None;
    }
    action_line(hidden, width)
        .actions
        .into_iter()
        .find(|span| (span.start..span.end).contains(&content_column))
        .map(|span| span.action)
}

impl IntentReviewActionLine {
    fn push_visible_text(&mut self, width: u16, text: &str) {
        let remaining = width as usize - self.text.len().min(width as usize);
        self.text.push_str(&text[..text.len().min(remaining)]);
    }

    fn push_visible_action(&mut self, width: u16, text: &'static str, action: IntentReviewAction) {
        if self.text.len() + text.len() > width as usize {
            return;
        }
        let start = self.text.len() as u16;
        self.text.push_str(text);
        self.actions.push(IntentReviewActionSpan {
            start,
            end: self.text.len() as u16,
            action,
        });
    }

    fn truncate_to_width(&mut self, width: u16) {
        self.text.truncate(self.text.len().min(width as usize));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_row_maps_to_staged_change_index_below_summary_and_header() {
        assert_eq!(staged_change_index_at_content_row(2, 2), None);
        assert_eq!(staged_change_index_at_content_row(2, 3), Some(0));
        assert_eq!(staged_change_index_at_content_row(2, 4), Some(1));
        assert_eq!(staged_change_index_at_content_row(2, 5), None);
    }

    #[test]
    fn action_line_maps_visible_buttons_to_actions() {
        let line = action_line(3, 120);
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

        assert_eq!(
            &line.text[execute.start as usize..execute.end as usize],
            "[execute]"
        );
        assert_eq!(
            action_at_content_cell(3, 120, 1, execute.start),
            Some(IntentReviewAction::ExecuteSelected)
        );
        assert_eq!(
            &line.text[close.start as usize..close.end as usize],
            "[close]"
        );
        assert_eq!(
            action_at_content_cell(3, 120, 1, close.end - 1),
            Some(IntentReviewAction::CloseSelected)
        );
        assert_eq!(action_at_content_cell(3, 120, 0, execute.start), None);
    }

    #[test]
    fn narrow_action_line_does_not_expose_hidden_actions() {
        let line = action_line(0, 40);

        assert!(line.text.len() <= 40);
        assert!(line.actions.is_empty());
        assert_eq!(action_at_content_cell(0, 40, 1, 39), None);
    }
}
