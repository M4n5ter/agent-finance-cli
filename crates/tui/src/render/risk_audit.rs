use agent_finance_core::{DiagnosticCheck, ProfilePermission, RiskPolicy};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::profile_snapshot::ProfileValidationState;
use crate::staged_gate_preview::{self, GatePreviewRow, GatePreviewSeverity};
use crate::state::{AppState, StagedChangeQueueStatus};
use crate::task_log::{TaskLogEntry, TaskStatus};

use super::widgets::{compact_text, panel_block};

pub(super) fn render_risk_audit(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let lines = risk_audit_lines(state);

    frame.render_widget(
        Paragraph::new(hover_lines(
            lines,
            mouse_target,
            state.theme.selected_style(),
        ))
        .block(panel_block(Panel::RiskAudit, state))
        .wrap(Wrap { trim: true }),
        area,
    );
}

pub(crate) fn risk_audit_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "trading gate",
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
        profile_validation_line(state),
    ];
    lines.extend(profile_validation_failures(state));
    lines.extend(risk_policy_lines(state));
    lines.extend(selected_staged_gate_lines(state));
    lines.extend(staged_queue_lines(state));
    lines.extend(recent_event_lines(state));

    lines
}

fn hover_lines(
    lines: Vec<Line<'static>>,
    mouse_target: Option<MouseTarget>,
    selected_style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if mouse_target
                .is_some_and(|target| target.panel_info_row_hovered(Panel::RiskAudit, index))
            {
                line.style(selected_style)
            } else {
                line
            }
        })
        .collect()
}

fn profile_validation_line(state: &AppState) -> Line<'static> {
    match &state.profile_validation {
        ProfileValidationState::Idle => match state.trading_profile.as_deref() {
            Some(profile) => Line::from(format!("profile validation: {profile} pending")),
            None => Line::from(Span::styled(
                "profile validation: no profile",
                state.theme.warning_style(),
            )),
        },
        ProfileValidationState::Loading { profile } => {
            Line::from(format!("profile validation: {profile} loading"))
        }
        ProfileValidationState::Ready {
            profile,
            path,
            checks,
            ..
        } => {
            let failures = required_failure_count(checks);
            if failures == 0 {
                Line::from(format!(
                    "profile validation: {profile} ok  path={}",
                    path.display()
                ))
            } else {
                Line::from(Span::styled(
                    format!(
                        "profile validation: {profile} {failures} required failure(s)  path={}",
                        path.display()
                    ),
                    state.theme.warning_style(),
                ))
            }
        }
        ProfileValidationState::Failed { profile, error } => Line::from(Span::styled(
            format!("profile validation: {profile} failed  {error}"),
            state.theme.warning_style(),
        )),
    }
}

fn profile_validation_failures(state: &AppState) -> Vec<Line<'static>> {
    let ProfileValidationState::Ready { checks, .. } = &state.profile_validation else {
        return Vec::new();
    };

    checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .take(2)
        .map(|check| {
            Line::from(Span::styled(
                format!("required failure: {}", compact_text(&check.message, 70)),
                state.theme.warning_style(),
            ))
        })
        .collect()
}

fn risk_policy_lines(state: &AppState) -> Vec<Line<'static>> {
    let ProfileValidationState::Ready { profile_config, .. } = &state.profile_validation else {
        return vec![Line::from(
            "risk policy: unavailable until profile validation completes",
        )];
    };
    let risk = &profile_config.risk;
    let mut lines = vec![
        Line::from(format!(
            "risk policy: live:{}  daily order cap:{}",
            if risk.allow_live {
                "allowed"
            } else {
                "blocked"
            },
            risk.max_daily_order_notional_usdt
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_string())
        )),
        Line::from(format!(
            "required permissions: {}",
            permission_list_or_none(risk.required_profile_permissions().iter())
        )),
    ];
    lines.push(Line::from(format!(
        "symbols: {}",
        risk_symbol_summary(risk)
    )));
    lines.push(Line::from(format!(
        "transfers:{}  futures-state:{}",
        risk.allowed_transfers.len(),
        risk.allowed_futures_state_changes.len()
    )));
    lines
}

fn selected_staged_gate_lines(state: &AppState) -> Vec<Line<'static>> {
    let Some(preview) = staged_gate_preview::selected_gate_preview(state) else {
        return Vec::new();
    };

    let change = preview.change;
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "selected gate preview",
                state.theme.accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {} {} {}",
                change.change_kind,
                change
                    .mode
                    .map(|mode| mode.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                change.stage
            )),
        ]),
    ];
    lines.extend(
        preview
            .rows
            .into_iter()
            .map(|row| gate_preview_line(state, row)),
    );
    lines
}

