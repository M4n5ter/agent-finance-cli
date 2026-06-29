use crate::command::ActionId;
use crate::panel_action_line_view::{PanelActionLine, PanelActionSpan};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TicketPanelRow {
    Header,
    Detail(usize),
    Action(usize),
    Field(usize),
    ReadyAction,
    Blocker(usize),
    Hint,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TicketPanelClick {
    Field(usize),
    ReadyAction,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TicketPanelAction {
    pub label: &'static str,
    pub action: ActionId,
}

impl TicketPanelAction {
    pub(crate) fn line(self, width: u16) -> PanelActionLine {
        let mut line = PanelActionLine::new("", width);
        line.push_visible_action(self.label, self.action);
        line
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TicketPanelRows {
    pub detail_count: usize,
    pub actions: &'static [TicketPanelAction],
    pub field_count: usize,
    pub ready: bool,
    pub blocker_count: usize,
}

impl TicketPanelRows {
    pub(crate) fn rows(self) -> Vec<TicketPanelRow> {
        let mut rows = vec![TicketPanelRow::Header];
        rows.extend((0..self.detail_count).map(TicketPanelRow::Detail));
        rows.extend((0..self.field_count).map(TicketPanelRow::Field));
        if self.ready {
            rows.push(TicketPanelRow::ReadyAction);
        } else {
            rows.extend((0..self.blocker_count.min(3)).map(TicketPanelRow::Blocker));
        }
        rows.extend((0..self.actions.len()).map(TicketPanelRow::Action));
        rows.push(TicketPanelRow::Hint);
        rows
    }

    pub(crate) fn click_at(self, content_row: usize) -> Option<TicketPanelClick> {
        match self.rows().get(content_row)? {
            TicketPanelRow::Field(index) => Some(TicketPanelClick::Field(*index)),
            TicketPanelRow::ReadyAction => Some(TicketPanelClick::ReadyAction),
            _ => None,
        }
    }

    pub(crate) fn action_at_content_cell(
        self,
        width: u16,
        content_row: usize,
        content_column: u16,
    ) -> Option<PanelActionSpan> {
        let rows = self.rows();
        let TicketPanelRow::Action(index) = rows.get(content_row)? else {
            return None;
        };
        self.actions
            .get(*index)?
            .line(width)
            .action_at(content_column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_mark_fields_and_ready_action_as_clickable() {
        let rows = TicketPanelRows {
            detail_count: 1,
            actions: &[],
            field_count: 2,
            ready: true,
            blocker_count: 0,
        };

        assert_eq!(
            rows.rows(),
            vec![
                TicketPanelRow::Header,
                TicketPanelRow::Detail(0),
                TicketPanelRow::Field(0),
                TicketPanelRow::Field(1),
                TicketPanelRow::ReadyAction,
                TicketPanelRow::Hint,
            ]
        );
        assert_eq!(rows.click_at(2), Some(TicketPanelClick::Field(0)));
        assert_eq!(rows.click_at(4), Some(TicketPanelClick::ReadyAction));
        assert_eq!(rows.click_at(5), None);
    }

    #[test]
    fn blocked_rows_do_not_expose_stage_action() {
        let rows = TicketPanelRows {
            detail_count: 0,
            actions: &[],
            field_count: 1,
            ready: false,
            blocker_count: 4,
        };

        assert_eq!(
            rows.rows(),
            vec![
                TicketPanelRow::Header,
                TicketPanelRow::Field(0),
                TicketPanelRow::Blocker(0),
                TicketPanelRow::Blocker(1),
                TicketPanelRow::Blocker(2),
                TicketPanelRow::Hint,
            ]
        );
        assert_eq!(rows.click_at(1), Some(TicketPanelClick::Field(0)));
        assert_eq!(rows.click_at(2), None);
    }

    #[test]
    fn rows_place_ticket_actions_after_state_rows() {
        const ACTIONS: &[TicketPanelAction] = &[TicketPanelAction {
            label: "[capture price]",
            action: ActionId::CaptureOrderReferencePrice,
        }];
        let rows = TicketPanelRows {
            detail_count: 1,
            actions: ACTIONS,
            field_count: 1,
            ready: false,
            blocker_count: 1,
        };

        assert_eq!(
            rows.rows(),
            vec![
                TicketPanelRow::Header,
                TicketPanelRow::Detail(0),
                TicketPanelRow::Field(0),
                TicketPanelRow::Blocker(0),
                TicketPanelRow::Action(0),
                TicketPanelRow::Hint,
            ]
        );
        assert_eq!(rows.click_at(2), Some(TicketPanelClick::Field(0)));
        assert_eq!(rows.click_at(4), None);
        assert_eq!(
            rows.action_at_content_cell(80, 4, 0)
                .map(|span| (span.label, span.action)),
            Some(("[capture price]", ActionId::CaptureOrderReferencePrice))
        );
        assert_eq!(rows.action_at_content_cell(80, 4, 15), None);
        assert_eq!(rows.action_at_content_cell(3, 4, 0), None);
    }
}
