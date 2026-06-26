use crate::command::ActionId;
use crate::model::{FloatingKind, InteractionMode};
use crate::state::AppState;

pub fn mode_key_hints(state: &AppState) -> Vec<String> {
    match state.interaction_mode() {
        InteractionMode::Normal => normal_key_hints(state),
        InteractionMode::Command | InteractionMode::Search => {
            input_mode_spec(state.interaction_mode()).hints()
        }
        InteractionMode::Help | InteractionMode::Inspect => vec![
            hint_for(state, ActionId::CloseFocusedFloating, "close")
                .unwrap_or_else(|| "esc close".to_string()),
            "q quit".to_string(),
        ],
    }
}

pub fn input_floating_title(mode: InteractionMode) -> Option<String> {
    let spec = input_mode_spec(mode);
    spec.valid().then(|| {
        let hints = spec.hints().join("  ");
        format!("{}  {}", spec.title, hints)
    })
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
    accept: &'static str,
}

impl InputModeSpec {
    const fn valid(&self) -> bool {
        !self.title.is_empty()
    }

    fn hints(&self) -> Vec<String> {
        vec![
            "type filter".to_string(),
            format!("enter {}", self.accept),
            "up/down move".to_string(),
            "esc close".to_string(),
        ]
    }
}

fn input_mode_spec(mode: InteractionMode) -> InputModeSpec {
    match mode {
        InteractionMode::Command => InputModeSpec {
            title: "Command Palette",
            accept: "run",
        },
        InteractionMode::Search => InputModeSpec {
            title: "Symbol Search",
            accept: "select",
        },
        InteractionMode::Normal | InteractionMode::Help | InteractionMode::Inspect => {
            InputModeSpec {
                title: "",
                accept: "",
            }
        }
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
    }

    #[test]
    fn status_hints_fit_width_by_dropping_low_priority_items() {
        let state = AppState::from_config(TuiConfig::default());

        let hints = status_key_hints(&state, 20);

        assert!(hints.len() <= 20);
        assert!(hints.contains("workspace"));
    }
}