fn gate_preview_line(state: &AppState, row: GatePreviewRow) -> Line<'static> {
    match row.severity {
        GatePreviewSeverity::Info => Line::from(row.text),
        GatePreviewSeverity::Warning => {
            Line::from(Span::styled(row.text, state.theme.warning_style()))
        }
        GatePreviewSeverity::Block => {
            Line::from(Span::styled(row.text, state.theme.danger_style()))
        }
    }
}

fn staged_queue_lines(state: &AppState) -> Vec<Line<'static>> {
    let changes = state.staged_change_views();
    let mut lines = vec![Line::from("")];
    if changes.is_empty() {
        lines.push(Line::from("staged queue: empty"));
    } else {
        lines.push(Line::from(format!(
            "staged queue: total:{}  {}",
            changes.len(),
            queue_status_summary(&changes)
        )));
    }
    if let Some(request) = state.pending_staged_confirmation() {
        lines.push(Line::from(Span::styled(
            format!(
                "confirmation pending: {} {}",
                request.kind_label(),
                compact_text(&request.summary(), 54)
            ),
            state.theme.warning_style(),
        )));
    }
    lines
}

fn recent_event_lines(state: &AppState) -> Vec<Line<'static>> {
    let events = state.task_log.iter().rev().take(4).collect::<Vec<_>>();
    let mut lines = vec![Line::from(""), Line::from("recent events")];
    if events.is_empty() {
        lines.push(Line::from("no runtime events yet"));
        return lines;
    }
    lines.extend(events.into_iter().map(task_log_line));
    lines
}

fn task_log_line(entry: &TaskLogEntry) -> Line<'static> {
    Line::from(format!(
        "{} {}",
        task_status_label(entry.status),
        compact_text(&entry.message, 72)
    ))
}

fn task_status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Info => "info",
        TaskStatus::Running => "running",
        TaskStatus::Succeeded => "ok",
        TaskStatus::Warning => "warn",
        TaskStatus::Failed => "fail",
    }
}

fn required_failure_count(checks: &[DiagnosticCheck]) -> usize {
    checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .count()
}

fn permission_list_or_none(values: impl Iterator<Item = ProfilePermission>) -> String {
    let labels = values.map(|value| value.to_string()).collect::<Vec<_>>();
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(",")
    }
}

fn risk_symbol_summary(risk: &RiskPolicy) -> String {
    if risk.allowed_symbols.is_empty() {
        "none".to_string()
    } else {
        let first = risk
            .allowed_symbols
            .iter()
            .take(3)
            .map(|(symbol, policy)| format!("{symbol} <= {}", policy.max_order_notional_usdt))
            .collect::<Vec<_>>()
            .join("; ");
        let hidden = risk.allowed_symbols.len().saturating_sub(3);
        if hidden == 0 {
            first
        } else {
            format!("{first}; +{hidden} more")
        }
    }
}

fn queue_status_summary(changes: &[crate::state::StagedChangeView]) -> String {
    let counts = [
        (StagedChangeQueueStatus::Draft, "draft"),
        (StagedChangeQueueStatus::Ready, "ready"),
        (StagedChangeQueueStatus::Running, "running"),
        (StagedChangeQueueStatus::Done, "done"),
        (StagedChangeQueueStatus::Failed, "failed"),
        (StagedChangeQueueStatus::Closed, "closed"),
    ]
    .into_iter()
    .filter_map(|(status, label)| {
        let count = changes
            .iter()
            .filter(|change| change.stage.queue_status() == status)
            .count();
        (count > 0).then(|| format!("{label}:{count}"))
    })
    .collect::<Vec<_>>();
    counts.join(" ")
}

#[cfg(test)]
mod tests {
    use agent_finance_core::SubmitMode;
    use ratatui::text::Line;

    use super::*;
    use crate::config::TuiConfig;
    use crate::model::WorkspaceKind;
    use crate::profile_snapshot::test_profile_validation_snapshot;
    use crate::state::Action;
    use crate::task_log::TaskKey;

