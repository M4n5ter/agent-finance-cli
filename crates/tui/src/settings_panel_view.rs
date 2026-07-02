use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::action_line_view::{ActionLine, ActionSpan};
use crate::command::ActionId;
use crate::i18n::TuiText;
use crate::model::{FloatingKind, Panel};
use crate::mouse_target::MouseTarget;
use crate::panel_action_line_view::{
    PanelActionLine, PanelActionSpan, RenderedPanelActionLine, panel_action_span_at,
    render_panel_action_line,
};
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
    pub panel_actions: Vec<PanelActionSpan>,
}

impl SettingsPanelRow {
    fn text(text: impl Into<String>) -> Self {
        Self {
            line: Line::from(text.into()),
            setting_index: None,
            actions: Vec::new(),
            panel_actions: Vec::new(),
        }
    }

    fn line(line: Line<'static>) -> Self {
        Self {
            line,
            setting_index: None,
            actions: Vec::new(),
            panel_actions: Vec::new(),
        }
    }

    fn setting(line: Line<'static>, index: usize, actions: Vec<SettingActionSpan>) -> Self {
        Self {
            line,
            setting_index: Some(index),
            actions,
            panel_actions: Vec::new(),
        }
    }

    fn panel_action(rendered: RenderedPanelActionLine) -> Self {
        Self {
            line: rendered.line,
            setting_index: None,
            actions: Vec::new(),
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
) -> Vec<SettingsPanelRow> {
    let text = SettingsText::new(state.locale);
    let dirty = if state.config_changes.is_empty() {
        text.t("tui-settings-clean")
    } else {
        state.config_changes.join(", ")
    };
    let profile = state.trading_profile.as_deref().unwrap_or("-");
    let workspace = state.workspace.to_string();
    let watchlist_count = state.watchlist.len().to_string();
    let default_submit_mode = state.default_submit_mode.to_string();
    let effective_submit_mode = state.effective_submit_mode().to_string();
    let equity_provider = state.providers.equity.to_string();
    let crypto_provider = state.providers.crypto.to_string();
    let theme_accent = state.theme.accent.to_string();
    let theme_selection_background = state.theme.selection_background.to_string();
    let theme_selection_foreground = state.theme.selection_foreground.to_string();
    let provider_profile_count = state.provider_profiles.len().to_string();
    let normal_key_count = state.keymap.normal_len().to_string();
    let mut rows = vec![
        SettingsPanelRow::line(Line::from(Span::styled(
            text.t("tui-settings-title"),
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ))),
        SettingsPanelRow::text(text.f("tui-settings-workspace", &[("workspace", &workspace)])),
        SettingsPanelRow::text(text.f(
            "tui-settings-language-summary",
            &[
                ("language", state.locale.display_name()),
                ("locale", state.locale.as_str()),
            ],
        )),
        SettingsPanelRow::text(text.f("tui-settings-dirty-config", &[("dirty", &dirty)])),
        SettingsPanelRow::text(text.f(
            "tui-settings-watchlist",
            &[
                ("count", &watchlist_count),
                ("selected", state.selected_symbol().unwrap_or("-")),
            ],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-trading-profile",
            &[
                ("profile", profile),
                (
                    "liveWrites",
                    if state.live_writes_enabled {
                        "on"
                    } else {
                        "off"
                    },
                ),
            ],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-submit-mode",
            &[
                ("default", &default_submit_mode),
                ("effective", &effective_submit_mode),
            ],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-provider-preferences",
            &[("equity", &equity_provider), ("crypto", &crypto_provider)],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-theme",
            &[
                ("accent", &theme_accent),
                ("selectionBackground", &theme_selection_background),
                ("selectionForeground", &theme_selection_foreground),
            ],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-provider-capability-profiles",
            &[("count", &provider_profile_count)],
        )),
        SettingsPanelRow::text(text.f(
            "tui-settings-normal-key-bindings",
            &[("count", &normal_key_count)],
        )),
        SettingsPanelRow::text(""),
        SettingsPanelRow::text(text.t("tui-settings-editor-heading")),
    ];
    rows.extend(settings_action_rows(state, width, mouse_target, &text));
    rows.extend(setting_rows(state, width, mouse_target, &text));
    rows.extend([
        SettingsPanelRow::text(""),
        SettingsPanelRow::text(crate::settings_controls::settings_panel_hint()),
        SettingsPanelRow::text(""),
    ]);
    rows.extend(state.config_changes.iter().take(3).map(|change| {
        SettingsPanelRow::line(Line::from(Span::styled(
            text.f("tui-settings-pending", &[("change", change)]),
            state.theme.warning_style(),
        )))
    }));
    rows
}

type SettingsText = TuiText;

pub(crate) fn setting_index_at_content_row(
    state: &AppState,
    width: u16,
    content_row: usize,
) -> Option<usize> {
    rows(state, width, None).get(content_row)?.setting_index
}

pub(crate) fn panel_action_at_content_cell(
    state: &AppState,
    width: u16,
    content_row: usize,
    content_column: u16,
) -> Option<PanelActionSpan> {
    rows(state, width, None)
        .get(content_row)?
        .panel_action_at(content_column)
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
        .find(|span| (span.start..span.end).contains(&content_column))
        .cloned()
}

fn settings_action_rows(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
    text: &SettingsText,
) -> Vec<SettingsPanelRow> {
    vec![settings_action_row(
        state,
        width,
        mouse_target,
        &text.t("tui-settings-actions"),
        &[
            (
                text.t("tui-settings-action-add-symbols"),
                ActionId::OpenFloating(FloatingKind::WatchlistAdd),
            ),
            (
                text.t("tui-settings-action-set-profile"),
                ActionId::OpenFloating(FloatingKind::TradingProfile),
            ),
            (
                text.t("tui-settings-action-profile-risk"),
                ActionId::FocusPanel(Panel::ProfileRisk),
            ),
            (text.t("tui-settings-action-save"), ActionId::SaveConfig),
            (
                text.t("tui-settings-action-undo"),
                ActionId::UndoConfigChange,
            ),
            (
                text.t("tui-settings-action-providers"),
                ActionId::OpenFloating(FloatingKind::ProviderDetails),
            ),
            (
                text.t("tui-settings-action-allow-live"),
                ActionId::StageProfileLiveToggle,
            ),
        ],
    )]
}

fn settings_action_row(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
    label: &str,
    actions: &[(String, ActionId)],
) -> SettingsPanelRow {
    let mut action_line = PanelActionLine::new(label, width);
    for (label, action) in actions {
        action_line.push_visible_text("  ");
        action_line.push_visible_action(label.as_str(), *action);
    }
    SettingsPanelRow::panel_action(render_panel_action_line(
        &action_line,
        &state.theme,
        Panel::Settings,
        mouse_target,
    ))
}

fn setting_rows(
    state: &AppState,
    width: u16,
    mouse_target: Option<MouseTarget>,
    text: &SettingsText,
) -> Vec<SettingsPanelRow> {
    SettingRow::ALL
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let selected = state.settings_editor.selected() == row;
            let hovered =
                mouse_target.is_some_and(|target| target.panel_row_hovered(Panel::Settings, index));
            let marker = if selected { ">" } else { " " };
            let value = row.value(&state.locale, &state.providers, &state.theme, &state.keymap);
            let label = text.setting_label(row);
            let action_line = setting_action_line_for(index, marker, &label, &value, width, text);
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
    _text: &SettingsText,
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
        push_text_span(&mut spans, action_line.action_text(action), action_style);
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
    fn rows_localize_static_settings_text() {
        let mut config = crate::config::TuiConfig::default();
        config.locale.current = Some(agent_finance_i18n::LocaleId::ZhCn);
        let state = AppState::from_config(config);
        let text = rows(&state, 120, None)
            .into_iter()
            .map(|row| row.line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("配置驾驶舱"));
        assert!(text.contains("工作区："));
        assert!(text.contains("> 语言: 简体中文"));
    }

    #[test]
    fn rows_expose_panel_driven_settings_actions() {
        let state = AppState::from_config(crate::config::TuiConfig::default());

        let actions = rows(&state, 140, None)
            .into_iter()
            .flat_map(|row| row.panel_actions)
            .map(|span| span.action)
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                ActionId::OpenFloating(FloatingKind::WatchlistAdd),
                ActionId::OpenFloating(FloatingKind::TradingProfile),
                ActionId::FocusPanel(Panel::ProfileRisk),
                ActionId::SaveConfig,
                ActionId::UndoConfigChange,
                ActionId::OpenFloating(FloatingKind::ProviderDetails),
                ActionId::StageProfileLiveToggle,
            ]
        );
    }

    #[test]
    fn panel_action_at_content_cell_maps_rendered_settings_action() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let rendered_rows = rows(&state, 140, None);
        let (content_row, span) = rendered_rows
            .iter()
            .enumerate()
            .find_map(|(content_row, row)| {
                row.panel_actions
                    .iter()
                    .find(|span| span.action == ActionId::OpenFloating(FloatingKind::WatchlistAdd))
                    .map(|span| (content_row, span.clone()))
            })
            .expect("watchlist add action is rendered");

        let action = panel_action_at_content_cell(&state, 140, content_row, span.start)
            .expect("watchlist add action is clickable");

        assert_eq!(action, span);
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
