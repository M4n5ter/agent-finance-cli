use crate::command::ActionId;
use crate::model::{FloatingKind, InteractionMode, Panel};
use crate::state::AppState;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StatusHint {
    pub text: String,
    pub action: Option<StatusHintAction>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct StatusHintAction {
    pub action: ActionId,
    pub mouse_label: &'static str,
}

pub fn mode_key_hints(state: &AppState) -> Vec<String> {
    mode_status_hints(state)
        .into_iter()
        .map(|hint| hint.text)
        .collect()
}

fn mode_status_hints(state: &AppState) -> Vec<StatusHint> {
    if state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::LiveWritesConfirmation)
    {
        return text_only_hints(["enter enable live writes", "esc close"]);
    }
    if state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::StagedExecutionConfirmation)
    {
        if let Some(gate) = state.pending_staged_confirmation_gate()
            && !gate.matched
        {
            return text_only_hints([
                format!("type {}", gate.phrase),
                "enter check".to_string(),
                "esc cancel".to_string(),
            ]);
        }
        return text_only_hints(["enter confirm", "esc cancel"]);
    }

    if let Some(spec) = active_input_mode_spec(state) {
        return text_only_hints(spec.hints.iter().copied());
    }

    match state.interaction_mode() {
        InteractionMode::Normal if state.panels.focused() == Panel::IntentReview => {
            text_only_hints(intent_review_control_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::OrderTicket => {
            text_only_hints(crate::order_ticket_controls::order_ticket_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::OpenOrders => {
            text_only_hints(crate::open_order_controls::open_order_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::Account => {
            text_only_hints(crate::account_controls::account_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::TransferTicket => {
            text_only_hints(crate::transfer_ticket_controls::transfer_ticket_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::FuturesState => {
            text_only_hints(crate::futures_state_controls::futures_state_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::ProfileRisk => {
            text_only_hints(crate::profile_risk_controls::profile_risk_key_hints())
        }
        InteractionMode::Normal if state.panels.focused() == Panel::Settings => {
            text_only_hints(crate::settings_controls::settings_key_hints())
        }
        InteractionMode::Normal => normal_status_hints(state),
        InteractionMode::Command | InteractionMode::Search => Vec::new(),
        InteractionMode::Help | InteractionMode::Inspect => [
            hint_for(
                state,
                ActionId::CloseFocusedFloating,
                "close floating",
                "close",
            )
            .unwrap_or_else(|| StatusHint::text("esc close")),
            StatusHint::text("q quit"),
        ]
        .into_iter()
        .collect(),
    }
}

pub fn intent_review_panel_hint() -> String {
    intent_review_control_hints().join("  ")
}

fn intent_review_control_hints() -> [&'static str; 4] {
    [
        "up/down/k/j select",
        "enter submit",
        "d/backspace close",
        "q quit",
    ]
}

pub fn input_floating_title_for_kind(kind: FloatingKind) -> Option<String> {
    input_mode_spec_for_kind(kind).map(|spec| {
        let hints = spec.hints().join("  ");
        format!("{}  {}", spec.title, hints)
    })
}

fn active_input_mode_spec(state: &AppState) -> Option<InputModeSpec> {
    state
        .floating
        .last()
        .and_then(|pane| input_mode_spec_for_kind(pane.kind))
}

pub(crate) fn status_key_hint_specs(state: &AppState, max_width: usize) -> Vec<StatusHint> {
    let mut hints = mode_status_hints(state);
    while !hints.is_empty() {
        let text = hints
            .iter()
            .map(|hint| hint.text.as_str())
            .collect::<Vec<_>>()
            .join("  ");
        if text.len() <= max_width {
            return hints;
        }
        hints.pop();
    }
    Vec::new()
}

fn normal_status_hints(state: &AppState) -> Vec<StatusHint> {
    [
        hint_for(
            state,
            ActionId::OpenFloating(FloatingKind::CommandPalette),
            "open command palette",
            "command",
        ),
        hint_for(
            state,
            ActionId::OpenFloating(FloatingKind::SymbolSearch),
            "open symbol search",
            "search",
        ),
        hint_for(
            state,
            ActionId::OpenFloating(FloatingKind::Help),
            "open help",
            "help",
        ),
        pair_hint(
            state,
            ActionId::ShiftWorkspace(-1),
            ActionId::ShiftWorkspace(1),
            "workspace",
        ),
        pair_hint(
            state,
            ActionId::FocusPanelBy(-1),
            ActionId::FocusPanelBy(1),
            "pane",
        ),
        hint_for(state, ActionId::ToggleFocusedZoom, "toggle zoom", "zoom"),
        hint_for(state, ActionId::CloseFocusedPanel, "close panel", "close"),
        hint_for(state, ActionId::RestorePanels, "restore panels", "restore"),
        Some(StatusHint::text("q quit")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn hint_for(
    state: &AppState,
    action: ActionId,
    mouse_label: &'static str,
    visible_label: &'static str,
) -> Option<StatusHint> {
    state.keymap.normal_key_for(action).map(|key| StatusHint {
        text: format!("{key} {visible_label}"),
        action: Some(StatusHintAction {
            action,
            mouse_label,
        }),
    })
}

fn text_only_hints(hints: impl IntoIterator<Item = impl Into<String>>) -> Vec<StatusHint> {
    hints.into_iter().map(StatusHint::text).collect()
}

impl StatusHint {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action: None,
        }
    }
}

fn pair_hint(
    state: &AppState,
    previous: ActionId,
    next: ActionId,
    label: &'static str,
) -> Option<StatusHint> {
    match (
        state.keymap.normal_key_for(previous),
        state.keymap.normal_key_for(next),
    ) {
        (Some(previous), Some(next)) => {
            Some(StatusHint::text(format!("{previous}/{next} {label}")))
        }
        (None, Some(next)) => Some(StatusHint::text(format!("{next} {label}"))),
        (Some(previous), None) => Some(StatusHint::text(format!("{previous} {label}"))),
        (None, None) => None,
    }
}

struct InputModeSpec {
    title: &'static str,
    hints: &'static [&'static str],
}

impl InputModeSpec {
    fn hints(&self) -> Vec<String> {
        self.hints.iter().map(|hint| (*hint).to_string()).collect()
    }
}

fn input_mode_spec_for_kind(kind: FloatingKind) -> Option<InputModeSpec> {
    match kind {
        FloatingKind::CommandPalette => Some(InputModeSpec {
            title: "Command Palette",
            hints: &["type filter", "enter run", "up/down move", "esc close"],
        }),
        FloatingKind::SymbolSearch => Some(InputModeSpec {
            title: "Symbol Search",
            hints: &["type filter", "enter select", "up/down move", "esc close"],
        }),
        FloatingKind::WatchlistAdd => Some(InputModeSpec {
            title: "Add Symbols",
            hints: &["type symbols", "enter add", "esc close"],
        }),
        FloatingKind::TradingProfile => Some(InputModeSpec {
            title: "Trading Profile",
            hints: &["type profile", "enter set", "blank clears", "esc close"],
        }),
        FloatingKind::Help
        | FloatingKind::LiveWritesConfirmation
        | FloatingKind::StagedExecutionConfirmation
        | FloatingKind::ProviderDetails => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::TuiConfig;
    use crate::keymap::{KeyBinding, KeymapConfig};
    use crate::model::FloatingKind;
    use crate::state::Action;

    #[test]
    fn normal_hints_follow_configured_keymap() {
        let state = AppState::from_config(TuiConfig::default());

        let hints = mode_key_hints(&state);

        assert!(hints.iter().any(|hint| hint == "[/] workspace"));
        assert!(hints.iter().any(|hint| hint == ": command"));
        assert!(hints.iter().any(|hint| hint == "/ search"));
    }

    #[test]
    fn normal_hints_omit_actions_without_effective_bindings() {
        let state = AppState::from_config(TuiConfig {
            keymap: KeymapConfig::from_overrides(vec![KeyBinding {
                key: ":".parse().expect("key"),
                action: ActionId::OpenFloating(FloatingKind::ProviderDetails),
            }]),
            ..TuiConfig::default()
        });

        let hints = mode_key_hints(&state);

        assert!(!hints.iter().any(|hint| hint.contains("command")));
        assert!(hints.iter().any(|hint| hint == "/ search"));
    }

    #[test]
    fn text_input_modes_show_input_specific_hints() {
        let mut state = AppState::from_config(TuiConfig::default());

        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::SymbolSearch,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "type filter".to_string(),
                "enter select".to_string(),
                "up/down move".to_string(),
                "esc close".to_string(),
            ]
        );

        state.reduce(Action::Execute(ActionId::OpenFloating(
            FloatingKind::WatchlistAdd,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "type symbols".to_string(),
                "enter add".to_string(),
                "esc close".to_string(),
            ]
        );
    }

    #[test]
    fn status_hints_fit_width_by_dropping_low_priority_items() {
        let state = AppState::from_config(TuiConfig::default());

        let hints = status_key_hint_specs(&state, 20)
            .into_iter()
            .map(|hint| hint.text)
            .collect::<Vec<_>>()
            .join("  ");

        assert!(hints.len() <= 20);
        assert!(hints.contains("command"));
    }

    #[test]
    fn settings_focus_shows_provider_editor_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Settings,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "up/down select setting",
                "left/right adjust",
                "enter next value",
                "u undo",
                "q quit",
            ]
        );
    }

    #[test]
    fn account_focus_shows_open_order_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::Account,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec!["up/down open order", "c stage cancel", "q quit"]
        );
    }

    #[test]
    fn transfer_ticket_focus_shows_transfer_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::TransferTicket,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "[/] transfer field",
                "left/right transfer",
                "t stage transfer",
                "q quit",
            ]
        );
    }

    #[test]
    fn futures_state_focus_shows_futures_state_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Account,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::FuturesState,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "u futures field",
                "i futures adjust",
                "f stage state",
                "q quit",
            ]
        );
    }

    #[test]
    fn profile_risk_focus_shows_profile_risk_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Settings,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::ProfileRisk,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec!["e profile", "v validate", "t stage risk", "q quit"]
        );
    }

    #[test]
    fn open_orders_focus_shows_cancel_operation_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::OpenOrders,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec!["up/down open order", "c stage cancel", "q quit"]
        );
    }

    #[test]
    fn order_ticket_focus_shows_stage_operation_hints() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(Action::Execute(ActionId::SetWorkspace(
            crate::model::WorkspaceKind::Trade,
        )));
        state.reduce(Action::Execute(ActionId::FocusPanel(
            crate::model::Panel::OrderTicket,
        )));

        assert_eq!(
            mode_key_hints(&state),
            vec![
                "up/down field",
                "left/right adjust",
                "enter adjust",
                "s stage order",
                "q quit",
            ]
        );
    }
}
