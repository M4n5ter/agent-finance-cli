use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Shadow, Tabs, Wrap};

use crate::model::{FloatingKind, WorkspaceKind};
use crate::state::AppState;

pub(super) fn render_status(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    if area.is_empty() {
        return;
    }

    let tab_width = workspace_tabs_width().min(area.width);
    let [tabs_area, detail_area] =
        split_horizontal(area, [Constraint::Length(tab_width), Constraint::Min(0)]);
    let tabs = Tabs::new(
        WorkspaceKind::ALL
            .iter()
            .map(|workspace| workspace.title())
            .collect::<Vec<_>>(),
    )
    .select(workspace_index(state.workspace))
    .style(Style::default().bg(Color::DarkGray).fg(Color::Gray))
    .highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .divider("|")
    .padding(" ", " ");
    frame.render_widget(tabs, tabs_area);

    if detail_area.is_empty() {
        return;
    }

    let symbol = state.selected_symbol().unwrap_or("N/A");
    let errors = state
        .market_snapshot
        .as_ref()
        .map(|snapshot| snapshot.errors.len())
        .unwrap_or(0);
    let text = status_detail(state, symbol, errors, detail_area.width);
    frame.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        detail_area,
    );
}

pub(super) fn render_floating(
    frame: &mut Frame<'_>,
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
) {
    if kind == FloatingKind::CommandPalette {
        render_command_palette(frame, state, area);
        return;
    }

    let text = match kind {
        FloatingKind::CommandPalette => unreachable!("command palette is rendered separately"),
        FloatingKind::Help => vec![
            Line::from("agent-finance cockpit"),
            Line::from("[/]: switch workspace"),
            Line::from("Tab/Shift-Tab: move pane focus"),
            Line::from("z: zoom focused pane or restore workspace layout"),
            Line::from("j/k or arrows: switch selected symbol"),
            Line::from("1-6: focus watchlist, quote, history, evidence, Polymarket, research"),
            Line::from(": open command palette"),
            Line::from("Enter: execute selected command in command palette"),
            Line::from("p inspect provider details"),
            Line::from("x close focused panel"),
            Line::from("0 restore all panels"),
            Line::from("r reset layout"),
            Line::from("mouse: focus panels, drag docked borders, resize floating corners"),
            Line::from("q quit"),
        ],
        FloatingKind::ProviderDetails => state
            .provider_profiles
            .iter()
            .take(12)
            .map(|profile| {
                Line::from(format!(
                    "{}: {}",
                    profile.provider,
                    profile
                        .capabilities
                        .iter()
                        .filter(|capability| capability.implemented)
                        .map(|capability| format!("{}:{}", capability.module, capability.status))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
            .collect(),
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(floating_block(kind.title()))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_command_palette(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    if area.height < 4 {
        frame.render_widget(
            Paragraph::new("Command Palette").block(floating_block("Command Palette")),
            area,
        );
        return;
    }

    let [input_area, list_area] = split_vertical(area, [Constraint::Length(3), Constraint::Min(0)]);
    let query = state.command_palette.query();
    let input = if query.is_empty() {
        "filter commands".to_string()
    } else {
        query.to_string()
    };
    frame.render_widget(
        Paragraph::new(input)
            .style(if query.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Cyan)
            })
            .block(floating_block(
                "Command Palette  type filter  Enter run  Esc close",
            )),
        input_area,
    );

    let selected = state.command_palette.selected();
    let total = state.command_palette.len();
    let visible = command_window(total, selected, list_area.height.saturating_sub(2) as usize);
    let hidden_before = visible.start > 0;
    let hidden_after = visible.end < total;
    let visible_start = visible.start;
    let items = visible
        .enumerate()
        .filter_map(|(offset, index)| {
            let command = state.command_palette.command_at(index)?;
            let index = visible_start + offset;
            let is_selected = index == selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(command.title, style),
                Span::styled(" - ", style),
                Span::styled(command.description, style),
            ])))
        })
        .collect::<Vec<_>>();

    let title = match (total, hidden_before, hidden_after) {
        (0, _, _) => "0 matches",
        (_, true, true) => "matches  more above/below",
        (_, true, false) => "matches  more above",
        (_, false, true) => "matches  more below",
        (_, false, false) => "matches",
    };
    let items = if items.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        items
    };
    frame.render_widget(List::new(items).block(floating_block(title)), list_area);
}

fn status_detail(state: &AppState, symbol: &str, errors: usize, width: u16) -> String {
    let runtime = if state.scheduler_error.is_some() {
        "scheduler error"
    } else if state.refresh.loading {
        "refreshing"
    } else {
        "ready"
    };

    if width < 42 {
        return format!(
            " {symbol} {} {runtime} e:{errors} ",
            state.interaction_mode().label()
        );
    }

    if width < 82 {
        return format!(
            " {symbol} | mode: {} | focus: {} | {runtime} | e:{errors} ",
            state.interaction_mode().label(),
            state.panels.focused().title(),
        );
    }

    format!(
        " {symbol} | mode: {} | focus: {} | visible: {}/{} | {runtime} | errors: {errors} | {} ",
        state.interaction_mode().label(),
        state.panels.focused().title(),
        state.visible_panels().len(),
        state.workspace.panels().len(),
        "[/] workspace  Tab pane  z zoom  : command  h help  x close  0 restore  q quit"
    )
}

fn workspace_index(current: WorkspaceKind) -> usize {
    WorkspaceKind::ALL
        .iter()
        .position(|workspace| *workspace == current)
        .unwrap_or(0)
}

fn workspace_tabs_width() -> u16 {
    let titles = WorkspaceKind::ALL
        .iter()
        .map(|workspace| workspace.title().len() as u16)
        .sum::<u16>();
    let padding = WorkspaceKind::ALL.len() as u16 * 2;
    let dividers = WorkspaceKind::ALL.len().saturating_sub(1) as u16;
    titles + padding + dividers
}

fn command_window(total: usize, selected: usize, capacity: usize) -> Range<usize> {
    if total == 0 || capacity == 0 {
        return 0..0;
    }

    let selected = selected.min(total - 1);
    let capacity = capacity.min(total);
    let start = selected.saturating_add(1).saturating_sub(capacity);
    let end = (start + capacity).min(total);
    start..end
}

fn floating_block(title: &'static str) -> Block<'static> {
    simple_block(title).shadow(
        Shadow::dark_shade()
            .style(Style::default().fg(Color::Black).bg(Color::DarkGray))
            .offset(Offset::new(1, 1)),
    )
}

fn simple_block(title: &'static str) -> Block<'static> {
    Block::default().title(title).borders(Borders::ALL)
}

fn split_horizontal<const N: usize>(area: Rect, constraints: [Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
        .as_ref()
        .try_into()
        .unwrap_or([Rect::default(); N])
}

fn split_vertical<const N: usize>(area: Rect, constraints: [Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
        .as_ref()
        .try_into()
        .unwrap_or([Rect::default(); N])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_window_keeps_selected_command_visible() {
        assert_eq!(command_window(11, 0, 7), 0..7);
        assert_eq!(command_window(11, 6, 7), 0..7);
        assert_eq!(command_window(11, 10, 7), 4..11);
        assert_eq!(command_window(11, 10, 0), 0..0);
    }

    #[test]
    fn workspace_tabs_width_tracks_workspace_titles() {
        let title_width = WorkspaceKind::ALL
            .iter()
            .map(|workspace| workspace.title().len() as u16)
            .sum::<u16>();

        assert!(workspace_tabs_width() > title_width);
        assert!(workspace_tabs_width() < 80);
    }
}
