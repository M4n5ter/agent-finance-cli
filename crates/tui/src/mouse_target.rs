use crate::command::ActionId;
use crate::confirmation_dialog::ConfirmationButtonAction;
use crate::intent_review_view::IntentReviewAction;
use crate::layout::{CockpitLayout, LayoutHit};
use crate::model::{FloatingKind, Panel, WorkspaceKind};
use crate::state::AppState;
use crate::status_bar::StatusAction;
use crate::workspace_tabs::workspace_tab_at;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MousePosition {
    pub column: u16,
    pub row: u16,
}

impl MousePosition {
    pub const fn new(column: u16, row: u16) -> Self {
        Self { column, row }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MouseTarget {
    WorkspaceTab(WorkspaceKind),
    Panel(Panel),
    PanelAction {
        panel: Panel,
        action: PanelMouseAction,
    },
    Floating(FloatingKind),
    FloatingAction {
        kind: FloatingKind,
        action: FloatingMouseAction,
    },
    FloatingResize(FloatingKind),
    StatusAction(StatusAction),
    DockedSplit,
}

impl MouseTarget {
    pub fn workspace_tab_hovered(self, workspace: WorkspaceKind) -> bool {
        matches!(self, Self::WorkspaceTab(hit) if hit == workspace)
    }

    pub fn panel_row_hovered(self, panel: Panel, index: usize) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::SelectRow { index: hover_index },
            } if hover_panel == panel && hover_index == index
        )
    }

    pub fn panel_field_hovered(self, panel: Panel, index: usize) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::SelectField { index: hover_index },
            } if hover_panel == panel && hover_index == index
        )
    }

    pub fn panel_field_adjust_hovered(self, panel: Panel, index: usize, direction: isize) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::AdjustField {
                    index: hover_index,
                    direction: hover_direction,
                },
            } if hover_panel == panel && hover_index == index && hover_direction == direction
        )
    }

    pub fn panel_ready_action_hovered(self, panel: Panel) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::StageReadyChange,
            } if hover_panel == panel
        )
    }

    pub fn panel_action_hovered(self, panel: Panel, action: ActionId) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::ExecuteAction {
                    action: hover_action,
                    ..
                },
            } if hover_panel == panel && hover_action == action
        )
    }

    pub fn panel_row_action_hovered(self, panel: Panel, content_row: usize) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::RowAction { content_row: hover_content_row },
            } if hover_panel == panel && hover_content_row == content_row
        )
    }

    pub fn panel_setting_adjust_hovered(
        self,
        panel: Panel,
        index: usize,
        direction: isize,
    ) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::SettingAdjust {
                    index: hover_index,
                    direction: hover_direction,
                },
            } if hover_panel == panel && hover_index == index && hover_direction == direction
        )
    }

    pub fn panel_intent_review_action_hovered(
        self,
        panel: Panel,
        action: IntentReviewAction,
    ) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::IntentReviewAction {
                    action: hover_action,
                },
            } if hover_panel == panel && hover_action == action
        )
    }

    pub fn panel_info_row_hovered(self, panel: Panel, index: usize) -> bool {
        matches!(
            self,
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::InspectRow { index: hover_index },
            } if hover_panel == panel && hover_index == index
        )
    }

    pub fn panel_chart_hovered(self, panel: Panel) -> Option<MousePosition> {
        match self {
            Self::PanelAction {
                panel: hover_panel,
                action: PanelMouseAction::InspectChart { position },
            } if hover_panel == panel => Some(position),
            _ => None,
        }
    }

    pub fn floating_result_hovered(self, kind: FloatingKind, index: usize) -> bool {
        matches!(
            self,
            Self::FloatingAction {
                kind: hover_kind,
                action: FloatingMouseAction::ExecuteResult { index: hover_index }
                    | FloatingMouseAction::SelectResult { index: hover_index },
            } if hover_kind == kind && hover_index == index
        )
    }

    pub fn confirmation_button_hovered(
        self,
        kind: FloatingKind,
    ) -> Option<ConfirmationButtonAction> {
        match self {
            Self::FloatingAction {
                kind: hover_kind,
                action: FloatingMouseAction::Confirm,
            } if hover_kind == kind => Some(ConfirmationButtonAction::Primary),
            Self::FloatingAction {
                kind: hover_kind,
                action: FloatingMouseAction::Cancel,
            } if hover_kind == kind => Some(ConfirmationButtonAction::Cancel),
            _ => None,
        }
    }

    pub fn status_action_hovered(self, action: StatusAction) -> bool {
        matches!(self, Self::StatusAction(hover_action) if hover_action == action)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PanelMouseAction {
    SelectRow {
        index: usize,
    },
    SelectField {
        index: usize,
    },
    AdjustField {
        index: usize,
        direction: isize,
    },
    StageReadyChange,
    ExecuteAction {
        label: &'static str,
        action: ActionId,
    },
    RowAction {
        content_row: usize,
    },
    SettingAdjust {
        index: usize,
        direction: isize,
    },
    IntentReviewAction {
        action: IntentReviewAction,
    },
    InspectRow {
        index: usize,
    },
    InspectChart {
        position: MousePosition,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FloatingMouseAction {
    ExecuteResult { index: usize },
    SelectResult { index: usize },
    Confirm,
    Cancel,
}

pub(crate) fn target_at(
    state: &AppState,
    layout: &CockpitLayout,
    position: MousePosition,
) -> Option<MouseTarget> {
    if mouse_is_blocked_by_modal(state) {
        return modal_target_at(state, layout, position);
    }

    match layout.hit_test(position.column, position.row)? {
        LayoutHit::Panel(panel) => layout
            .panel_rect(panel)
            .and_then(|area| {
                crate::panel_mouse::hover_target(state, panel, area, position.column, position.row)
            })
            .or(Some(MouseTarget::Panel(panel))),
        LayoutHit::DockedSplit(_) => Some(MouseTarget::DockedSplit),
        LayoutHit::FloatingResize(kind) => Some(MouseTarget::FloatingResize(kind)),
        LayoutHit::Floating(kind) => layout
            .floating_rect(kind)
            .and_then(|area| {
                crate::floating_input::hover_target(
                    state,
                    kind,
                    area,
                    position.column,
                    position.row,
                )
            })
            .or(Some(MouseTarget::Floating(kind))),
        LayoutHit::Status => workspace_tab_at(layout.status, position.column)
            .map(MouseTarget::WorkspaceTab)
            .or_else(|| {
                crate::status_bar::visible_action_at(
                    state,
                    crate::status_bar::areas(layout.status).detail,
                    position.column,
                )
                .map(MouseTarget::StatusAction)
            }),
    }
}

fn modal_target_at(
    state: &AppState,
    layout: &CockpitLayout,
    position: MousePosition,
) -> Option<MouseTarget> {
    let kind = state.floating.last()?.kind;
    if !matches!(
        kind,
        FloatingKind::LiveWritesConfirmation | FloatingKind::StagedExecutionConfirmation
    ) {
        return None;
    }
    let area = layout.floating_rect(kind)?;
    layout
        .hit_test(position.column, position.row)
        .and_then(|hit| match hit {
            LayoutHit::Floating(hit_kind) | LayoutHit::FloatingResize(hit_kind)
                if hit_kind == kind =>
            {
                crate::floating_input::hover_target(
                    state,
                    kind,
                    area,
                    position.column,
                    position.row,
                )
            }
            _ => None,
        })
}

fn mouse_is_blocked_by_modal(state: &AppState) -> bool {
    crate::floating_input::live_writes_confirmation_is_top(state)
        || crate::floating_input::staged_execution_confirmation_is_top(state)
}
