use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::model::Panel;
use crate::profile_snapshot::{ProfileValidationState, TradingProfileSnapshot};
use crate::state::AppState;

use super::profile_policy::{ProfilePolicyFormat, profile_policy_heading, profile_policy_lines};
use super::widgets::{compact_text, panel_block};

pub(super) fn render_profile_risk(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let mut lines = vec![
        profile_policy_heading(&state.theme),
        Line::from(format!(
            "selected profile: {}",
            state.trading_profile.as_deref().unwrap_or("-")
        )),
        validation_summary_line(state),
    ];

    match &state.profile_validation {
        ProfileValidationState::Ready {
            profile_config,
            checks,
            path,
            ..
        } => {
            lines.push(Line::from(format!("path: {}", path.display())));
            let profile = TradingProfileSnapshot::from(profile_config.as_ref());
            lines.extend(profile_policy_lines(
                &state.theme,
                &profile,
                ProfilePolicyFormat::ProfileRisk,
            ));
            lines.extend(required_failure_lines(state, checks));
        }
        ProfileValidationState::Failed { error, .. } => {
            lines.push(Line::from(Span::styled(
                compact_text(error, 96),
                state.theme.warning_style(),
            )));
        }
        ProfileValidationState::Loading { .. } | ProfileValidationState::Idle => {}
    }

    lines.extend([
        Line::from(""),
        Line::from(crate::profile_risk_controls::profile_risk_panel_hint()),
    ]);

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::ProfileRisk, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn validation_summary_line(state: &AppState) -> Line<'static> {
    match &state.profile_validation {
        ProfileValidationState::Idle if state.trading_profile.is_some() => {
            Line::from("validation: pending")
        }
        ProfileValidationState::Idle => Line::from("validation: no profile selected"),
        ProfileValidationState::Loading { profile } => {
            Line::from(format!("validation: {profile} loading"))
        }
        ProfileValidationState::Ready { checks, .. } => {
            let required_failures = checks
                .iter()
                .filter(|check| check.required && !check.ok)
                .count();
            if required_failures == 0 {
                Line::from(Span::styled("validation: ok", state.theme.success_style()))
            } else {
                Line::from(Span::styled(
                    format!("validation: {required_failures} required failure(s)"),
                    state.theme.warning_style(),
                ))
            }
        }
        ProfileValidationState::Failed { profile, .. } => Line::from(Span::styled(
            format!("validation: {profile} failed"),
            state.theme.warning_style(),
        )),
    }
}

fn required_failure_lines(
    state: &AppState,
    checks: &[agent_finance_core::DiagnosticCheck],
) -> Vec<Line<'static>> {
    checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .take(3)
        .map(|check| {
            Line::from(Span::styled(
                format!("failure: {}", check.message),
                state.theme.warning_style(),
            ))
        })
        .collect()
}
