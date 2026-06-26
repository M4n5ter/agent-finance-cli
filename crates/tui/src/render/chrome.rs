use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Shadow, Tabs, Wrap};

use crate::hints;
use crate::model::{FloatingKind, InteractionMode, WorkspaceKind};
use crate::state::AppState;
use crate::theme::ThemeConfig;

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
    .style(state.theme.chrome_style())
    .highlight_style(
        state
            .theme
            .chrome_style()
            .fg(state.theme.accent.color())
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
        Paragraph::new(text).style(state.theme.chrome_style().fg(state.theme.text.color())),
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
    if kind == FloatingKind::SymbolSearch {
        render_symbol_search(frame, state, area);
        return;
    }

    let text = match kind {
        FloatingKind::CommandPalette => unreachable!("command palette is rendered separately"),
        FloatingKind::SymbolSearch => unreachable!("symbol search is rendered separately"),
        FloatingKind::Help => vec![
            Line::from("agent-finance cockpit"),
            Line::from("[/]: switch workspace"),
            Line::from("Tab/Shift-Tab: move pane focus"),
            Line::from("z: zoom focused pane or restore workspace layout"),
            Line::from("j/k or arrows: switch selected symbol"),
            Line::from("1-6: focus watchlist, quote, history, evidence, Polymarket, research"),
            Line::from(": open command palette"),
            Line::from("/ search watchlist symbols"),
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
            .block(floating_block(kind.title(), &state.theme))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_command_palette(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Command Palette",
            input_title: hints::input_floating_title(InteractionMode::Command)
                .expect("command mode has an input title"),
            placeholder: "filter commands",
            query: state.command_palette.query(),
            selected: state.command_palette.selected(),
            total: state.command_palette.len(),
            noun: "matches",
            empty: "No matching commands",
        },
        &state.theme,
        |index, is_selected| {
            let command = state.command_palette.command_at(index)?;
            let style = if is_selected {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.text_style()
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(command.title, style),
                Span::styled(" - ", style),
                Span::styled(command.description, style),
            ])))
        },
    );
}

fn render_symbol_search(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Symbol Search",
            input_title: hints::input_floating_title(InteractionMode::Search)
                .expect("search mode has an input title"),
            placeholder: "filter symbols",
            query: state.symbol_search.query(),
            selected: state.symbol_search.selected(),
            total: state.symbol_search.len(),
            noun: "symbols",
            empty: "No matching symbols",
        },
        &state.theme,
        |index, is_selected| {
            let symbol_index = state.symbol_search.symbol_index_at(index)?;
            let symbol = state.watchlist.get(symbol_index)?;
            let is_current = symbol_index == state.selected_symbol;
            let style = if is_selected {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else if is_current {
                state.theme.accent_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.text_style()
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(symbol.clone(), style),
                Span::styled(if is_current { " current" } else { "" }, style),
            ])))
        },
    );
}

struct SearchFloating<'a> {
    title: &'static str,
    input_title: String,
    placeholder: &'static str,
    query: &'a str,
    selected: usize,
    total: usize,
    noun: &'static str,
    empty: &'static str,
}

fn render_search_floating(
    frame: &mut Frame<'_>,
    area: Rect,
    floating: SearchFloating<'_>,
    theme: &ThemeConfig,
    mut item_at: impl FnMut(usize, bool) -> Option<ListItem<'static>>,
) {
    if area.height < 4 {
        frame.render_widget(
            Paragraph::new(floating.title).block(floating_block(floating.title, theme)),
            area,
        );
        return;
    }

    let [input_area, list_area] = split_vertical(area, [Constraint::Length(3), Constraint::Min(0)]);
    let input = if floating.query.is_empty() {
        floating.placeholder.to_string()
    } else {
        floating.query.to_string()
    };
    frame.render_widget(
        Paragraph::new(input)
            .style(if floating.query.is_empty() {
                theme.muted_style()
            } else {
                theme.accent_style()
            })
            .block(dynamic_floating_block(floating.input_title, theme)),
        input_area,
    );

    let visible = command_window(
        floating.total,
        floating.selected,
        list_area.height.saturating_sub(2) as usize,
    );
    let visible_start = visible.start;
    let hidden_before = visible.start > 0;
    let hidden_after = visible.end < floating.total;
    let items = visible
        .enumerate()
        .filter_map(|(offset, _)| {
            let index = visible_start + offset;
            item_at(index, index == floating.selected)
        })
        .collect::<Vec<_>>();
    let title = match (floating.total, hidden_before, hidden_after) {
        (0, _, _) => format!("0 {}", floating.noun),
        (_, true, true) => format!("{}  more above/below", floating.noun),
        (_, true, false) => format!("{}  more above", floating.noun),
        (_, false, true) => format!("{}  more below", floating.noun),
        (_, false, false) => floating.noun.to_string(),
    };
    let items = if items.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            floating.empty,
            theme.muted_style(),
        )))]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(dynamic_floating_block(title, theme)),
        list_area,
    );
}

fn status_detail(state: &AppState, symbol: &str, errors: usize, width: u16) -> String {
    let runtime = if state.scheduler_error.is_some() {
        "scheduler error"
    } else if state.refresh_loading() {
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
        if let Some(profile) = state.trading_profile.as_deref() {
            return format!(
                " {symbol} | mode: {} | profile: {profile} | {runtime} | e:{errors} ",
                state.interaction_mode().label(),
            );
        }
        return format!(
            " {symbol} | mode: {} | focus: {} | {runtime} | e:{errors} ",
            state.interaction_mode().label(),
            state.panels.focused().title(),
        );
    }

    let profile = state
        .trading_profile
        .as_deref()
        .map(|profile| format!(" | profile: {profile}"))
        .unwrap_or_default();
    let prefix = format!(
        " {symbol} | mode: {}{profile} | focus: {} | visible: {}/{} | {runtime} | errors: {errors} | ",
        state.interaction_mode().label(),
        state.panels.focused().title(),
        state.visible_panels().len(),
        state.workspace.panels().len(),
    );
    let hint_budget = width.saturating_sub(prefix.len() as u16 + 1) as usize;
    format!("{}{} ", prefix, hints::status_key_hints(state, hint_budget))
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

fn floating_block(title: &'static str, theme: &ThemeConfig) -> Block<'static> {
    shadowed_block(simple_block(title), theme)
}

fn dynamic_floating_block(title: String, theme: &ThemeConfig) -> Block<'static> {
    shadowed_block(Block::default().title(title).borders(Borders::ALL), theme)
}

fn shadowed_block(block: Block<'static>, theme: &ThemeConfig) -> Block<'static> {
    block.shadow(
        Shadow::dark_shade()
            .style(theme.shadow_style())
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
