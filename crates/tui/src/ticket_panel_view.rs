#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TicketPanelRow {
    Header,
    Detail(usize),
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
pub(crate) struct TicketPanelRows {
    pub detail_count: usize,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_mark_fields_and_ready_action_as_clickable() {
        let rows = TicketPanelRows {
            detail_count: 1,
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
}
