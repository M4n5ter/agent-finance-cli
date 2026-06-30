use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Wrap};

use crate::model::Panel;
use agent_finance_core::intent::IntentStatus;

use crate::intent_review_view::{
    INTENT_REVIEW_SUMMARY_ROWS, IntentReviewActionLine, action_line, action_state_for_status,
};
use crate::mouse_target::MouseTarget;
use crate::staged_gate_preview::{GatePreviewRow, GatePreviewSeverity, selected_gate_preview};
use crate::state::{AppState, StagedChangeQueueStatus, StagedChangeView, VISIBLE_REVIEW_LIMIT};

use super::panels::panel_row_hovered;
use super::widgets::panel_block;

const STAGE_COLUMN_WIDTH: u16 = 21;

pub(super) fn render_intent_review(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let changes = state.staged_change_review_views();
    if changes.is_empty() {
        render_empty_intent_review(frame, state, area);
        return;
    }

    frame.render_widget(panel_block(Panel::IntentReview, state), area);
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(INTENT_REVIEW_SUMMARY_ROWS),
            Constraint::Min(3),
        ])
        .split(inner);
    let live_label = if state.live_writes_enabled {
        "live:on"
    } else {
        "live:off"
    };
    let hidden = state
        .staged_change_count()
        .saturating_sub(VISIBLE_REVIEW_LIMIT);
    let summary = intent_review_summary_lines(
        state,
        &changes,
        live_label,
        hidden,
        inner.width,
        mouse_target,
    );
    frame.render_widget(Paragraph::new(summary), chunks[0]);

    frame.render_widget(
        staged_changes_table(state, &changes, mouse_target),
        chunks[1],
    );
}

fn intent_review_summary_lines(
    state: &AppState,
    changes: &[StagedChangeView],
    live_label: &str,
    hidden: usize,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(INTENT_REVIEW_SUMMARY_ROWS as usize);
    lines.push(Line::from(vec![
        Span::styled(
            "operation queue",
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {live_label} / effective:{} / visible:{} / total:{}",
            state.effective_submit_mode(),
            changes.len(),
            state.staged_change_count()
        )),
    ]));

    let selected = changes.iter().find(|change| change.selected);
    if let Some(selected) = selected {
        lines.push(selected_change_line(state, selected));
    } else {
        lines.push(Line::from(Span::styled(
            "selected: none",
            state.theme.warning_style(),
        )));
    }

    let compact_rows = selected_gate_preview(state)
        .map(|preview| preview.compact_rows().into_iter().cloned().collect())
        .unwrap_or_else(|| {
            vec![GatePreviewRow {
                severity: GatePreviewSeverity::Warning,
                text: "gate preview: no selected change".to_string(),
            }]
        });
    lines.push(compact_progress_gate_line(state, selected, &compact_rows));

    lines.push(action_line_to_line(
        state,
        action_line(
            hidden,
            width,
            action_state_for_status(selected.map(|change| change.stage.queue_status())),
        ),
        mouse_target,
    ));
    lines
}

fn selected_change_line(state: &AppState, change: &StagedChangeView) -> Line<'static> {
    let mode = change
        .mode
        .map(|mode| mode.to_string())
        .unwrap_or_else(|| "local".to_string());
    Line::from(vec![
        Span::styled(
            "selected",
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  {} {} {}  {}",
            change.change_kind, mode, change.stage, change.summary
        )),
    ])
}

fn execution_next_step(change: &StagedChangeView) -> &'static str {
    match change.stage.queue_status() {
        StagedChangeQueueStatus::Draft => "next: wait for validation",
        StagedChangeQueueStatus::Ready => "next: execute or close",
        StagedChangeQueueStatus::Running => "next: wait for worker result",
        StagedChangeQueueStatus::Done => "next: close after review",
        StagedChangeQueueStatus::Failed => "next: inspect task log or close",
        StagedChangeQueueStatus::Closed => "next: close",
    }
}

fn compact_progress_gate_line(
    state: &AppState,
    selected: Option<&StagedChangeView>,
    rows: &[GatePreviewRow],
) -> Line<'static> {
    let mut spans = vec![
        Span::styled("progress", state.theme.muted_style()),
        Span::raw("  "),
    ];
    if let Some(change) = selected {
        spans.push(Span::raw(format!(
            "status:{} / stage:{} / {} / intent:{}",
            change.stage.queue_status().label(),
            change.stage,
            execution_next_step(change),
            staged_change_tracking(change),
        )));
    } else {
        spans.push(Span::styled(
            "no selected change",
            state.theme.warning_style(),
        ));
    }
    spans.extend([
        Span::styled("  gate", state.theme.muted_style()),
        Span::raw("  "),
    ]);
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", state.theme.muted_style()));
        }
        spans.push(Span::styled(
            row.text.clone(),
            gate_preview_style(state, row.severity),
        ));
    }
    Line::from(spans)
}

fn gate_preview_style(state: &AppState, severity: GatePreviewSeverity) -> Style {
    match severity {
        GatePreviewSeverity::Info => state.theme.text_style(),
        GatePreviewSeverity::Warning => state.theme.warning_style(),
        GatePreviewSeverity::Block => state.theme.danger_style().add_modifier(Modifier::BOLD),
    }
}

fn action_line_to_line(
    state: &AppState,
    action_line: IntentReviewActionLine,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for action in &action_line.actions {
        push_text_span(
            &mut spans,
            &action_line.text,
            cursor,
            action.byte_start,
            state.theme.text_style(),
        );
        let style = if mouse_target.is_some_and(|target| {
            target.panel_intent_review_action_hovered(Panel::IntentReview, action.action)
        }) {
            state.theme.selected_style().add_modifier(Modifier::BOLD)
        } else {
            state.theme.accent_style().add_modifier(Modifier::BOLD)
        };
        push_text_span(
            &mut spans,
            &action_line.text,
            action.byte_start,
            action.byte_end,
            style,
        );
        cursor = action.byte_end;
    }
    push_text_span(
        &mut spans,
        &action_line.text,
        cursor,
        action_line.text.len(),
        state.theme.text_style(),
    );
    Line::from(spans)
}

