use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use agent_finance_i18n::LocaleId;

use crate::action_line_view::{ActionLine, ActionSpan};
use crate::command::ActionId;
use crate::hints::{self, StatusHint};
use crate::i18n::TuiText;
use crate::state::AppState;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StatusAreas {
    pub tabs: Rect,
    pub detail: Rect,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StatusDetail {
    pub text: String,
    pub actions: Vec<StatusActionSpan>,
}

pub(crate) type StatusActionSpan = ActionSpan<StatusAction>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct StatusAction {
    pub label: &'static str,
    pub action: ActionId,
}

pub(crate) fn detail(state: &AppState, symbol: &str, errors: usize, width: u16) -> StatusDetail {
    let text = TuiText::new(state.locale);
    let runtime = runtime_label(state, &text);
    let live = live_label(state, &text);
    let compact = format!(
        " {symbol} {} live:{}{} {runtime} e:{errors} ",
        state.interaction_mode().label(),
        live,
        compact_config_segment(state),
    );
    let compact_without_errors = format!(
        " {symbol} {} live:{}{} {runtime} ",
        state.interaction_mode().label(),
        live,
        compact_config_segment(state),
    );
    let terse = format!(" {symbol} {runtime} ");
    let (medium, medium_without_errors, semantic_short) =
        if let Some(profile) = state.trading_profile.as_deref() {
            (
                format!(
                    " {symbol} | profile: {profile} | live:{}{} | {} | {runtime} | e:{errors} ",
                    live,
                    config_segment(state),
                    state.effective_submit_mode()
                ),
                format!(
                    " {symbol} | profile: {profile} | live:{}{} | {} | {runtime} ",
                    live,
                    config_segment(state),
                    state.effective_submit_mode()
                ),
                format!(
                    " {symbol} profile: {profile} live:{}{} {} {runtime} ",
                    live,
                    compact_config_segment(state),
                    state.effective_submit_mode()
                ),
            )
        } else {
            (
                format!(
                    " {symbol} | mode: {} | live:{}{} | {} | focus: {} | {runtime} | e:{errors} ",
                    state.interaction_mode().label(),
                    live,
                    config_segment(state),
                    state.effective_submit_mode(),
                    text.panel_title(state.panels.focused()),
                ),
                format!(
                    " {symbol} | mode: {} | live:{}{} | {} | {runtime} ",
                    state.interaction_mode().label(),
                    live,
                    config_segment(state),
                    state.effective_submit_mode(),
                ),
                format!(
                    " {symbol} mode: {} live:{}{} {} {runtime} ",
                    state.interaction_mode().label(),
                    live,
                    compact_config_segment(state),
                    state.effective_submit_mode(),
                ),
            )
        };

    let long = long_detail(state, symbol, errors, &runtime, width, &text);
    fit_status_detail(
        width,
        std::iter::once(long)
            .chain([
                StatusDetail::plain(medium),
                StatusDetail::plain(medium_without_errors),
                StatusDetail::plain(semantic_short),
                StatusDetail::plain(compact),
                StatusDetail::plain(compact_without_errors),
            ])
            .chain([StatusDetail::plain(terse)]),
    )
}

pub(crate) fn action_at(
    state: &AppState,
    symbol: &str,
    errors: usize,
    area: Rect,
    column: u16,
) -> Option<StatusAction> {
    if !(area.x..area.right()).contains(&column) {
        return None;
    }
    let detail = detail(state, symbol, errors, area.width);
    let relative_column = column.saturating_sub(area.x);
    detail
        .actions
        .into_iter()
        .find(|span| (span.start..span.end).contains(&relative_column))
        .map(|span| span.action)
}

fn long_detail(
    state: &AppState,
    symbol: &str,
    errors: usize,
    runtime: &str,
    width: u16,
    text: &TuiText,
) -> StatusDetail {
    let profile = state
        .trading_profile
        .as_deref()
        .map(|profile| format!(" | profile: {profile}"))
        .unwrap_or_default();
    let prefix = format!(
        " {symbol} | mode: {}{profile}{} | {} | focus: {} | visible: {}/{} | {runtime} | errors: {errors} | ",
        state.interaction_mode().label(),
        config_segment(state),
        write_label(state, text),
        text.panel_title(state.panels.focused()),
        state.visible_panels().len(),
        state.workspace.panels().len(),
    );
    let hint_budget =
        width.saturating_sub(UnicodeWidthStr::width(prefix.as_str()) as u16 + 1) as usize;
    let key_hints = hints::status_key_hint_specs(state, hint_budget);
    detail_with_hints(prefix, key_hints, " ")
}

fn detail_with_hints(prefix: String, hints: Vec<StatusHint>, suffix: &'static str) -> StatusDetail {
    let mut line = ActionLine::new(prefix, u16::MAX);
    for (index, hint) in hints.into_iter().enumerate() {
        if index > 0 {
            line.push_visible_text("  ");
        }
        if let Some(action) = hint.action {
            line.push_visible_action(
                &hint.text,
                StatusAction {
                    label: action.mouse_label,
                    action: action.action,
                },
            );
        } else {
            line.push_visible_text(&hint.text);
        }
    }
    line.push_visible_text(suffix);
    StatusDetail {
        text: line.text,
        actions: line.actions,
    }
}

fn fit_status_detail(
    width: u16,
    candidates: impl IntoIterator<Item = StatusDetail>,
) -> StatusDetail {
    let width = width as usize;
    candidates
        .into_iter()
        .find(|candidate| UnicodeWidthStr::width(candidate.text.as_str()) <= width)
        .unwrap_or_else(|| StatusDetail::plain(String::new()))
}

fn runtime_label(state: &AppState, text: &TuiText) -> String {
    if state.scheduler_error.is_some() {
        text.t("tui-status-scheduler-error")
    } else if state.refresh_loading() {
        text.t("tui-status-refreshing")
    } else {
        text.t("tui-status-ready")
    }
}

fn write_label(state: &AppState, text: &TuiText) -> String {
    format!(
        "live: {} / write: {}",
        live_label(state, text),
        state.effective_submit_mode()
    )
}

fn live_label(state: &AppState, text: &TuiText) -> String {
    if state.live_writes_enabled {
        text.t("tui-status-on")
    } else {
        text.t("tui-status-off")
    }
}

fn compact_config_segment(state: &AppState) -> String {
    config_changes_label(state)
        .map(|label| format!(" cfg:{label}"))
        .unwrap_or_default()
}

fn config_segment(state: &AppState) -> String {
    config_changes_label(state)
        .map(|label| format!(" | cfg:{label}"))
        .unwrap_or_default()
}

fn config_changes_label(state: &AppState) -> Option<String> {
    (!state.config_changes.is_empty()).then(|| state.config_changes.join(","))
}

impl StatusDetail {
    fn plain(text: String) -> Self {
        Self {
            text,
            actions: Vec::new(),
        }
    }
}

pub(crate) fn status_symbol_and_errors(state: &AppState) -> (&str, usize) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let errors = state
        .market_snapshot
        .as_ref()
        .map(|snapshot| snapshot.errors.len())
        .unwrap_or(0);
    (symbol, errors)
}

