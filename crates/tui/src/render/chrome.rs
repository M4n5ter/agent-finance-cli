use ratatui::Frame;
use ratatui::layout::{Offset, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Shadow, Wrap};

use crate::confirmation_dialog::{self, ConfirmationRow};
use crate::hints;
use crate::model::FloatingKind;
use crate::mouse_target::MouseTarget;
use crate::search_floating_view::SearchFloatingLayout;
use crate::state::AppState;
use crate::status_bar::{StatusDetail, status_symbol_and_errors};
use crate::theme::ThemeConfig;
use crate::workspace_tabs::workspace_tab_segments;

pub(super) fn render_status(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    if area.is_empty() {
        return;
    }

    let areas = crate::status_bar::areas(area);
    render_workspace_tabs(frame, state, areas.tabs, mouse_target);

    if areas.detail.is_empty() {
        return;
    }

    let (symbol, errors) = status_symbol_and_errors(state);
    let text = crate::status_bar::detail(state, symbol, errors, areas.detail.width);
    frame.render_widget(
        Paragraph::new(status_detail_line(&state.theme, text, mouse_target))
            .style(state.theme.chrome_style().fg(state.theme.text.color())),
        areas.detail,
    );
}

fn status_detail_line(
    theme: &ThemeConfig,
    detail: StatusDetail,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    if detail.actions.is_empty() {
        return Line::from(detail.text);
    }
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for action in detail.actions {
        let start = action.start as usize;
        let end = action.end as usize;
        push_status_span(
            &mut spans,
            &detail.text,
            cursor,
            start,
            theme.chrome_style(),
        );
        let style =
            if mouse_target.is_some_and(|target| target.status_action_hovered(action.action)) {
                theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                theme.accent_style().add_modifier(Modifier::BOLD)
            };
        push_status_span(&mut spans, &detail.text, start, end, style);
        cursor = end;
    }
    push_status_span(
        &mut spans,
        &detail.text,
        cursor,
        detail.text.len(),
        theme.chrome_style(),
    );
    Line::from(spans)
}

fn push_status_span(
    spans: &mut Vec<Span<'static>>,
    text: &str,
    start: usize,
    end: usize,
    style: Style,
) {
    if start < end {
        spans.push(Span::styled(text[start..end].to_string(), style));
    }
}