fn push_text_span(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    start: usize,
    end: usize,
    style: Style,
) {
    if start < end {
        spans.push(Span::styled(text[start..end].to_string(), style));
    }
}

fn render_empty_intent_review(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let preview = state.order_ticket_preview();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "operation queue",
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  live:{} / effective:{}",
                if state.live_writes_enabled {
                    "on"
                } else {
                    "off"
                },
                state.effective_submit_mode()
            )),
        ]),
        Line::from("No staged changes."),
        Line::from("Stage order tickets from Order Ticket."),
        Line::from("Stage cancels from Open Orders; transfers and futures state from Account."),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "order candidate",
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(if preview.ready {
                " ready to stage"
            } else {
                " blocked"
            }),
        ]),
    ];
    if preview.ready {
        lines.push(Line::from(format!(
            "{} {} {} {} @ {}",
            preview.side,
            preview.quantity.as_deref().unwrap_or("-"),
            preview.symbol.as_deref().unwrap_or("-"),
            preview.kind,
            preview.price.as_deref().unwrap_or("market")
        )));
    } else {
        for blocker in preview.blockers.iter().take(3) {
            lines.push(Line::from(Span::styled(
                format!("blocked: {blocker}"),
                state.theme.warning_style(),
            )));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::IntentReview, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn staged_changes_table<'a>(
    state: &'a AppState,
    changes: &'a [StagedChangeView],
    mouse_target: Option<MouseTarget>,
) -> Table<'a> {
    let rows = changes
        .iter()
        .enumerate()
        .map(|(index, change)| staged_change_row(state, change, mouse_target, index));
    Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Length(STAGE_COLUMN_WIDTH),
            Constraint::Length(8),
            Constraint::Length(13),
            Constraint::Length(10),
            Constraint::Min(24),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new([
            "", "status", "stage", "mode", "kind", "profile", "summary", "intent",
        ])
        .style(state.theme.muted_style().add_modifier(Modifier::BOLD)),
    )
}

fn staged_change_row<'a>(
    state: &'a AppState,
    change: &'a StagedChangeView,
    mouse_target: Option<MouseTarget>,
    index: usize,
) -> Row<'a> {
    let marker = if change.selected { ">" } else { " " };
    let hovered = panel_row_hovered(mouse_target, Panel::IntentReview, index);
    let row_style = if hovered {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else if change.selected {
        state.theme.selected_style()
    } else {
        state.theme.text_style()
    };
    let status = change.stage.queue_status();
    let status_style = staged_status_style(state, status);
    Row::new(vec![
        Cell::from(marker),
        Cell::from(Span::styled(status.label(), status_style)),
        Cell::from(change.stage.to_string()),
        Cell::from(
            change
                .mode
                .map(|mode| mode.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ),
        Cell::from(change.change_kind.to_string()),
        Cell::from(change.profile.clone()),
        Cell::from(change.summary.clone()),
        Cell::from(staged_change_tracking(change)),
    ])
    .style(row_style)
}

fn staged_status_style(state: &AppState, status: StagedChangeQueueStatus) -> Style {
    match status {
        StagedChangeQueueStatus::Ready => state.theme.accent_style(),
        StagedChangeQueueStatus::Running => state.theme.warning_style(),
        StagedChangeQueueStatus::Done => state.theme.success_style(),
        StagedChangeQueueStatus::Failed => state.theme.danger_style(),
        StagedChangeQueueStatus::Closed => state.theme.muted_style(),
        StagedChangeQueueStatus::Draft => state.theme.neutral_style(),
    }
}

fn staged_change_tracking(change: &StagedChangeView) -> String {
    match (&change.intent_id, change.intent_status) {
        (Some(intent_id), Some(status)) => format!("{intent_id} {}", intent_status_label(status)),
        (Some(intent_id), None) => intent_id.clone(),
        (None, _) => "-".to_string(),
    }
}

fn intent_status_label(status: IntentStatus) -> &'static str {
    match status {
        IntentStatus::Created => "created",
        IntentStatus::Submitting => "submitting",
        IntentStatus::Submitted => "submitted",
        IntentStatus::Failed => "failed",
        IntentStatus::Expired => "expired",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StagedChangeStage;

    #[test]
    fn stage_column_width_covers_all_stage_labels() {
        let stages = [
            StagedChangeStage::Draft,
            StagedChangeStage::Validating,
            StagedChangeStage::Ready,
            StagedChangeStage::SubmitQueued,
            StagedChangeStage::IntentCreated,
            StagedChangeStage::DryRunCompleted,
            StagedChangeStage::TestCompleted,
            StagedChangeStage::DryRunFailed,
            StagedChangeStage::TestFailed,
            StagedChangeStage::LivePreflightFailed,
            StagedChangeStage::LiveIntentClaimed,
            StagedChangeStage::LiveSubmitted,
            StagedChangeStage::FailedBeforeIntent,
            StagedChangeStage::IntentFailed,
            StagedChangeStage::LocalCommitQueued,
            StagedChangeStage::LocalCommitted,
            StagedChangeStage::LocalCommitFailed,
            StagedChangeStage::Abandoned,
        ];

        for stage in stages {
            assert!(
                unicode_width::UnicodeWidthStr::width(stage.label()) <= STAGE_COLUMN_WIDTH as usize,
                "{stage} should fit the stage column"
            );
        }
    }
}