pub(crate) fn areas(area: Rect, locale: LocaleId) -> StatusAreas {
    let tab_width = crate::workspace_tabs::workspace_tabs_width(locale).min(area.width);
    StatusAreas {
        tabs: Rect {
            x: area.x,
            y: area.y,
            width: tab_width,
            height: area.height,
        },
        detail: Rect {
            x: area.x.saturating_add(tab_width),
            y: area.y,
            width: area.width.saturating_sub(tab_width),
            height: area.height,
        },
    }
}

pub(crate) fn visible_action_at(state: &AppState, area: Rect, column: u16) -> Option<StatusAction> {
    let (symbol, errors) = status_symbol_and_errors(state);
    action_at(state, symbol, errors, area, column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::{LocaleConfig, TuiConfig};
    use crate::model::FloatingKind;

    #[test]
    fn action_spans_match_visible_status_text() {
        let state = AppState::from_config(TuiConfig::default());
        let area = Rect::new(32, 20, 150, 1);
        let detail = detail(&state, "CRDO", 0, area.width);
        let expected = [
            (
                ActionId::OpenFloating(FloatingKind::CommandPalette),
                ": command",
            ),
            (
                ActionId::OpenFloating(FloatingKind::SymbolSearch),
                "/ search",
            ),
            (ActionId::OpenFloating(FloatingKind::Help), "h help"),
        ];

        assert_action_spans(&state, area, &detail, expected);
    }

    #[test]
    fn wide_action_spans_include_lower_priority_status_commands() {
        let state = AppState::from_config(TuiConfig::default());
        let area = Rect::new(32, 20, 240, 1);
        let detail = detail(&state, "CRDO", 0, area.width);
        let expected = [
            (ActionId::ToggleFocusedZoom, "z zoom"),
            (ActionId::CloseFocusedPanel, "x close"),
            (ActionId::RestorePanels, "0 restore"),
        ];

        assert_action_spans(&state, area, &detail, expected);
    }

    #[test]
    fn hidden_actions_are_not_clickable_after_width_fallback() {
        let state = AppState::from_config(TuiConfig::default());
        let area = Rect::new(80, 20, 28, 1);
        let detail = detail(&state, "CRDO", 0, area.width);

        assert!(detail.actions.is_empty());
        for column in area.x..area.right() {
            assert_eq!(action_at(&state, "CRDO", 0, area, column), None);
        }
    }

    #[test]
    fn action_spans_use_terminal_cells_when_prefix_contains_wide_text() {
        let state = AppState::from_config(TuiConfig {
            locale: LocaleConfig {
                current: Some(LocaleId::ZhCn),
            },
            ..TuiConfig::default()
        });
        let area = Rect::new(32, 20, 150, 1);
        let detail = detail(&state, "CRDO", 0, area.width);

        assert!(!detail.text.is_ascii());
        assert_action_spans(
            &state,
            area,
            &detail,
            [
                (
                    ActionId::OpenFloating(FloatingKind::CommandPalette),
                    ": command",
                ),
                (
                    ActionId::OpenFloating(FloatingKind::SymbolSearch),
                    "/ search",
                ),
            ],
        );
    }

    #[test]
    fn status_areas_share_layout_between_render_and_hit_test() {
        let area = Rect::new(4, 10, 120, 1);
        let areas = areas(area, LocaleId::EnUs);

        assert_eq!(areas.tabs.x, area.x);
        assert_eq!(areas.detail.x, areas.tabs.right());
        assert_eq!(areas.tabs.width + areas.detail.width, area.width);
    }

    fn assert_action_spans<const N: usize>(
        state: &AppState,
        area: Rect,
        detail: &StatusDetail,
        expected: [(ActionId, &str); N],
    ) {
        for (action, visible_text) in expected {
            let span = detail
                .actions
                .iter()
                .find(|span| span.action.action == action)
                .expect("status action is visible at representative width");
            assert_eq!(&detail.text[span.byte_start..span.byte_end], visible_text);
            assert_eq!(
                UnicodeWidthStr::width(&detail.text[..span.byte_start]),
                span.start as usize
            );
            assert_eq!(
                action_at(state, "CRDO", 0, area, area.x + span.start),
                Some(span.action)
            );
            assert_eq!(
                action_at(state, "CRDO", 0, area, area.x + span.end - 1),
                Some(span.action)
            );
        }
    }
}