fn render_workspace_tabs(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    if area.is_empty() {
        return;
    }

    let spans = workspace_tab_segments(area)
        .into_iter()
        .flat_map(|segment| {
            let hovered =
                mouse_target.is_some_and(|target| target.workspace_tab_hovered(segment.workspace));
            let selected = state.workspace == segment.workspace;
            let style = if hovered {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else if selected {
                state
                    .theme
                    .chrome_style()
                    .fg(state.theme.accent.color())
                    .add_modifier(Modifier::BOLD)
            } else {
                state.theme.chrome_style()
            };
            let mut spans = vec![Span::styled(segment.label, style)];
            if segment.has_divider_after {
                spans.push(Span::styled("|", state.theme.chrome_style()));
            }
            spans
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(state.theme.chrome_style()),
        area,
    );
}

pub(super) fn render_floating(
    frame: &mut Frame<'_>,
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    if kind == FloatingKind::CommandPalette {
        render_command_palette(frame, state, area, mouse_target);
        return;
    }
    if kind == FloatingKind::SymbolSearch {
        render_symbol_search(frame, state, area, mouse_target);
        return;
    }
    if kind == FloatingKind::WatchlistAdd {
        render_watchlist_add(frame, state, area);
        return;
    }
    if kind == FloatingKind::TradingProfile {
        render_trading_profile(frame, state, area);
        return;
    }
    if kind == FloatingKind::OrderTicketInput {
        render_order_ticket_input(frame, state, area);
        return;
    }
    if kind == FloatingKind::StagedExecutionConfirmation {
        render_staged_execution_confirmation(frame, state, area, mouse_target);
        return;
    }

    let text = match kind {
        FloatingKind::CommandPalette => unreachable!("command palette is rendered separately"),
        FloatingKind::SymbolSearch => unreachable!("symbol search is rendered separately"),
        FloatingKind::WatchlistAdd => unreachable!("watchlist add is rendered separately"),
        FloatingKind::TradingProfile => unreachable!("trading profile is rendered separately"),
        FloatingKind::OrderTicketInput => unreachable!("order ticket input is rendered separately"),
        FloatingKind::StagedExecutionConfirmation => {
            unreachable!("staged execution confirmation is rendered separately")
        }
        FloatingKind::Help => vec![
            Line::from("agent-finance cockpit"),
            Line::from("[/]: switch workspace"),
            Line::from("Tab/Shift-Tab: move pane focus"),
            Line::from("z: zoom focused pane or restore workspace layout"),
            Line::from("j/k or arrows: switch selected symbol"),
            Line::from(
                "1-7: focus watchlist, quote, history, evidence, Polymarket, research, settings",
            ),
            Line::from(": open command palette"),
            Line::from("/ search watchlist symbols"),
            Line::from("Enter: execute selected command in command palette"),
            Line::from("p inspect provider details"),
            Line::from("x close focused panel"),
            Line::from("0 restore all panels"),
            Line::from("r reset layout"),
            Line::from("mouse: focus panels, drag docked borders, resize floating corners"),
            Line::from("watchlist focus: a add, d delete, left/right reorder"),
            Line::from("q quit"),
        ],
        FloatingKind::LiveWritesConfirmation => {
            render_confirmation_dialog(frame, state, kind, area, mouse_target);
            return;
        }
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

fn render_staged_execution_confirmation(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    render_confirmation_dialog(
        frame,
        state,
        FloatingKind::StagedExecutionConfirmation,
        area,
        mouse_target,
    );
}

fn render_confirmation_dialog(
    frame: &mut Frame<'_>,
    state: &AppState,
    kind: FloatingKind,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let content_width = area.width.saturating_sub(2) as usize;
    let lines = confirmation_dialog::rows_for(
        kind,
        state.pending_staged_confirmation_view(),
        content_width,
    )
    .into_iter()
    .enumerate()
    .map(|(index, row)| confirmation_line(state, kind, row, index == 0, mouse_target))
    .map(ListItem::new);
    frame.render_widget(
        List::new(lines).block(floating_block(kind.title(), &state.theme)),
        area,
    );
}

fn confirmation_line(
    state: &AppState,
    kind: FloatingKind,
    row: ConfirmationRow,
    heading: bool,
    mouse_target: Option<MouseTarget>,
) -> Line<'static> {
    match row {
        ConfirmationRow::Text(text) if heading => Line::from(Span::styled(
            text,
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        )),
        ConfirmationRow::Text(text) => Line::from(text),
        ConfirmationRow::Input {
            label,
            value,
            matched,
        } => {
            let status = if matched { "matched" } else { "required" };
            let style = if matched {
                state.theme.accent_style()
            } else {
                state.theme.warning_style()
            };
            Line::from(vec![
                Span::raw(format!("{label}: ")),
                Span::styled(value, style.add_modifier(Modifier::BOLD)),
                Span::raw(format!("  {status}")),
            ])
        }
        ConfirmationRow::Blank => Line::from(""),
        ConfirmationRow::Buttons(buttons) => {
            let hovered = mouse_target.and_then(|target| target.confirmation_button_hovered(kind));
            Line::from(
                confirmation_dialog::button_segments(&buttons)
                    .into_iter()
                    .map(|segment| {
                        let style = match (segment.action, hovered) {
                            (Some(action), Some(hovered)) if action == hovered => {
                                state.theme.selected_style().add_modifier(Modifier::BOLD)
                            }
                            (Some(_), _) => state.theme.accent_style().add_modifier(Modifier::BOLD),
                            (None, _) => state.theme.text_style(),
                        };
                        Span::styled(segment.text, style)
                    })
                    .collect::<Vec<_>>(),
            )
        }
    }
}

fn floating_result_hovered(
    mouse_target: Option<MouseTarget>,
    kind: FloatingKind,
    index: usize,
) -> bool {
    mouse_target.is_some_and(|target| target.floating_result_hovered(kind, index))
}

fn render_command_palette(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Command Palette",
            input_title: hints::input_floating_title_for_kind(FloatingKind::CommandPalette)
                .expect("command palette has an input title"),
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
            let hovered =
                floating_result_hovered(mouse_target, FloatingKind::CommandPalette, index);
            let style = if hovered || is_selected {
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

fn render_symbol_search(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Symbol Search",
            input_title: hints::input_floating_title_for_kind(FloatingKind::SymbolSearch)
                .expect("symbol search has an input title"),
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
            let hovered = floating_result_hovered(mouse_target, FloatingKind::SymbolSearch, index);
            let style = if hovered || is_selected {
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

fn render_watchlist_add(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Add Symbols",
            input_title: hints::input_floating_title_for_kind(FloatingKind::WatchlistAdd)
                .expect("watchlist add has an input title"),
            placeholder: "LITE, AAOI, BTCUSDT",
            query: state.watchlist_add.query(),
            selected: 0,
            total: 2,
            noun: "actions",
            empty: "Enter adds symbols, Esc cancels",
        },
        &state.theme,
        |index, is_selected| {
            let style = if is_selected {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.text_style()
            };
            let text = match index {
                0 => "Enter - add normalized symbols",
                1 => "Esc - cancel",
                _ => return None,
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(text, style),
            ])))
        },
    );
}

fn render_trading_profile(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Trading Profile",
            input_title: hints::input_floating_title_for_kind(FloatingKind::TradingProfile)
                .expect("trading profile has an input title"),
            placeholder: "mainnet, testnet, paper",
            query: state.profile_editor.query(),
            selected: 0,
            total: 3,
            noun: "actions",
            empty: "Enter sets profile, blank clears it, Esc cancels",
        },
        &state.theme,
        |index, is_selected| {
            let style = if is_selected {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.text_style()
            };
            let text = match index {
                0 => format!(
                    "current - {}",
                    state.trading_profile.as_deref().unwrap_or("none")
                ),
                1 => format!(
                    "next - {}",
                    state.profile_editor.profile().as_deref().unwrap_or("none")
                ),
                2 => "Enter - set default trading profile".to_string(),
                _ => return None,
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(text, style),
            ])))
        },
    );
}

