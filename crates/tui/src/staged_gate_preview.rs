use agent_finance_core::Profile;
use agent_finance_core::risk::RiskSeverity;
use agent_finance_core::submit::SubmitMode;

use crate::profile_snapshot::ProfileValidationState;
use crate::staged_intent::{
    cancel_intent_from_review, futures_state_intent_from_review, order_intent_from_review,
    transfer_intent_from_review,
};
use crate::state::{AppState, ProfileRiskReview, StagedChangeSubject, StagedChangeView};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GatePreview {
    pub change: StagedChangeView,
    pub rows: Vec<GatePreviewRow>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct GatePreviewRow {
    pub severity: GatePreviewSeverity,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GatePreviewSeverity {
    Info,
    Warning,
    Block,
}

impl GatePreviewRow {
    fn info(text: impl Into<String>) -> Self {
        Self {
            severity: GatePreviewSeverity::Info,
            text: text.into(),
        }
    }

    fn warning(text: impl Into<String>) -> Self {
        Self {
            severity: GatePreviewSeverity::Warning,
            text: text.into(),
        }
    }

    fn block(text: impl Into<String>) -> Self {
        Self {
            severity: GatePreviewSeverity::Block,
            text: text.into(),
        }
    }
}

pub(crate) fn selected_gate_preview(state: &AppState) -> Option<GatePreview> {
    let change = state
        .staged_change_review_views()
        .into_iter()
        .find(|change| change.selected)?;
    let rows = selected_gate_rows(state, &change);
    Some(GatePreview { change, rows })
}

fn selected_gate_rows(state: &AppState, change: &StagedChangeView) -> Vec<GatePreviewRow> {
    let Some(mode) = change.mode else {
        return local_commit_gate_rows(&change.subject);
    };
    let live = mode == SubmitMode::Live;
    let mut rows = vec![GatePreviewRow::info(format!(
        "runtime preview: {}  mode:{}  profile:{}",
        change
            .intent_kind
            .map(|kind| kind.to_string())
            .unwrap_or_else(|| "local-commit".to_string()),
        mode,
        change.profile
    ))];
    let profile = match profile_for_change(state, change) {
        ProfileGate::Ready(profile) => profile,
        ProfileGate::NoProfileValidation => {
            rows.push(GatePreviewRow::warning(
                "profile gate: unavailable until profile validation completes",
            ));
            return rows;
        }
        ProfileGate::ProfileMismatch { validated } => {
            rows.push(GatePreviewRow::block(format!(
                "profile gate: selected change uses {}, validated profile is {}",
                change.profile, validated
            )));
            return rows;
        }
        ProfileGate::ValidationFailed { error } => {
            rows.push(GatePreviewRow::block(format!(
                "profile gate: validation failed  {error}"
            )));
            return rows;
        }
    };

    rows.push(live_gate_row(live, profile));
    rows.extend(subject_gate_rows(&change.subject, profile, live));
    rows
}

enum ProfileGate<'a> {
    Ready(&'a Profile),
    NoProfileValidation,
    ProfileMismatch { validated: String },
    ValidationFailed { error: String },
}

fn profile_for_change<'a>(state: &'a AppState, change: &StagedChangeView) -> ProfileGate<'a> {
    match &state.profile_validation {
        ProfileValidationState::Ready {
            profile,
            profile_config,
            ..
        } if profile == &change.profile => ProfileGate::Ready(profile_config),
        ProfileValidationState::Ready { profile, .. } => ProfileGate::ProfileMismatch {
            validated: profile.clone(),
        },
        ProfileValidationState::Failed { profile, error } if profile == &change.profile => {
            ProfileGate::ValidationFailed {
                error: error.clone(),
            }
        }
        ProfileValidationState::Failed { profile, .. } => ProfileGate::ProfileMismatch {
            validated: profile.clone(),
        },
        ProfileValidationState::Idle | ProfileValidationState::Loading { .. } => {
            ProfileGate::NoProfileValidation
        }
    }
}

