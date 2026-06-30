use ratatui::layout::Rect;

use crate::account_panel_view::{AccountPanelHit, AccountTicketPreset};
use crate::intent_review_view::IntentReviewAction;
use crate::model::Panel;
use crate::mouse_target::{MousePosition, MouseTarget, PanelMouseAction};
use crate::state::{Action, AppState};
use crate::ticket_panel_view::TicketPanelClick;

pub(crate) fn click_action(
    state: &AppState,
    panel: Panel,
    area: Rect,
    column: u16,
    row: u16,
) -> Option<Action> {
    let hit = panel_hit_at(state, panel, area, column, row)?;
    if panel == Panel::History
        && let PanelHit::ChartPoint { position } = &hit
    {
        return history_chart_price_at(state, area, position.row)
            .map(|price| Action::CaptureChartPrice { price });
    }
    hit.action_for(panel)
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
    ChartPoint {
        position: MousePosition,
    },
}

impl PanelHit {
    fn from_panel_action(action: crate::panel_action_line_view::PanelActionSpan) -> Self {
        Self::Action {
            label: action.label,
            action: action.action,
        }
    }

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
            Self::ChartPoint { position } => PanelMouseAction::InspectChart { position },
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
                return Some(PanelHit::from_panel_action(action));
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
                return Some(PanelHit::from_panel_action(action));
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
            crate::ticket_panel_view::order_ticket_rows(state),
        ),
        Panel::TransferTicket => ticket_hit_at(
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
            content_width(area),
            crate::ticket_panel_view::transfer_ticket_rows(state),
        ),
        Panel::FuturesState => ticket_hit_at(
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
            content_width(area),
            crate::ticket_panel_view::futures_state_ticket_rows(state),
        ),
        Panel::Settings => {
            let content_row = content_row(area, row)?;
            let content_width = content_width(area);
            let content_column = content_column(area, column).unwrap_or(u16::MAX);
            if let Some(action) = crate::settings_panel_view::panel_action_at_content_cell(
                state,
                content_width,
                content_row,
                content_column,
            ) {
                return Some(PanelHit::from_panel_action(action));
            }
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
        Panel::ProfileRisk => crate::profile_risk_panel_view::action_at_content_cell(
            state,
            content_width(area),
            content_row(area, row)?,
            content_column(area, column).unwrap_or(u16::MAX),
        )
        .map(PanelHit::from_panel_action),
        Panel::History => history_hit_at(state, area, column, row),
        Panel::Quote
        | Panel::Evidence
        | Panel::Polymarket
        | Panel::Research
        | Panel::RiskAudit
        | Panel::ProviderHealth
        | Panel::TaskLog => {
            let content_row = content_row(area, row)?;
            let content_column = content_column(area, column).unwrap_or(u16::MAX);
            if let Some(action) = crate::read_only_panel_view::panel_action_at_content_cell(
                state,
                panel,
                area,
                content_row,
                content_column,
            ) {
                return Some(PanelHit::from_panel_action(action));
            }
            crate::read_only_panel_view::info_row_at_content_row(state, panel, area, content_row)
                .map(PanelHit::InfoRow)
        }
    }
}

fn history_hit_at(state: &AppState, area: Rect, column: u16, row: u16) -> Option<PanelHit> {
    let content_row = content_row(area, row)?;
    let content_column = content_column(area, column).unwrap_or(u16::MAX);
    if let Some(action) = crate::read_only_panel_view::panel_action_at_content_cell(
        state,
        Panel::History,
        area,
        content_row,
        content_column,
    ) {
        return Some(PanelHit::from_panel_action(action));
    }
    if let Some(action) = crate::read_only_panel_view::history_toolbar_action_at_content_cell(
        state,
        area,
        content_row,
        content_column,
    ) {
        return Some(PanelHit::from_panel_action(action));
    }
    let workbench = crate::read_only_panel_view::history_workbench_active(state);
    if rect_contains(
        crate::read_only_panel_view::history_chart_area(area, workbench),
        column,
        row,
    ) {
        return Some(PanelHit::ChartPoint {
            position: MousePosition::new(column, row),
        });
    }
    crate::read_only_panel_view::info_row_at_content_row(state, Panel::History, area, content_row)
        .map(PanelHit::InfoRow)
}

fn history_chart_price_at(state: &AppState, panel_area: Rect, row: u16) -> Option<f64> {
    let workbench = crate::read_only_panel_view::history_workbench_active(state);
    let chart_area = crate::read_only_panel_view::history_chart_area(panel_area, workbench);
    state
        .selected_symbol()
        .and_then(|symbol| state.history.selected_snapshot(symbol))
        .and_then(|snapshot| {
            crate::history_chart::chart_price_at_row(
                &snapshot.bars,
                state.chart.window(),
                chart_area,
                row,
            )
        })
}

fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.right() && row >= area.y && row < area.bottom()
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
    rows: crate::ticket_panel_view::TicketPanelRows,
) -> Option<PanelHit> {
    if let Some(action) = rows.action_at_content_cell(content_width, content_row, content_column) {
        return Some(PanelHit::from_panel_action(action));
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
