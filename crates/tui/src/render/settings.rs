use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::model::Panel;
use crate::state::AppState;

use super::widgets::panel_block;

pub(super) fn render_settings(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let dirty = if state.config_changes.is_empty() {
        "clean".to_string()
    } else {
        state.config_changes.join(", ")
    };
    let profile = state.trading_profile.as_deref().unwrap_or("-");
    let mut lines = vec![
        Line::from(Span::styled(
            "configuration cockpit",
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("workspace: {}", state.workspace)),
        Line::from(format!("dirty config: {dirty}")),
        Line::from(format!(
            "watchlist: {} symbols  selected={}",
            state.watchlist.len(),
            state.selected_symbol().unwrap_or("-")
        )),
        Line::from(format!(
            "trading profile: {profile}  live writes={}",
            if state.live_writes_enabled {
                "on"
            } else {
                "off"
            }
        )),
        Line::from(format!(
            "default submit mode: {}  effective={}",
            state.default_submit_mode,
            state.effective_submit_mode()
        )),
        Line::from(format!(
            "provider profiles: {}",
            state.provider_profiles.len()
        )),
        Line::from(format!(
            "theme: configured  normal key bindings: {}",
            state.keymap.normal_len()
        )),
        Line::from(""),
        Line::from("available editors"),
        Line::from(": command palette  a add symbols  d delete symbol  left/right reorder"),
        Line::from("profile: command palette -> Set trading profile"),
        Line::from("save: command palette -> Save config"),
    ];
    for change in state.config_changes.iter().take(3) {
        lines.push(Line::from(Span::styled(
            format!("pending: {change}"),
            state.theme.warning_style(),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::Settings, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}
