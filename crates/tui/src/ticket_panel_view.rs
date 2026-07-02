use crate::action_line_view::{ActionLine, ActionSpan, right_aligned_action_line};
use crate::command::ActionId;
use crate::i18n::TuiText;
use crate::order_ticket::OrderTicketPreview;
use crate::panel_action_line_view::{PanelActionLine, PanelActionSpan};
use crate::state::AppState;

const FIELD_PREV_LABEL: &str = "[prev]";
const FIELD_NEXT_LABEL: &str = "[next]";
const FIELD_ACTION_GAP: u16 = 1;

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

pub(crate) type TicketFieldActionLine = ActionLine<TicketFieldAction>;
pub(crate) type TicketFieldActionSpan = ActionSpan<TicketFieldAction>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TicketFieldAction {
    pub index: usize,
    pub direction: isize,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct TicketPanelRows {
    pub detail_count: usize,
    pub actions: &'static [TicketPanelAction],
    pub field_adjustable: Vec<bool>,
    pub ready: bool,
    pub blocker_count: usize,
}

impl TicketPanelRows {
    pub(crate) fn rows(&self) -> Vec<TicketPanelRow> {
        let mut rows = vec![TicketPanelRow::Header];
        rows.extend((0..self.detail_count).map(TicketPanelRow::Detail));
        rows.extend((0..self.field_count()).map(TicketPanelRow::Field));
        if self.ready {
            rows.push(TicketPanelRow::ReadyAction);
        } else {
            rows.extend((0..self.blocker_count.min(3)).map(TicketPanelRow::Blocker));
        }
        rows.extend((0..self.actions.len()).map(TicketPanelRow::Action));
        rows.push(TicketPanelRow::Hint);
        rows
    }

    pub(crate) fn click_at(&self, content_row: usize) -> Option<TicketPanelClick> {
        match self.rows().get(content_row)? {
            TicketPanelRow::Field(index) => Some(TicketPanelClick::Field(*index)),
            TicketPanelRow::ReadyAction => Some(TicketPanelClick::ReadyAction),
            _ => None,
        }
    }

    pub(crate) fn action_at_content_cell(
        &self,
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

    pub(crate) fn field_action_at_content_cell(
        &self,
        width: u16,
        content_row: usize,
        content_column: u16,
    ) -> Option<TicketFieldActionSpan> {
        let rows = self.rows();
        let TicketPanelRow::Field(index) = rows.get(content_row)? else {
            return None;
        };
        field_action_line(width, *index, "", self.field_is_adjustable(*index))
            .action_at(content_column)
    }

    pub(crate) fn field_count(&self) -> usize {
        self.field_adjustable.len()
    }

    pub(crate) fn field_is_adjustable(&self, index: usize) -> bool {
        self.field_adjustable.get(index).copied().unwrap_or(false)
    }
}

pub(crate) fn order_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.order_ticket_preview();
    TicketPanelRows {
        detail_count: order_ticket_detail_lines(&preview, state.locale).len(),
        actions: crate::order_ticket_controls::ORDER_TICKET_ACTIONS,
        field_adjustable: vec![true; crate::order_ticket::OrderTicketField::COUNT],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

pub(crate) fn order_ticket_detail_lines(
    preview: &OrderTicketPreview,
    locale: agent_finance_i18n::LocaleId,
) -> Vec<String> {
    let text = TuiText::new(locale);
    let mut lines = vec![text.f(
        "tui-ticket-detail-symbol-profile",
        &[
            ("symbol", preview.symbol.as_deref().unwrap_or("-")),
            ("profile", preview.profile.as_deref().unwrap_or("-")),
        ],
    )];
    if !preview.protective_draft.is_empty() {
        lines.push(
            text.f(
                "tui-ticket-detail-protective-draft",
                &[
                    (
                        "stopLoss",
                        preview.protective_draft.stop_loss.as_deref().unwrap_or("-"),
                    ),
                    (
                        "takeProfit",
                        preview
                            .protective_draft
                            .take_profit
                            .as_deref()
                            .unwrap_or("-"),
                    ),
                ],
            ),
        );
    }
    lines
}

pub(crate) fn transfer_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.transfer_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        actions: crate::transfer_ticket_controls::TRANSFER_TICKET_ACTIONS,
        field_adjustable: vec![true; crate::transfer_ticket::TransferTicketField::COUNT],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

pub(crate) fn futures_state_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.futures_state_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        actions: crate::futures_state_controls::FUTURES_STATE_ACTIONS,
        field_adjustable: vec![true, preview.scope_adjustable(), true],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

pub(crate) fn field_action_line(
    width: u16,
    index: usize,
    text: &str,
    adjustable: bool,
) -> TicketFieldActionLine {
    if !adjustable {
        let mut line = TicketFieldActionLine::new("", width);
        line.push_visible_text(text);
        return line;
    }
    right_aligned_action_line(
        width,
        text,
        FIELD_ACTION_GAP,
        &[
            (
                FIELD_PREV_LABEL,
                TicketFieldAction {
                    index,
                    direction: -1,
                },
            ),
            (
                FIELD_NEXT_LABEL,
                TicketFieldAction {
                    index,
                    direction: 1,
                },
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_mark_fields_and_ready_action_as_clickable() {
        let rows = TicketPanelRows {
            detail_count: 1,
            actions: &[],
            field_adjustable: vec![true, true],
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
            field_adjustable: vec![true],
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
            field_adjustable: vec![true],
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
            Some((
                "[capture price]".to_string(),
                ActionId::CaptureOrderReferencePrice,
            ))
        );
        assert_eq!(rows.action_at_content_cell(80, 4, 15), None);
        assert_eq!(rows.action_at_content_cell(3, 4, 0), None);
    }

    #[test]
    fn order_ticket_rows_include_protective_draft_detail_line() {
        let mut state = crate::state::AppState::from_config(crate::config::TuiConfig::default());
        state
            .order_ticket
            .capture_protective_reference(90.0, crate::order_ticket::ProtectiveDraftSlot::StopLoss);

        let preview = state.order_ticket_preview();
        let detail_lines = order_ticket_detail_lines(&preview, state.locale);
        let rows = order_ticket_rows(&state);

        assert_eq!(detail_lines.len(), 2);
        assert_eq!(rows.detail_count, detail_lines.len());
        assert!(detail_lines[1].contains("protective draft: stop-loss=90.0000"));
        assert!(rows.rows().contains(&TicketPanelRow::Detail(1)));
    }

    #[test]
    fn field_actions_are_right_aligned_and_hidden_when_narrow() {
        let rows = TicketPanelRows {
            detail_count: 0,
            actions: &[],
            field_adjustable: vec![true],
            ready: false,
            blocker_count: 0,
        };

        let prev = rows
            .field_action_at_content_cell(30, 1, 18)
            .expect("prev action is visible");
        let next = rows
            .field_action_at_content_cell(30, 1, 25)
            .expect("next action is visible");

        assert_eq!(
            prev.action,
            TicketFieldAction {
                index: 0,
                direction: -1
            }
        );
        assert_eq!(
            next.action,
            TicketFieldAction {
                index: 0,
                direction: 1
            }
        );
        assert_eq!(rows.field_action_at_content_cell(12, 1, 0), None);
    }

    #[test]
    fn inactive_fields_do_not_expose_adjust_actions() {
        let rows = TicketPanelRows {
            detail_count: 0,
            actions: &[],
            field_adjustable: vec![true, false],
            ready: false,
            blocker_count: 0,
        };

        assert!(rows.field_action_at_content_cell(30, 2, 18).is_none());
    }
}
