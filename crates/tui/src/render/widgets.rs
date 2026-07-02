use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};

use crate::i18n::TuiText;
use crate::model::Panel;
use crate::pane_status::{TuiPaneStatus, pane_health};
use crate::state::AppState;

pub(crate) fn compact_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut output = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

pub(crate) fn format_price(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.4}")
    }
}

pub(crate) fn format_volume(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

pub(super) fn panel_block(panel: Panel, state: &AppState) -> Block<'static> {
    let status = pane_health(state, panel).status;
    let text = TuiText::new(state.locale);
    let style = if state.panels.focused() == panel {
        state.theme.accent_style()
    } else {
        status_style(state, status)
    };
    let title = format!("{} [{}]", text.panel_title(panel), text.pane_status(status));
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style)
}

fn status_style(state: &AppState, status: TuiPaneStatus) -> Style {
    match status {
        TuiPaneStatus::Fresh => state.theme.success_style(),
        TuiPaneStatus::Loading => state.theme.warning_style(),
        TuiPaneStatus::Partial | TuiPaneStatus::Empty | TuiPaneStatus::Stale => {
            state.theme.neutral_style()
        }
        TuiPaneStatus::Error => state.theme.danger_style(),
    }
}
