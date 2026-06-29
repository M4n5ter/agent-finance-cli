use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::action_line_view::{ActionLine, ActionSpan};
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::settings_editor::SettingRow;
use crate::state::AppState;

pub(crate) type SettingActionLine = ActionLine<SettingRowAction>;
pub(crate) type SettingActionSpan = ActionSpan<SettingRowAction>;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct SettingRowAction {
    pub index: usize,
    pub direction: isize,
}

pub(crate) struct SettingsPanelRow {
    pub line: Line<'static>,
    pub setting_index: Option<usize>,
    pub actions: Vec<SettingActionSpan>,
}

impl SettingsPanelRow {
    fn text(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            setting_index: None,
            actions: Vec::new(),
        }
    }

    fn line(line: Line<'static>) -> Self {
        Self {
            line,
            setting_index: None,
            actions: Vec::new(),
        }
    }

    fn setting(line: Line<'static>, index: usize, actions: Vec<SettingActionSpan>) -> Self {
        Self {
            line,
            setting_index: Some(index),
            actions,
        }
    }
}

pub(crate) fn rows(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Vec<SettingsPanelRow> {
    let dirty = if state.config_changes.is_empty() {
        "clean".to_string()
    } else {
        state.config_changes.join(", ")
    };
    let profile = state.trading_profile.as_deref().unwrap_or("-");
    let mut rows = vec![
        SettingsPanelRow::line(Line::from(Span::styled(
            "configuration cockpit",
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ))),
        SettingsPanelRow::text(format!("workspace: {}", state.workspace)),
        SettingsPanelRow::text(format!("dirty config: {dirty}")),
        SettingsPanelRow::text(format!(
            "watchlist: {} symbols  selected={}",
            state.watchlist.len(),
            state.selected_symbol().unwrap_or("-")
        )),
        SettingsPanelRow::text(format!(
            "trading profile: {profile}  live writes={}",
            if state.live_writes_enabled {
                "on"
            } else {
                "off"
            }
        )),
        SettingsPanelRow::text(format!(
            "default submit mode: {}  effective={}",
            state.default_submit_mode,
            state.effective_submit_mode()
        )),
        SettingsPanelRow::text(format!(
            "provider preferences: equity={}  crypto={}",
            state.providers.equity, state.providers.crypto
        )),
        SettingsPanelRow::text(format!(
            "theme: accent={}  selection={}/{}",
            state.theme.accent, state.theme.selection_background, state.theme.selection_foreground
        )),
        SettingsPanelRow::text(format!(
            "provider capability profiles: {}",
            state.provider_profiles.len()
        )),
        SettingsPanelRow::text(format!(
            "normal key bindings: {}",
            state.keymap.normal_len()
        )),
        SettingsPanelRow::text(""),
        SettingsPanelRow::text("settings editor"),
    ];
    rows.extend(setting_rows(state, width, mouse_target));
    rows.extend([
        SettingsPanelRow::text(""),
        SettingsPanelRow::text(crate::settings_controls::settings_panel_hint()),
        SettingsPanelRow::text(""),
        SettingsPanelRow::text("available editors"),
        SettingsPanelRow::text(
            ": command palette  a add symbols  d delete symbol  watchlist left/right reorder",
        ),
        SettingsPanelRow::text(
            "profile/risk: use the Profile / Risk panel for validation and risk changes",
        ),
        SettingsPanelRow::text("save/undo: command palette -> Save config / Undo config change"),
    ]);
    rows.extend(state.config_changes.iter().take(3).map(|change| {
        SettingsPanelRow::line(Line::from(Span::styled(
            format!("pending: {change}"),
            state.theme.warning_style(),
        )))
    }));
    rows
}

pub(crate) fn setting_index_at_content_row(
    state: &AppState,
    width: u16,
    content_row: usize,
) -> Option<usize> {
    rows(state, width, None).get(content_row)?.setting_index
}

pub(crate) fn action_at_content_cell(
    state: &AppState,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<SettingActionSpan> {
    rows(state, width, None)
        .get(content_row)?
        .actions
        .iter()
        .copied()
        .find(|span| (span.start..span.end).contains(&content_column))
}

fn setting_rows(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
) -> Vec<SettingsPanelRow> {
    SettingRow::ALL
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let selected = state.settings_editor.selected() == row;
            let hovered =
                mouse_target.is_some_and(|target| target.panel_row_hovered(Panel::Settings, index));
            let marker = if selected { ">" } else { " " };
            let value = row.value(&state.providers, &state.theme, &state.keymap);
            let action_line = setting_action_line_for(index, marker, row.label(), &value, width);
            let actions = action_line.actions.clone();
            let line = styled_setting_action_line(
                &action_line,
                state,
                index,
                selected,
                hovered,
                mouse_target,
            );
            SettingsPanelRow::setting(line, index, actions)
        })
        .collect()
}

