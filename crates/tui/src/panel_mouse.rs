use ratatui::layout::Rect;

use crate::account_panel_view::{AccountPanelHit, AccountTicketPreset};
use crate::futures_state_ticket::FuturesStateTicketField;
use crate::intent_review_view::IntentReviewAction;
use crate::model::Panel;
use crate::mouse_target::{MouseTarget, PanelMouseAction};
use crate::order_ticket::OrderTicketField;
use crate::state::{Action, AppState};
use crate::ticket_panel_view::{TicketPanelClick, TicketPanelRows};
use crate::transfer_ticket::TransferTicketField;

pub(crate) fn click_action(
    state: &AppState,
    panel: Panel,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<Action> {
    panel_hit_at(state, panel, area, column, row).and_then(|hit| hit.action_for(panel))
}

pub(crate) fn hover_target(
    state: &AppState,
    panel: Panel,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<MouseTarget> {
    panel_hit_at(state, panel, area, column, row)
        .map(|hit| MouseTarget::PanelAction {
            panel,
            action: hit.mouse_action(),
        })
        .or(Some(MouseTarget::Panel(panel)))
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum PanelHit {
    Row(usize),
    InfoRow(usize),
    TicketField(usize),
    TicketFieldAdjust {
        index: usize,
        direction: isize,
    },
    TicketReadyAction,
    Action {
        label: &'static str,
        action: crate::command::ActionId,
    },
    AccountHit {
        content_row: usize,
        hit: AccountPanelHit,
    },
    SettingAdjust {
        index: usize,
        direction: isize,
    },
    IntentReviewAction(IntentReviewAction),
}

impl PanelHit {
    fn action_for(self, panel: Panel) -> Option<Action> {
        match (panel, self) {
            (Panel::Watchlist, Self::Row(index)) => Some(Action::SelectWatchlistSymbol(index)),
            (Panel::Account | Panel::OpenOrders, Self::Row(index)) => {
                Some(Action::SelectOpenOrder(index))
            }
            (Panel::Account, Self::AccountHit { hit, .. }) => match hit {
                AccountPanelHit::OpenOrder(index) => Some(Action::SelectOpenOrder(index)),
                AccountPanelHit::TicketPreset(AccountTicketPreset::Transfer(preset)) => {
                    Some(Action::ApplyTransferTicketPreset(preset))
                }
                AccountPanelHit::TicketPreset(AccountTicketPreset::FuturesState(preset)) => {
                    Some(Action::ApplyFuturesStateTicketPreset(preset))
                }
            },
            (_, Self::Action { action, .. }) => Some(Action::Execute(action)),
            (Panel::Settings, Self::SettingAdjust { index, direction }) => {
                Some(Action::AdjustSettingRow { index, direction })
            }
            (Panel::IntentReview, Self::Row(index)) => Some(Action::SelectStagedChange(index)),
            (Panel::IntentReview, Self::IntentReviewAction(action)) => match action {
                IntentReviewAction::ExecuteSelected => Some(Action::ExecuteStagedChange),
                IntentReviewAction::CloseSelected => Some(Action::CloseSelectedStagedChange),
            },
            (Panel::Settings, Self::Row(index)) => Some(Action::SelectSettingRow(index)),
            (Panel::OrderTicket, Self::TicketField(index)) => {
                Some(Action::SelectOrderTicketField(index))
            }
            (Panel::OrderTicket, Self::TicketFieldAdjust { index, direction }) => {
                Some(Action::AdjustOrderTicketFieldAt { index, direction })
            }
            (Panel::OrderTicket, Self::TicketReadyAction) => Some(Action::StageOrderTicket),
            (Panel::TransferTicket, Self::TicketField(index)) => {
                Some(Action::SelectTransferTicketField(index))
            }
            (Panel::TransferTicket, Self::TicketFieldAdjust { index, direction }) => {
                Some(Action::AdjustTransferTicketFieldAt { index, direction })
            }
            (Panel::TransferTicket, Self::TicketReadyAction) => Some(Action::StageTransferTicket),
            (Panel::FuturesState, Self::TicketField(index)) => {
                Some(Action::SelectFuturesStateTicketField(index))
            }
            (Panel::FuturesState, Self::TicketFieldAdjust { index, direction }) => {
                Some(Action::AdjustFuturesStateTicketFieldAt { index, direction })
            }
            (Panel::FuturesState, Self::TicketReadyAction) => Some(Action::StageFuturesStateTicket),
            _ => None,
        }
    }

    fn mouse_action(self) -> PanelMouseAction {
        match self {
            Self::Row(index) => PanelMouseAction::SelectRow { index },
            Self::InfoRow(index) => PanelMouseAction::InspectRow { index },
            Self::TicketField(index) => PanelMouseAction::SelectField { index },
            Self::TicketFieldAdjust { index, direction } => {
                PanelMouseAction::AdjustField { index, direction }
            }
            Self::TicketReadyAction => PanelMouseAction::StageReadyChange,
            Self::Action { label, action } => PanelMouseAction::ExecuteAction { label, action },
            Self::AccountHit {
                hit: AccountPanelHit::OpenOrder(index),
                ..
            } => PanelMouseAction::SelectRow { index },
            Self::AccountHit {
                content_row,
                hit: AccountPanelHit::TicketPreset(_),
            } => PanelMouseAction::RowAction { content_row },
            Self::SettingAdjust { index, direction } => {
                PanelMouseAction::SettingAdjust { index, direction }
            }
            Self::IntentReviewAction(action) => PanelMouseAction::IntentReviewAction { action },
        }
    }
}

fn panel_hit_at(
    state: &AppState,
    panel: Panel,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<PanelHit> {
    match panel {
        Panel::Watchlist => {
            let index = content_row(area, row)?;
            (index < state.watchlist.len()).then_some(PanelHit::Row(index))
        }
        Panel::OpenOrders => {
            let open_orders = state.account_snapshot.as_ref()?.open_orders();
            let content_row = content_row(area, row)?;
            let content_column = content_column(area, column).unwrap_or(u16::MAX);
            if let Some(action) = crate::open_order_view::open_order_action_at_content_cell(
                &open_orders,
                state.selected_open_order,
                content_width(area),
                content_row,
                content_column,
            ) {
                return Some(PanelHit::Action {
                    label: action.label,
                    action: action.action,
                });
            }
            crate::open_order_view::open_order_index_at_content_row(
                &open_orders,
                state.selected_open_order,
                content_row,
            )
            .map(PanelHit::Row)
        }
        Panel::Account => {
            let content_row = content_row(area, row)?;
            let content_width = content_width(area);
            let content_column = content_column(area, column).unwrap_or(u16::MAX);
            if let Some(action) = crate::account_panel_view::action_at_content_cell(
                state,
                content_width,
                content_row,
                content_column,
            ) {
                return Some(PanelHit::Action {
                    label: action.label,
                    action: action.action,
                });
            }
            if let Some(action) = crate::account_panel_view::preset_at_content_cell(
                state,
                content_width,
                content_row,
                content_column,
            ) {
                return Some(PanelHit::AccountHit {
                    content_row,
                    hit: AccountPanelHit::TicketPreset(action.action),
                });
            }
            crate::account_panel_view::hit_at_content_row(state, content_width, content_row)
                .map(|hit| PanelHit::AccountHit { content_row, hit })
        }
        Panel::IntentReview => intent_review_hit_at(state, area, column, row),
        Panel::OrderTicket => ticket_hit_at(
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
            content_width(area),
            order_ticket_rows(state),
        ),
        Panel::TransferTicket => ticket_hit_at(
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
            content_width(area),
            transfer_ticket_rows(state),
        ),
        Panel::FuturesState => ticket_hit_at(
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
            content_width(area),
            futures_state_ticket_rows(state),
        ),
        Panel::Settings => {
            let content_row = content_row(area, row)?;
            let content_width = content_width(area);
            let content_column = content_column(area, column).unwrap_or(u16::MAX);
            if let Some(action) = crate::settings_panel_view::action_at_content_cell(
                state,
                content_width,
                content_row,
                content_column,
            ) {
                return Some(PanelHit::SettingAdjust {
                    index: action.action.index,
                    direction: action.action.direction,
                });
            }
            crate::settings_panel_view::setting_index_at_content_row(
                state,
                content_width,
                content_row,
            )
            .map(PanelHit::Row)
        }
        Panel::ProfileRisk => {
            crate::profile_risk_panel_view::action_at_content_row(state, content_row(area, row)?)
                .map(|action| PanelHit::Action {
                    label: action.label,
                    action: action.action,
                })
        }
        Panel::Quote
        | Panel::History
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => crate::read_only_panel_view::info_row_at_content_row(
            state,
            panel,
            area,
            content_row(area, row)?,
        )
        .map(PanelHit::InfoRow),
    }
}

fn intent_review_hit_at(state: &AppState, area: Rect, column: u16, row: u16) -> Option<PanelHit> {
    let content_row = content_row(area, row)?;
    let changes = state.staged_change_review_views();
    if !changes.is_empty()
        && let Some(action) = crate::intent_review_view::action_at_content_cell(
            state
                .staged_change_count()
                .saturating_sub(crate::state::VISIBLE_REVIEW_LIMIT),
            content_width(area),
            crate::intent_review_view::action_state_for_status(
                changes
                    .iter()
                    .find(|change| change.selected)
                    .map(|change| change.stage.queue_status()),
            ),
            content_row,
            content_column(area, column).unwrap_or(u16::MAX),
        )
    {
        return Some(PanelHit::IntentReviewAction(action));
    }
    crate::intent_review_view::staged_change_index_at_content_row(changes.len(), content_row)
        .map(PanelHit::Row)
}

fn ticket_hit_at(
    content_row: usize,
    content_column: u16,
    content_width: u16,
    rows: TicketPanelRows,
) -> Option<PanelHit> {
    if let Some(action) = rows.action_at_content_cell(content_width, content_row, content_column) {
        return Some(PanelHit::Action {
            label: action.label,
            action: action.action,
        });
    }
    if let Some(action) =
        rows.field_action_at_content_cell(content_width, content_row, content_column)
    {
        return Some(PanelHit::TicketFieldAdjust {
            index: action.action.index,
            direction: action.action.direction,
        });
    }
    match rows.click_at(content_row)? {
        TicketPanelClick::Field(index) => Some(PanelHit::TicketField(index)),
        TicketPanelClick::ReadyAction => Some(PanelHit::TicketReadyAction),
    }
}

fn order_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.order_ticket_preview();
    TicketPanelRows {
        detail_count: 1,
        actions: crate::order_ticket_controls::ORDER_TICKET_ACTIONS,
        field_count: OrderTicketField::COUNT,
        field_adjustable: vec![true; OrderTicketField::COUNT],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn transfer_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.transfer_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        actions: &[],
        field_count: TransferTicketField::COUNT,
        field_adjustable: vec![true; TransferTicketField::COUNT],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn futures_state_ticket_rows(state: &AppState) -> TicketPanelRows {
    let preview = state.futures_state_ticket_preview();
    TicketPanelRows {
        detail_count: 0,
        actions: &[],
        field_count: FuturesStateTicketField::MAX_COUNT,
        field_adjustable: vec![true, preview.scope_adjustable(), true],
        ready: preview.ready,
        blocker_count: preview.blockers.len(),
    }
}

fn content_row(area: Rect, row: u16) -> Option<usize> {
    if row <= area.y || row >= area.bottom().saturating_sub(1) {
        return None;
    }
    Some(row.saturating_sub(area.y).saturating_sub(1) as usize)
}

fn content_column(area: Rect, column: u16) -> Option<u16> {
    if column <= area.x || column >= area.right().saturating_sub(1) {
        return None;
    }
    Some(column.saturating_sub(area.x).saturating_sub(1))
}

fn content_width(area: Rect) -> u16 {
    area.width.saturating_sub(2)
}
