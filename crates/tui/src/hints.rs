use crate::command::ActionId;
use crate::model::{FloatingKind, InteractionMode, Panel};
use crate::state::AppState;

pub fn mode_key_hints(state: &AppState) -> Vec<String> {
    if state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::LiveWritesConfirmation)
    {
        return vec![
            "enter enable live writes".to_string(),
            "esc close".to_string(),
        ];
    }
    if state
        .floating
        .last()
        .is_some_and(|pane| pane.kind == FloatingKind::StagedExecutionConfirmation)
    {
        return vec!["enter confirm".to_string(), "esc cancel".to_string()];
    }

    if let Some(spec) = active_input_mode_spec(state) {
        return spec.hints();
    }

    match state.interaction_mode() {
        InteractionMode::Normal if state.panels.focused() == Panel::IntentReview => {
            intent_review_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::OrderTicket => {
            crate::order_ticket_controls::order_ticket_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::OpenOrders => {
            crate::open_order_controls::open_order_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::Account => {
            crate::account_controls::account_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::TransferTicket => {
            crate::transfer_ticket_controls::transfer_ticket_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::FuturesState => {
            crate::futures_state_controls::futures_state_key_hints()
        }
        InteractionMode::Normal if state.panels.focused() == Panel::Settings => {
            crate::settings_controls::settings_key_hints()
        }
        InteractionMode::Normal => normal_key_hints(state),
        InteractionMode::Command | InteractionMode::Search => Vec::new(),
        InteractionMode::Help | InteractionMode::Inspect => vec![
            hint_for(state, ActionId::CloseFocusedFloating, "close")
                .unwrap_or_else(|| "esc close".to_string()),
            "q quit".to_string(),
        ],
    }
}

fn intent_review_key_hints() -> Vec<String> {
    intent_review_control_hints()
        .iter()
        .map(|hint| (*hint).to_string())
        .collect()
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

pub fn status_key_hints(state: &AppState, max_width: usize) -> String {
    let mut hints = mode_key_hints(state);
    while !hints.is_empty() {
        let text = hints.join("  ");
        if text.len() <= max_width {
            return text;
        }
        hints.pop();
    }
    String::new()
}

fn normal_key_hints(state: &AppState) -> Vec<String> {
    [
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
        hint_for(state, ActionId::ToggleFocusedZoom, "zoom"),
        hint_for(
            state,
            ActionId::OpenFloating(FloatingKind::CommandPalette),
            "command",
        ),
        hint_for(
            state,
            ActionId::OpenFloating(FloatingKind::SymbolSearch),
            "search",
        ),
        hint_for(state, ActionId::OpenFloating(FloatingKind::Help), "help"),
        hint_for(state, ActionId::CloseFocusedPanel, "close"),
        hint_for(state, ActionId::RestorePanels, "restore"),
        Some("q quit".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn hint_for(state: &AppState, action: ActionId, label: &'static str) -> Option<String> {
    state
        .keymap
        .normal_key_for(action)
        .map(|key| format!("{key} {label}"))
}

fn pair_hint(
    state: &AppState,
    previous: ActionId,
    next: ActionId,
    label: &'static str,
) -> Option<String> {
    match (
        state.keymap.normal_key_for(previous),
        state.keymap.normal_key_for(next),
    ) {
        (Some(previous), Some(next)) => Some(format!("{previous}/{next} {label}")),
        (None, Some(next)) => Some(format!("{next} {label}")),
        (Some(previous), None) => Some(format!("{previous} {label}")),
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

        let hints = status_key_hints(&state, 20);

        assert!(hints.len() <= 20);
        assert!(hints.contains("workspace"));
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
                "e profile",
                "v validate",
                "t risk",
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
