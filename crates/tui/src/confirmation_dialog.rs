use crate::model::FloatingKind;
use crate::staged_gate_preview::GatePreviewRow;
use crate::state::{PendingStagedConfirmationView, StagedExecution};

pub(crate) const GATE_ROW_PREFIX: &str = "gate: ";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ConfirmationButtonAction {
    Primary,
    Cancel,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ConfirmationRow {
    Text(String),
    Gate(GatePreviewRow),
    Input {
        label: String,
        value: String,
        matched: bool,
    },
    Blank,
    Buttons(ConfirmationButtons),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ConfirmationButtons {
    pub primary: Option<String>,
    pub cancel: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ConfirmationButtonSegment {
    pub text: String,
    pub action: Option<ConfirmationButtonAction>,
    pub start: usize,
    pub end: usize,
}

pub(crate) fn rows_for(
    kind: FloatingKind,
    pending_staged_confirmation: Option<PendingStagedConfirmationView<'_>>,
    gate_preview: Vec<GatePreviewRow>,
    content_width: usize,
) -> Vec<ConfirmationRow> {
    let rows = match kind {
        FloatingKind::LiveWritesConfirmation => live_writes_rows(),
        FloatingKind::StagedExecutionConfirmation => {
            staged_execution_rows(pending_staged_confirmation, gate_preview)
        }
        _ => Vec::new(),
    };
    materialize_visual_rows(rows, content_width)
}

pub(crate) fn click_action_at(
    rows: &[ConfirmationRow],
    content_column: usize,
    content_row: usize,
) -> Option<ConfirmationButtonAction> {
    let ConfirmationRow::Buttons(buttons) = rows.get(content_row)? else {
        return None;
    };
    button_segments(buttons)
        .into_iter()
        .find(|segment| (segment.start..segment.end).contains(&content_column))
        .and_then(|segment| segment.action)
}

pub(crate) fn button_segments(buttons: &ConfirmationButtons) -> Vec<ConfirmationButtonSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    if let Some(primary) = buttons.primary.as_deref() {
        push_button_segment(
            &mut segments,
            &mut cursor,
            format!("[{primary}]"),
            Some(ConfirmationButtonAction::Primary),
        );
        push_button_segment(&mut segments, &mut cursor, "  ".to_string(), None);
    }
    push_button_segment(
        &mut segments,
        &mut cursor,
        format!("[{}]", buttons.cancel),
        Some(ConfirmationButtonAction::Cancel),
    );
    segments
}

fn push_button_segment(
    segments: &mut Vec<ConfirmationButtonSegment>,
    cursor: &mut usize,
    text: String,
    action: Option<ConfirmationButtonAction>,
) {
    let start = *cursor;
    *cursor += text.chars().count();
    segments.push(ConfirmationButtonSegment {
        text,
        action,
        start,
        end: *cursor,
    });
}

fn live_writes_rows() -> Vec<ConfirmationRow> {
    vec![
        ConfirmationRow::Text("Live writes are disabled by default for every TUI session.".into()),
        ConfirmationRow::Blank,
        ConfirmationRow::Text(
            "Enabling live writes allows staged orders, cancels, transfers, and futures state changes to reach live providers after their own review and risk gates.".into(),
        ),
        ConfirmationRow::Blank,
        ConfirmationRow::Buttons(ConfirmationButtons {
            primary: Some("Enable live writes".into()),
            cancel: "Keep disabled".into(),
        }),
    ]
}

fn staged_execution_rows(
    pending: Option<PendingStagedConfirmationView<'_>>,
    gate_preview: Vec<GatePreviewRow>,
) -> Vec<ConfirmationRow> {
    let Some(pending) = pending else {
        return vec![
            ConfirmationRow::Text("No staged execution is waiting for confirmation.".into()),
            ConfirmationRow::Blank,
            ConfirmationRow::Buttons(ConfirmationButtons {
                primary: None,
                cancel: "Close".into(),
            }),
        ];
    };
    let request = pending.request;

    let mut rows = vec![
        ConfirmationRow::Text("Review the selected staged change before executing it.".into()),
        ConfirmationRow::Blank,
        ConfirmationRow::Text(format!("kind: {}", request.kind_label())),
        ConfirmationRow::Text(format!("id: {}", request.id)),
        ConfirmationRow::Text(format!("summary: {}", request.summary())),
        ConfirmationRow::Blank,
    ];
    match &request.execution {
        StagedExecution::Submit { mode, .. } => {
            rows.push(ConfirmationRow::Text(format!("mode: {mode}")));
            push_gate_preview_rows(&mut rows, gate_preview);
            rows.push(ConfirmationRow::Blank);
            rows.push(ConfirmationRow::Text(
                "This creates an intent and runs the trading runtime gates.".into(),
            ));
            rows.push(ConfirmationRow::Text(
                "Live mode still requires profile permissions, risk policy, intent claim lock, and audit logging.".into(),
            ));
            if let Some(gate) = pending.typed_gate {
                rows.push(ConfirmationRow::Blank);
                rows.push(ConfirmationRow::Text(gate.reason.into()));
                rows.push(ConfirmationRow::Text(format!(
                    "Type {} exactly before submitting.",
                    gate.phrase
                )));
                rows.push(ConfirmationRow::Input {
                    label: "confirmation".into(),
                    value: gate.input.into(),
                    matched: gate.matched,
                });
            }
            rows.push(ConfirmationRow::Blank);
            rows.push(ConfirmationRow::Buttons(ConfirmationButtons {
                primary: pending.can_confirm.then(|| "Confirm submit".into()),
                cancel: "Cancel".into(),
            }));
        }
        StagedExecution::LocalCommit { .. } => {
            if push_gate_preview_rows(&mut rows, gate_preview) {
                rows.push(ConfirmationRow::Blank);
            }
            rows.push(ConfirmationRow::Text(
                "This writes the profile file through the core profile store.".into(),
            ));
            rows.push(ConfirmationRow::Text(
                "A backup is created before replacing an existing profile.".into(),
            ));
            rows.push(ConfirmationRow::Text(
                "The write fails if the profile changes before commit.".into(),
            ));
            rows.push(ConfirmationRow::Blank);
            rows.push(ConfirmationRow::Buttons(ConfirmationButtons {
                primary: Some("Confirm local write".into()),
                cancel: "Cancel".into(),
            }));
        }
    }
    rows
}

fn push_gate_preview_rows(
    rows: &mut Vec<ConfirmationRow>,
    gate_preview: Vec<GatePreviewRow>,
) -> bool {
    if gate_preview.is_empty() {
        return false;
    }
    rows.push(ConfirmationRow::Blank);
    rows.push(ConfirmationRow::Text("gate preview:".into()));
    rows.extend(gate_preview.into_iter().map(ConfirmationRow::Gate));
    true
}

fn materialize_visual_rows(
    rows: Vec<ConfirmationRow>,
    content_width: usize,
) -> Vec<ConfirmationRow> {
    let width = content_width.max(1);
    rows.into_iter()
        .flat_map(|row| match row {
            ConfirmationRow::Text(text) => wrap_text(&text, width)
                .into_iter()
                .map(ConfirmationRow::Text)
                .collect::<Vec<_>>(),
            ConfirmationRow::Gate(row) => wrap_text(
                &row.text,
                width.saturating_sub(GATE_ROW_PREFIX.chars().count()).max(1),
            )
            .into_iter()
            .map(|text| {
                ConfirmationRow::Gate(GatePreviewRow {
                    severity: row.severity,
                    text,
                })
            })
            .collect::<Vec<_>>(),
            row => vec![row],
        })
        .collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if !current.is_empty() && current.chars().count() + separator + word.chars().count() > width
        {
            lines.push(current);
            current = String::new();
        }
        if word.chars().count() > width {
            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            lines.extend(split_long_word(word, width));
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn split_long_word(word: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for character in word.chars() {
        if current.chars().count() == width {
            lines.push(current);
            current = String::new();
        }
        current.push(character);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staged_gate_preview::GatePreviewSeverity;
    use crate::state::{StagedExecutionRequest, TypedConfirmationGateView};
    use agent_finance_core::{DecimalValue, TransferDirection, submit::SubmitMode};
    use std::str::FromStr;

    #[test]
    fn live_writes_buttons_hit_primary_and_cancel_ranges() {
        let rows = rows_for(FloatingKind::LiveWritesConfirmation, None, Vec::new(), 80);
        let button_row = rows
            .iter()
            .position(|row| matches!(row, ConfirmationRow::Buttons(_)))
            .expect("button row is present");
        assert_eq!(
            click_action_at(&rows, 1, button_row),
            Some(ConfirmationButtonAction::Primary)
        );
        assert_eq!(
            click_action_at(&rows, 24, button_row),
            Some(ConfirmationButtonAction::Cancel)
        );
        assert_eq!(click_action_at(&rows, 20, button_row), None);
        assert_eq!(click_action_at(&rows, 1, 0), None);
    }

    #[test]
    fn close_only_dialog_exposes_only_cancel_action() {
        let rows = rows_for(
            FloatingKind::StagedExecutionConfirmation,
            None,
            Vec::new(),
            80,
        );
        let button_row = rows
            .iter()
            .position(|row| matches!(row, ConfirmationRow::Buttons(_)))
            .expect("button row is present");
        assert_eq!(
            click_action_at(&rows, 1, button_row),
            Some(ConfirmationButtonAction::Cancel)
        );
        assert_eq!(click_action_at(&rows, 8, button_row), None);
    }

    #[test]
    fn text_rows_wrap_before_render_and_hit_test() {
        let width = 40;
        let rows = rows_for(
            FloatingKind::LiveWritesConfirmation,
            None,
            Vec::new(),
            width,
        );

        assert!(rows.iter().all(|row| match row {
            ConfirmationRow::Text(text) => text.chars().count() <= width,
            _ => true,
        }));
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, ConfirmationRow::Buttons(_)))
                .count(),
            1
        );
    }

    #[test]
    fn transfer_submit_requires_typed_confirmation_before_primary_button() {
        let request = transfer_execution_request();
        let rows = rows_for(
            FloatingKind::StagedExecutionConfirmation,
            Some(typed_confirmation_view(&request, "", false)),
            Vec::new(),
            80,
        );
        let button_row = rows
            .iter()
            .position(|row| matches!(row, ConfirmationRow::Buttons(_)))
            .expect("button row is present");

        assert!(rows.iter().any(|row| matches!(
            row,
            ConfirmationRow::Input {
                value,
                matched: false,
                ..
            } if value.is_empty()
        )));
        assert_eq!(
            click_action_at(&rows, 1, button_row),
            Some(ConfirmationButtonAction::Cancel)
        );

        let whitespace_rows = rows_for(
            FloatingKind::StagedExecutionConfirmation,
            Some(typed_confirmation_view(&request, " TRANSFER ", false)),
            Vec::new(),
            80,
        );
        let whitespace_button_row = whitespace_rows
            .iter()
            .position(|row| matches!(row, ConfirmationRow::Buttons(_)))
            .expect("button row is present");
        assert_eq!(
            click_action_at(&whitespace_rows, 1, whitespace_button_row),
            Some(ConfirmationButtonAction::Cancel)
        );

        let matched_rows = rows_for(
            FloatingKind::StagedExecutionConfirmation,
            Some(typed_confirmation_view(&request, "TRANSFER", true)),
            Vec::new(),
            80,
        );
        let matched_button_row = matched_rows
            .iter()
            .position(|row| matches!(row, ConfirmationRow::Buttons(_)))
            .expect("button row is present");

        assert!(matched_rows.iter().any(|row| matches!(
            row,
            ConfirmationRow::Input {
                value,
                matched: true,
                ..
            } if value == "TRANSFER"
        )));
        assert_eq!(
            click_action_at(&matched_rows, 1, matched_button_row),
            Some(ConfirmationButtonAction::Primary)
        );
    }

    #[test]
    fn staged_execution_confirmation_surfaces_gate_preview_before_submit_copy() {
        let request = transfer_execution_request();
        let rows = rows_for(
            FloatingKind::StagedExecutionConfirmation,
            Some(typed_confirmation_view(&request, "TRANSFER", true)),
            vec![
                GatePreviewRow {
                    severity: GatePreviewSeverity::Info,
                    text: "core risk preview: allow  findings:0".into(),
                },
                GatePreviewRow {
                    severity: GatePreviewSeverity::Warning,
                    text: "runtime gate: daily transfer limits are checked at submit".into(),
                },
            ],
            80,
        );

        let gate_heading = row_index(&rows, "gate preview:").expect("gate heading is present");
        let submit_copy = row_index(&rows, "This creates an intent")
            .expect("submit copy is present after gate preview");
        assert!(gate_heading < submit_copy);
        assert!(rows.iter().any(|row| matches!(
            row,
            ConfirmationRow::Gate(GatePreviewRow {
                severity: GatePreviewSeverity::Info,
                text,
            }) if text.starts_with("core risk preview:")
        )));
        assert!(rows.iter().any(|row| matches!(
            row,
            ConfirmationRow::Gate(GatePreviewRow {
                severity: GatePreviewSeverity::Warning,
                text,
            }) if text.starts_with("runtime gate:")
        )));
    }

    fn row_index(rows: &[ConfirmationRow], prefix: &str) -> Option<usize> {
        rows.iter()
            .position(|row| matches!(row, ConfirmationRow::Text(text) if text.starts_with(prefix)))
    }

    fn typed_confirmation_view<'a>(
        request: &'a StagedExecutionRequest,
        input: &'a str,
        matched: bool,
    ) -> PendingStagedConfirmationView<'a> {
        PendingStagedConfirmationView {
            request,
            typed_gate: Some(TypedConfirmationGateView {
                phrase: "TRANSFER",
                reason: "Transfers move funds between Binance wallets.",
                input,
                matched,
            }),
            can_confirm: matched,
        }
    }

    fn transfer_execution_request() -> StagedExecutionRequest {
        StagedExecutionRequest {
            id: "transfer-mainnet".into(),
            execution: StagedExecution::Submit {
                subject: crate::state::StagedSubmitSubject::Transfer(
                    crate::state::TransferReview {
                        profile: "mainnet".into(),
                        direction: TransferDirection::SpotToUsdsFutures,
                        asset: "USDT".into(),
                        amount: "5".into(),
                        parsed_amount: DecimalValue::from_str("5").expect("valid decimal"),
                        effective_mode: SubmitMode::DryRun,
                    },
                ),
                mode: SubmitMode::DryRun,
            },
        }
    }
}
