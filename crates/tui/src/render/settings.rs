use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem};

use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::settings_panel_view;
use crate::state::AppState;

use super::widgets::panel_block;

pub(super) fn render_settings(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let items = settings_panel_view::rows(state, area.width.saturating_sub(2), mouse_target)
        .into_iter()
        .map(|row| ListItem::new(row.line));
    frame.render_widget(
        List::new(items).block(panel_block(Panel::Settings, state)),
        area,
    );
}
