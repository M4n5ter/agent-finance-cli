use ratatui::text::{Line, Span};

use crate::i18n::TuiText;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::panel_action_line_view::{
    PanelActionLine, PanelActionSpan, RenderedPanelActionLine, panel_action_span_at,
    render_panel_action_line,
};
use crate::profile_risk_controls::{PROFILE_RISK_ACTIONS, ProfileRiskActionSpec};
use crate::profile_snapshot::{ProfileValidationState, TradingProfileSnapshot};
use crate::state::AppState;

use crate::render::profile_policy::{
    ProfilePolicyFormat, profile_policy_heading, profile_policy_lines,
};
use crate::render::widgets::compact_text;

pub(crate) struct ProfileRiskPanelRow {
    pub line: Line<'static>,
    pub panel_actions: Vec<PanelActionSpan>,
}

impl ProfileRiskPanelRow {
    fn text(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            panel_actions: Vec::new(),
        }
    }

    fn line(line: Line<'static>) -> Self {
        Self {
            line,
            panel_actions: Vec::new(),
        }
    }

    fn action(
        state: &AppState,
        width: u16,
        mouse_target: Option<MouseTarget>,
        action: ProfileRiskActionSpec,
    ) -> Self {
        let mut action_line = PanelActionLine::new("", width);
        action_line.push_visible_action(action.label, action.action);
        let rendered =
            render_panel_action_line(&action_line, &state.theme, Panel::ProfileRisk, mouse_target);
        Self::panel_action(rendered)
    }

    fn panel_action(rendered: RenderedPanelActionLine) -> Self {
        Self {
            line: rendered.line,
            panel_actions: rendered.actions,
        }
    }

    fn panel_action_at(&self, content_column: u16) -> Option<PanelActionSpan> {
        panel_action_span_at(&self.panel_actions, content_column)
    }
}

pub(crate) fn rows(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Vec<ProfileRiskPanelRow> {
    let text = TuiText::new(state.locale);
    let mut rows = vec![
        ProfileRiskPanelRow::line(profile_policy_heading(&state.theme)),
        ProfileRiskPanelRow::text(text.f(
            "tui-profile-risk-selected-profile",
            &[("profile", state.trading_profile.as_deref().unwrap_or("-"))],
        )),
        ProfileRiskPanelRow::line(validation_summary_line(state, &text)),
    ];

    match &state.profile_validation {
        ProfileValidationState::Ready {
            profile_config,
            checks,
            path,
            ..
        } => {
            rows.push(ProfileRiskPanelRow::text(compact_text(
                &text.f(
                    "tui-profile-risk-path",
                    &[("path", &path.display().to_string())],
                ),
                96,
            )));
            let profile = TradingProfileSnapshot::from(profile_config.as_ref());
            rows.extend(
                profile_policy_lines(&state.theme, &profile, ProfilePolicyFormat::ProfileRisk)
                    .into_iter()
                    .map(compact_line)
                    .map(ProfileRiskPanelRow::line),
            );
            rows.extend(required_failure_lines(state, checks));
        }
        ProfileValidationState::Failed { error, .. } => {
            rows.push(ProfileRiskPanelRow::line(Line::from(Span::styled(
                compact_text(error, 96),
                state.theme.warning_style(),
            ))));
        }
        ProfileValidationState::Loading { .. } | ProfileValidationState::Idle => {}
    }

    rows.push(ProfileRiskPanelRow::text(""));
    rows.extend(
        PROFILE_RISK_ACTIONS
            .map(|action| ProfileRiskPanelRow::action(state, width, mouse_target, action)),
    );
    rows
}

pub(crate) fn action_at_content_cell(
    state: &AppState,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<PanelActionSpan> {
    rows(state, width, None)
        .get(content_row)?
        .panel_action_at(content_column)
}

fn validation_summary_line(state: &AppState, text: &TuiText) -> Line<'static> {
    match &state.profile_validation {
        ProfileValidationState::Idle if state.trading_profile.is_some() => {
            Line::from(text.t("tui-profile-risk-validation-pending"))
        }
        ProfileValidationState::Idle => {
            Line::from(text.t("tui-profile-risk-validation-no-profile"))
        }
        ProfileValidationState::Loading { profile } => Line::from(text.f(
            "tui-profile-risk-validation-loading",
            &[("profile", profile)],
        )),
        ProfileValidationState::Ready { checks, .. } => {
            let required_failures = checks
                .iter()
                .filter(|check| check.required && !check.ok)
                .count();
            if required_failures == 0 {
                Line::from(Span::styled(
                    text.t("tui-profile-risk-validation-ok"),
                    state.theme.success_style(),
                ))
            } else {
                Line::from(Span::styled(
                    text.f(
                        "tui-profile-risk-validation-failures",
                        &[("count", &required_failures.to_string())],
                    ),
                    state.theme.warning_style(),
                ))
            }
        }
        ProfileValidationState::Failed { profile, .. } => Line::from(Span::styled(
            text.f(
                "tui-profile-risk-validation-failed",
                &[("profile", profile)],
            ),
            state.theme.warning_style(),
        )),
    }
}

fn required_failure_lines(
    state: &AppState,
    checks: &[agent_finance_core::DiagnosticCheck],
) -> Vec<ProfileRiskPanelRow> {
    let text = TuiText::new(state.locale);
    checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .take(3)
        .map(|check| {
            ProfileRiskPanelRow::line(Line::from(Span::styled(
                compact_text(
                    &text.f("tui-profile-risk-failure", &[("message", &check.message)]),
                    96,
                ),
                state.theme.warning_style(),
            )))
        })
        .collect()
}

fn compact_line(line: Line<'static>) -> Line<'static> {
    let text = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    if text.chars().count() <= 96 {
        return line;
    }
    Line::from(Span::styled(compact_text(&text, 96), line.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;

    #[test]
    fn rows_mark_profile_risk_actions_as_clickable_metadata() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        let actions = rows(&state, 100, None)
            .into_iter()
            .flat_map(|row| row.panel_actions)
            .map(|action| action.action)
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                ActionId::OpenFloating(crate::model::FloatingKind::TradingProfile),
                ActionId::RevalidateTradingProfile,
                ActionId::StageProfileLiveToggle,
            ]
        );
    }

    #[test]
    fn action_hit_test_uses_visible_label_cells_only() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let rendered_rows = rows(&state, 100, None);
        let (content_row, span) = rendered_rows
            .iter()
            .enumerate()
            .find_map(|(content_row, row)| {
                row.panel_actions
                    .iter()
                    .find(|span| {
                        span.action
                            == ActionId::OpenFloating(crate::model::FloatingKind::TradingProfile)
                    })
                    .map(|span| (content_row, span.clone()))
            })
            .expect("profile editor action is rendered");

        assert_eq!(
            action_at_content_cell(&state, 100, content_row, span.start),
            Some(span.clone())
        );
        assert_eq!(
            action_at_content_cell(&state, 100, content_row, span.end),
            None
        );
    }
}