fn live_gate_row(live: bool, profile: &Profile) -> GatePreviewRow {
    if !live {
        return GatePreviewRow::info("live gate: not live; runtime risk still runs before submit");
    }
    if profile.risk.allow_live {
        GatePreviewRow::info(
            "live gate: risk.allow_live=true; submit still checks policy, runtime state, and audit",
        )
    } else {
        GatePreviewRow::block("live gate: risk.allow_live=false blocks live submit")
    }
}

fn subject_gate_rows(
    subject: &StagedChangeSubject,
    profile: &Profile,
    live: bool,
) -> Vec<GatePreviewRow> {
    match subject {
        StagedChangeSubject::OrderTicket(review) => {
            preview_decision(agent_finance_core::check_order_intent(
                profile,
                &order_intent_from_review(profile, review, "af-tui-preview".to_string()),
                live,
            ))
            .with_runtime_note(
                live,
                "runtime gate: daily order notional is checked at submit",
            )
        }
        StagedChangeSubject::Cancel(review) => {
            preview_decision(agent_finance_core::check_cancel_intent(
                profile,
                &cancel_intent_from_review(profile, review),
                live,
            ))
        }
        StagedChangeSubject::Transfer(review) => {
            preview_decision(agent_finance_core::check_transfer_intent(
                profile,
                &transfer_intent_from_review(
                    profile,
                    review,
                    "af-tui-transfer-preview".to_string(),
                ),
                live,
            ))
        }
        StagedChangeSubject::FuturesState(review) => {
            preview_decision(agent_finance_core::check_futures_state_intent(
                profile,
                &futures_state_intent_from_review(profile, review),
                live,
            ))
        }
        StagedChangeSubject::ProfileRisk(review) => profile_risk_rows(review),
        #[cfg(test)]
        StagedChangeSubject::Text { .. } => {
            vec![GatePreviewRow::info("test gate: no runtime policy preview")]
        }
    }
}

fn local_commit_gate_rows(subject: &StagedChangeSubject) -> Vec<GatePreviewRow> {
    match subject {
        StagedChangeSubject::ProfileRisk(review) => profile_risk_rows(review),
        #[cfg(test)]
        StagedChangeSubject::Text { .. } => {
            vec![GatePreviewRow::info("test gate: no runtime policy preview")]
        }
        StagedChangeSubject::OrderTicket(_)
        | StagedChangeSubject::Cancel(_)
        | StagedChangeSubject::Transfer(_)
        | StagedChangeSubject::FuturesState(_) => {
            vec![GatePreviewRow::warning(
                "runtime preview: staged change has no submit mode",
            )]
        }
    }
}

trait GatePreviewRowsExt {
    fn with_runtime_note(self, include: bool, text: &'static str) -> Vec<GatePreviewRow>;
}

impl GatePreviewRowsExt for Vec<GatePreviewRow> {
    fn with_runtime_note(mut self, include: bool, text: &'static str) -> Vec<GatePreviewRow> {
        if include {
            self.push(GatePreviewRow::warning(text));
        }
        self
    }
}

fn preview_decision(decision: agent_finance_core::RiskDecision) -> Vec<GatePreviewRow> {
    let mut rows = vec![GatePreviewRow::info(format!(
        "core risk preview: {}",
        if decision.allowed {
            "allowed"
        } else {
            "blocked"
        }
    ))];
    rows.extend(decision.findings.into_iter().map(|finding| {
        let text = format!("{}: {}", finding.code, finding.message);
        match finding.severity {
            RiskSeverity::Info => GatePreviewRow::info(text),
            RiskSeverity::Block => GatePreviewRow::block(text),
        }
    }));
    rows
}

fn profile_risk_rows(review: &ProfileRiskReview) -> Vec<GatePreviewRow> {
    vec![
        GatePreviewRow::info(format!(
            "profile-risk gate: diff:{} checks:{} required-failures:{}",
            review.diff.len(),
            review.checks.len(),
            review.required_failure_count
        )),
        GatePreviewRow::info("local commit: backup and stale-content check run at commit time"),
    ]
}
