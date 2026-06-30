use agent_finance_core::Profile;
use agent_finance_core::risk::RiskSeverity;
use agent_finance_core::submit::{SubmitIntentKind, SubmitMode};

use crate::model::FloatingKind;
use crate::profile_snapshot::ProfileValidationState;
use crate::staged_intent::{
    cancel_intent_from_review, futures_state_intent_from_review, order_intent_from_review,
    transfer_intent_from_review,
};
use crate::state::{
    AppState, PendingStagedConfirmationView, ProfileRiskReview, StagedChangeSubject,
    StagedChangeView, StagedExecution, StagedExecutionRequest, StagedLocalCommitSubject,
    StagedSubmitSubject,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GatePreview {
    pub change: StagedChangeView,
    pub rows: Vec<GatePreviewRow>,
}

impl GatePreview {
    pub(crate) fn compact_rows(&self) -> Vec<&GatePreviewRow> {
        compact_row_refs(&self.rows)
    }
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

impl GatePreviewSeverity {
    const fn compact_rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Block => 2,
        }
    }
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

pub(crate) fn confirmation_gate_preview(
    kind: FloatingKind,
    state: &AppState,
    pending: Option<PendingStagedConfirmationView<'_>>,
) -> Vec<GatePreviewRow> {
    match kind {
        FloatingKind::StagedExecutionConfirmation => pending
            .map(|pending| compact_rows(execution_request_gate_rows(state, pending.request)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn selected_gate_rows(state: &AppState, change: &StagedChangeView) -> Vec<GatePreviewRow> {
    gate_rows_for_subject(
        state,
        change.mode,
        change.intent_kind,
        &change.profile,
        &change.subject,
    )
}

fn execution_request_gate_rows(
    state: &AppState,
    request: &StagedExecutionRequest,
) -> Vec<GatePreviewRow> {
    let (subject, mode) = execution_request_subject_and_mode(request);
    gate_rows_for_subject(
        state,
        mode,
        subject.submit_intent_kind(),
        subject.profile_label(),
        &subject,
    )
}

fn execution_request_subject_and_mode(
    request: &StagedExecutionRequest,
) -> (StagedChangeSubject, Option<SubmitMode>) {
    match &request.execution {
        StagedExecution::Submit { subject, mode } => {
            (submit_subject_to_change_subject(subject), Some(*mode))
        }
        StagedExecution::LocalCommit { subject } => {
            (local_commit_subject_to_change_subject(subject), None)
        }
    }
}

fn submit_subject_to_change_subject(subject: &StagedSubmitSubject) -> StagedChangeSubject {
    match subject {
        StagedSubmitSubject::OrderTicket(review) => {
            StagedChangeSubject::OrderTicket(review.clone())
        }
        StagedSubmitSubject::Cancel(review) => StagedChangeSubject::Cancel(review.clone()),
        StagedSubmitSubject::Transfer(review) => StagedChangeSubject::Transfer(review.clone()),
        StagedSubmitSubject::FuturesState(review) => {
            StagedChangeSubject::FuturesState(review.clone())
        }
        #[cfg(test)]
        StagedSubmitSubject::Text {
            intent_kind,
            summary,
        } => StagedChangeSubject::Text {
            intent_kind: *intent_kind,
            summary: summary.clone(),
        },
    }
}

fn local_commit_subject_to_change_subject(
    subject: &StagedLocalCommitSubject,
) -> StagedChangeSubject {
    match subject {
        StagedLocalCommitSubject::ProfileRisk(review) => {
            StagedChangeSubject::ProfileRisk(review.clone())
        }
    }
}

fn gate_rows_for_subject(
    state: &AppState,
    mode: Option<SubmitMode>,
    intent_kind: Option<SubmitIntentKind>,
    profile_label: &str,
    subject: &StagedChangeSubject,
) -> Vec<GatePreviewRow> {
    let Some(mode) = mode else {
        return local_commit_gate_rows(subject);
    };
    let live = mode == SubmitMode::Live;
    let mut rows = vec![GatePreviewRow::info(format!(
        "runtime preview: {}  mode:{}  profile:{}",
        intent_kind
            .map(|kind| kind.to_string())
            .unwrap_or_else(|| "local-commit".to_string()),
        mode,
        profile_label
    ))];
    let profile = match profile_for_label(state, profile_label) {
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
                profile_label, validated
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
    rows.extend(subject_gate_rows(subject, profile, live));
    rows
}

enum ProfileGate<'a> {
    Ready(&'a Profile),
    NoProfileValidation,
    ProfileMismatch { validated: String },
    ValidationFailed { error: String },
}

fn profile_for_label<'a>(state: &'a AppState, profile_label: &str) -> ProfileGate<'a> {
    match &state.profile_validation {
        ProfileValidationState::Ready {
            profile,
            profile_config,
            ..
        } if profile == profile_label => ProfileGate::Ready(profile_config),
        ProfileValidationState::Ready { profile, .. } => ProfileGate::ProfileMismatch {
            validated: profile.clone(),
        },
        ProfileValidationState::Failed { profile, error } if profile == profile_label => {
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

fn compact_rows(rows: Vec<GatePreviewRow>) -> Vec<GatePreviewRow> {
    compact_row_refs(&rows).into_iter().cloned().collect()
}

fn compact_row_refs(rows: &[GatePreviewRow]) -> Vec<&GatePreviewRow> {
    let Some(outcome) = core_outcome_row(rows) else {
        return most_severe_row(rows)
            .into_iter()
            .collect::<Vec<&GatePreviewRow>>();
    };
    let caveat = most_severe_row(rows)
        .filter(|row| !std::ptr::eq(*row, outcome))
        .filter(|row| row.severity != GatePreviewSeverity::Info);
    std::iter::once(outcome).chain(caveat).collect()
}

fn core_outcome_row(rows: &[GatePreviewRow]) -> Option<&GatePreviewRow> {
    rows.iter()
        .find(|row| row.text.starts_with("core risk preview:"))
}

fn most_severe_row(rows: &[GatePreviewRow]) -> Option<&GatePreviewRow> {
    rows.iter().max_by_key(|row| row.severity.compact_rank())
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::command::ActionId;
    use crate::config::{TradingConfig, TuiConfig, WorkspaceConfig};
    use crate::model::{FloatingKind, WorkspaceKind};
    use crate::profile_snapshot::{
        ProfileValidationSnapshot, ProfileValidationState, test_profile,
    };
    use crate::state::Action;
    use agent_finance_core::{OrderKind, SubmitMode};

    #[test]
    fn compact_preview_keeps_core_outcome_before_compact_caveat() {
        let mut state = trade_state("BTCUSDT");
        load_test_profile(&mut state);
        state.reduce(Action::SetLiveWritesEnabled(true));
        state.default_submit_mode = SubmitMode::Live;
        stage_order(&mut state);

        let preview = selected_gate_preview(&state).expect("selected gate preview");
        let compact = compact_text(&preview);

        assert!(compact[0].starts_with("core risk preview:"));
        assert!(
            compact
                .iter()
                .skip(1)
                .any(|text| !text.starts_with("core risk preview:")),
            "a compact caveat should remain visible without replacing the risk outcome"
        );
    }

    #[test]
    fn compact_preview_keeps_blocking_subject_risk_visible() {
        let mut state = trade_state("ETHUSDT");
        load_test_profile(&mut state);
        stage_order(&mut state);

        let preview = selected_gate_preview(&state).expect("selected gate preview");
        let compact = compact_text(&preview);

        assert_eq!(compact[0], "core risk preview: blocked");
        assert!(
            compact
                .iter()
                .any(|text| text.starts_with("symbol-not-allowed:")),
            "blocking subject-level finding should remain visible"
        );
    }

    #[test]
    fn compact_preview_without_core_outcome_uses_most_severe_gate() {
        let mut state = trade_state("BTCUSDT");
        stage_order(&mut state);
        state.profile_validation = ProfileValidationState::ready(
            ProfileValidationSnapshot::from_profile(&test_profile("hedge"), "hedge.toml".into()),
        );

        let preview = selected_gate_preview(&state).expect("selected gate preview");
        let compact = compact_text(&preview);

        assert_eq!(
            compact,
            vec!["profile gate: selected change uses mainnet, validated profile is hedge"]
        );
    }

    #[test]
    fn confirmation_preview_follows_pending_request_not_later_selection() {
        let mut state =
            trade_state_with_watchlist(vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()]);
        load_test_profile(&mut state);
        stage_order(&mut state);
        state.reduce(Action::ExecuteStagedChange);
        assert!(state.pending_staged_confirmation().is_some());

        state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));
        stage_order(&mut state);
        state.reduce(Action::SelectStagedChange(1));
        let selected_preview = selected_gate_preview(&state).expect("selected gate preview");
        assert_eq!(
            compact_text(&selected_preview)[0],
            "core risk preview: blocked",
            "test setup should make the selected non-pending change visibly different"
        );

        let compact = confirmation_gate_preview(
            FloatingKind::StagedExecutionConfirmation,
            &state,
            state.pending_staged_confirmation_view(),
        );

        assert_eq!(compact[0].text, "core risk preview: allowed");
    }

    fn compact_text(preview: &GatePreview) -> Vec<String> {
        preview
            .compact_rows()
            .into_iter()
            .map(|row| row.text.clone())
            .collect()
    }

    fn trade_state(symbol: &str) -> AppState {
        trade_state_with_watchlist(vec![symbol.to_string()])
    }

    fn trade_state_with_watchlist(watchlist: Vec<String>) -> AppState {
        AppState::from_config(TuiConfig {
            watchlist,
            trading: TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            ..TuiConfig::default()
        })
    }

    fn load_test_profile(state: &mut AppState) {
        load_test_profile_named(state, "mainnet");
    }

    fn load_test_profile_named(state: &mut AppState, profile: &str) {
        let mut profile_config = test_profile(profile);
        if let Some(policy) = profile_config.risk.allowed_symbols.get_mut("btcusdt") {
            policy.order_kinds.push(OrderKind::PostOnlyLimit);
        }
        if let Some(policy) = profile_config.risk.allowed_symbols.get("btcusdt").cloned() {
            profile_config
                .risk
                .allowed_symbols
                .insert("BTCUSDT".to_string(), policy);
        }
        state.reduce(Action::ProfileValidationStarted {
            generation: 1,
            profile: profile.to_string(),
        });
        state.reduce(Action::ProfileValidationLoaded {
            generation: 1,
            snapshot: ProfileValidationSnapshot::from_profile(
                &profile_config,
                format!("{profile}.toml").into(),
            ),
        });
    }

    fn stage_order(state: &mut AppState) {
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));
        state.reduce(Action::StageOrderTicket);
    }
}
