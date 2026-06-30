use ratatui::Frame;
use ratatui::widgets::Clear;

use crate::layout;
use crate::state::AppState;

mod account;
mod chrome;
mod futures_state;
mod history;
mod history_annotations;
mod history_glyphs;
mod intent_review;
pub(crate) mod open_orders;
mod order_ticket;
mod panels;
pub(crate) mod profile_policy;
mod profile_risk;
mod provider_health;
pub(crate) mod risk_audit;
mod settings;
mod ticket_panel;
mod transfer_ticket;
pub(crate) mod widgets;

use chrome::{render_floating, render_status};
use panels::render_docked;

pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    let layout = layout::build(
        frame.area(),
        &state.layout,
        &state.floating,
        &state.visible_panels(),
    );
    let mouse_target = state
        .mouse_position
        .and_then(|position| crate::mouse_target::target_at(state, &layout, position));
    render_docked(frame, state, &layout, mouse_target);
    render_status(frame, state, layout.status, mouse_target);
    for floating in &layout.floating {
        frame.render_widget(Clear, floating.rect);
        render_floating(frame, state, floating.kind, floating.rect, mouse_target);
    }
}

#[cfg(test)]
mod tests;