    #[test]
    fn lines_show_trade_gate_queue_and_recent_events() {
        let mut state = trade_state("BTCUSDT");
        load_test_profile(&mut state, "mainnet");
        stage_order(&mut state);
        state.task_log.succeeded(
            TaskKey::ProfileValidation {
                generation: 1,
                profile: "mainnet".to_string(),
            },
            "mainnet profile validation passed",
        );

        let text = risk_audit_text(&state);

        assert!(text.contains("trading gate  live:off / effective:dry-run"));
        assert!(text.contains("profile validation: mainnet ok"));
        assert!(text.contains("risk policy: live:allowed  daily order cap:100"));
        assert!(text.contains("required permissions: spot_trading"));
        assert!(text.contains("symbols: btcusdt <= 50"));
        assert!(text.contains("selected gate preview"));
        assert!(text.contains("runtime preview: order  mode:dry-run  profile:mainnet"));
        assert!(text.contains("live gate: not live; runtime risk still runs before submit"));
        assert!(text.contains("core risk preview: blocked"));
        assert!(text.contains("symbol-not-allowed:"));
        assert!(text.contains("staged queue: total:1  ready:1"));
        assert!(text.contains("recent events"));
        assert!(text.contains("ok mainnet profile validation passed"));
    }

    #[test]
    fn lines_flag_selected_staged_order_without_symbol_policy() {
        let mut state = trade_state("CRDO");
        load_test_profile(&mut state, "mainnet");
        stage_order(&mut state);

        let text = risk_audit_text(&state);

        assert!(text.contains("selected gate preview"));
        assert!(text.contains("core risk preview: blocked"));
        assert!(text.contains("symbol-not-allowed:"));
    }

    #[test]
    fn lines_use_core_transfer_gate_preview() {
        let mut state = account_state();
        load_test_profile(&mut state, "mainnet");
        state.transfer_ticket.set_amount_text(Some("5".to_string()));
        state.reduce(Action::StageTransferTicket);

        let text = risk_audit_text(&state);

        assert!(text.contains("runtime preview: transfer  mode:dry-run  profile:mainnet"));
        assert!(text.contains("core risk preview: blocked"));
        assert!(text.contains("transfer-not-allowed:"));
    }

    #[test]
    fn lines_use_core_futures_state_gate_preview() {
        let mut state = account_state_with_symbol("ETHUSDT");
        load_test_profile(&mut state, "mainnet");
        state.futures_state_ticket.set_leverage(Some(2));
        state.reduce(Action::StageFuturesStateTicket);

        let text = risk_audit_text(&state);

        assert!(text.contains("runtime preview: futures-state  mode:dry-run  profile:mainnet"));
        assert!(text.contains("core risk preview: blocked"));
        assert!(text.contains("futures-state-change-not-allowed:"));
    }

    #[test]
    fn lines_keep_staged_live_mode_after_default_mode_changes() {
        let mut state = trade_state("BTCUSDT");
        load_test_profile(&mut state, "mainnet");
        state.reduce(Action::SetDefaultSubmitMode(SubmitMode::Live));
        state.reduce(Action::SetLiveWritesEnabled(true));
        stage_order(&mut state);
        state.reduce(Action::SetDefaultSubmitMode(SubmitMode::DryRun));

        let text = risk_audit_text(&state);

        assert!(text.contains("selected gate preview  order live ready"));
        assert!(text.contains("runtime preview: order  mode:live  profile:mainnet"));
        assert!(text.contains("live gate: risk.allow_live=true;"));
    }

    #[test]
    fn lines_do_not_attach_other_profile_validation_failure_to_selected_change() {
        let mut state = trade_state("BTCUSDT");
        stage_order(&mut state);
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: "hedge".to_string(),
        });
        state.reduce(Action::ProfileValidationFailed {
            generation: 1,
            profile: "hedge".to_string(),
            error: "invalid hedge profile".to_string(),
        });

        let text = risk_audit_text(&state);

        assert!(
            text.contains("profile gate: selected change uses mainnet, validated profile is hedge")
        );
        assert!(!text.contains("profile gate: validation failed  invalid hedge profile"));
    }

    fn trade_state(symbol: &str) -> AppState {
        AppState::from_config(TuiConfig {
            watchlist: vec![symbol.to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            ..TuiConfig::default()
        })
    }

    fn account_state() -> AppState {
        account_state_with_symbol("BTCUSDT")
    }

    fn account_state_with_symbol(symbol: &str) -> AppState {
        AppState::from_config(TuiConfig {
            watchlist: vec![symbol.to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            ..TuiConfig::default()
        })
    }

    fn load_test_profile(state: &mut AppState, profile: &str) {
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: profile.to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: test_profile_validation_snapshot(profile, format!("{profile}.toml")),
        });
    }

    fn stage_order(state: &mut AppState) {
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));
        state.reduce(Action::StageOrderTicket);
    }

    fn risk_audit_text(state: &AppState) -> String {
        joined_lines(risk_audit_lines(state))
    }

    fn joined_lines(lines: Vec<Line<'static>>) -> String {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