fn render_order_ticket_input(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let field_label = state.order_ticket_input.field().label();
    let next_value = state.order_ticket_input.committed_value();
    let next_label = next_value.as_deref().unwrap_or("blank");

    render_search_floating(
        frame,
        area,
        SearchFloating {
            title: "Order Ticket Input",
            input_title: hints::input_floating_title_for_kind(FloatingKind::OrderTicketInput)
                .expect("order ticket input has an input title"),
            placeholder: "0.05 or 204.00",
            query: state.order_ticket_input.query(),
            selected: 0,
            total: 3,
            noun: "actions",
            empty: "Enter applies value, Esc cancels",
        },
        &state.theme,
        |index, is_selected| {
            let style = if is_selected {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else {
                state.theme.text_style()
            };
            let text = match index {
                0 => format!("field - {field_label}"),
                1 => format!("next - {next_label}"),
                2 => "Enter - apply to order ticket".to_string(),
                _ => return None,
            };
            Some(ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(text, style),
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

    let layout = SearchFloatingLayout::new(area, floating.total, floating.selected);
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
        layout.input_area,
    );

    let window = layout.window();
    let visible_start = window.start();
    let hidden_before = window.has_hidden_before();
    let hidden_after = window.has_hidden_after(floating.total);
    let items = window
        .visible()
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
        layout.list_area,
    );
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