fn setting_action_line_for(
    index: usize,
    marker: &str,
    label: &str,
    value: &str,
    width: u16,
) -> SettingActionLine {
    let mut line = SettingActionLine::new(format!("{marker} {label}: {value}  "), width);
    line.push_visible_action(
        "[prev]",
        SettingRowAction {
            index,
            direction: -1,
        },
    );
    line.push_visible_text(" ");
    line.push_visible_action(
        "[next]",
        SettingRowAction {
            index,
            direction: 1,
        },
    );
    line
}

fn styled_setting_action_line(
    action_line: &SettingActionLine,
    state: &AppState,
    index: usize,
    selected: bool,
    hovered: bool,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    let text_style = if hovered {
        state.theme.selected_style().add_modifier(Modifier::BOLD)
    } else if selected {
        state.theme.accent_style().add_modifier(Modifier::BOLD)
    } else {
        state.theme.text_style()
    };
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for action in &action_line.actions {
        push_text_span(
            &mut spans,
            action_line.text_before(action.byte_start, cursor),
            text_style,
        );
        let action_hovered = mouse_target.is_some_and(|target| {
            target.panel_setting_adjust_hovered(Panel::Settings, index, action.action.direction)
        });
        let action_style = if action_hovered {
            state.theme.selected_style().add_modifier(Modifier::BOLD)
        } else {
            state.theme.accent_style()
        };
        push_text_span(&mut spans, action_line.action_text(*action), action_style);
        cursor = action.byte_end;
    }
    push_text_span(&mut spans, action_line.text_after(cursor), text_style);
    Line::from(spans)
}

fn push_text_span(spans: &mut Vec<Span<'static>>, text: &str, style: ratatui::style::Style) {
    if !text.is_empty() {
        spans.push(Span::styled(text.to_string(), style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_mark_all_rendered_settings_as_clickable_metadata() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        let clickable = rows(&state, 120, None)
            .into_iter()
            .filter_map(|row| row.setting_index)
            .collect::<Vec<_>>();

        assert_eq!(clickable, (0..SettingRow::ALL.len()).collect::<Vec<_>>());
    }

    #[test]
    fn action_at_content_cell_maps_panel_row_to_setting_action() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let rendered_rows = rows(&state, 120, None);
        let (content_row, row) = rendered_rows
            .iter()
            .enumerate()
            .find(|(_, row)| row.setting_index == Some(0))
            .expect("first setting row is rendered");
        let next_column = row
            .actions
            .iter()
            .find(|span| span.label == "[next]")
            .map(|span| span.start)
            .expect("next action is rendered");

        let action = action_at_content_cell(&state, 120, content_row, next_column)
            .expect("next action is clickable");

        assert_eq!(
            action.action,
            SettingRowAction {
                index: 0,
                direction: 1
            }
        );
    }

    #[test]
    fn narrow_rows_do_not_expose_hidden_setting_actions() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        let actions = rows(&state, 8, None)
            .into_iter()
            .filter(|row| row.setting_index.is_some())
            .flat_map(|row| row.actions)
            .collect::<Vec<_>>();

        assert!(actions.is_empty());
    }
}
