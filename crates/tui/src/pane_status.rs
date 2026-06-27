use serde::Serialize;

use crate::model::Panel;
use crate::provider_health::ProviderHealthReport;
use crate::state::{AppState, SelectedDataState};
use crate::task_failure::TaskFailureSource;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PaneHealth {
    pub loading: bool,
    pub has_data: bool,
    pub status: TuiPaneStatus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TuiPaneStatus {
    Fresh,
    Loading,
    Partial,
    Empty,
    Error,
    Stale,
}

impl TuiPaneStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Loading => "loading",
            Self::Partial => "partial",
            Self::Empty => "empty",
            Self::Error => "error",
            Self::Stale => "stale",
        }
    }
}

pub fn pane_health(state: &AppState, panel: Panel) -> PaneHealth {
    let data = pane_data_state(state, panel);
    let status = if data.has_error {
        TuiPaneStatus::Error
    } else if data.loading {
        TuiPaneStatus::Loading
    } else if pane_is_empty(state, panel, data) {
        TuiPaneStatus::Empty
    } else if data.selected_data == SelectedDataState::Fresh {
        TuiPaneStatus::Fresh
    } else if data.selected_data == SelectedDataState::Stale {
        TuiPaneStatus::Stale
    } else {
        TuiPaneStatus::Partial
    };

    PaneHealth {
        loading: data.loading,
        has_data: data.selected_data == SelectedDataState::Fresh,
        status,
    }
}

fn pane_is_empty(state: &AppState, panel: Panel, data: PaneDataState) -> bool {
    matches!(
        (panel, data.selected_data),
        (Panel::Evidence, _) if !selected_symbol_is_crypto(state)
    ) || matches!(
        (panel, data.selected_data),
        (Panel::Account, SelectedDataState::Empty) if state.account_snapshot.is_none()
    ) || matches!(
        (panel, data.selected_data),
        (Panel::TaskLog, SelectedDataState::Empty)
    )
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct PaneDataState {
    loading: bool,
    selected_data: SelectedDataState,
    has_error: bool,
}

impl PaneDataState {
    const fn new(loading: bool, selected_data: SelectedDataState, has_error: bool) -> Self {
        Self {
            loading,
            selected_data,
            has_error,
        }
    }
}

fn pane_data_state(state: &AppState, panel: Panel) -> PaneDataState {
    let selected = state.selected_symbol().unwrap_or_default();
    match panel {
        Panel::Watchlist => PaneDataState::new(
            false,
            if state.watchlist.is_empty() {
                SelectedDataState::Empty
            } else {
                SelectedDataState::Fresh
            },
            false,
        ),
        Panel::Quote => PaneDataState::new(
            state.refresh_loading(),
            if state.market_snapshot.is_some() {
                SelectedDataState::Fresh
            } else {
                SelectedDataState::Empty
            },
            state.task_failures.has_source(TaskFailureSource::Quotes),
        ),
        Panel::OrderTicket => PaneDataState::new(false, SelectedDataState::Fresh, false),
        Panel::Account => PaneDataState::new(
            state.account_loading(),
            match state.account_snapshot.as_ref() {
                Some(snapshot) if snapshot.complete() => SelectedDataState::Fresh,
                Some(snapshot) if snapshot.has_data() => SelectedDataState::Stale,
                Some(_) => SelectedDataState::Empty,
                None => SelectedDataState::Empty,
            },
            state.task_failures.has_source(TaskFailureSource::Account),
        ),
        Panel::History => PaneDataState::new(
            state.history.loading(),
            state.history.selected_data_state(selected),
            state.task_failures.has_source(TaskFailureSource::History),
        ),
        Panel::Evidence => PaneDataState::new(
            state.evidence.loading(),
            state.evidence.selected_data_state(selected),
            state
                .task_failures
                .has_source(TaskFailureSource::CryptoEvidence),
        ),
        Panel::Polymarket | Panel::Research => PaneDataState::new(
            state.research.loading(),
            state.research.selected_data_state(selected),
            false,
        ),
        Panel::ProviderHealth => {
            let report = ProviderHealthReport::from_state(state);
            PaneDataState::new(
                state.refresh_loading(),
                if report.is_empty() {
                    SelectedDataState::Empty
                } else {
                    SelectedDataState::Fresh
                },
                false,
            )
        }
        Panel::TaskLog => PaneDataState::new(
            false,
            if state.task_log.is_empty() {
                SelectedDataState::Empty
            } else {
                SelectedDataState::Fresh
            },
            false,
        ),
    }
}

fn selected_symbol_is_crypto(state: &AppState) -> bool {
    state
        .selected_symbol()
        .is_some_and(agent_finance_market::is_likely_crypto_pair)
}
