use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::snapshot::QuoteSnapshot;
use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Sparkline, Wrap};

use crate::command::COMMANDS;
use crate::layout::{self, CockpitLayout};
use crate::model::{FloatingKind, Panel, TaskLevel};
use crate::state::AppState;

pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    let layout = layout::build(
        frame.area(),
        &state.layout,
        &state.floating,
        state.panels.open_panels(),
    );
    render_docked(frame, state, &layout);
    render_status(frame, state, layout.status);
    for floating in &layout.floating {
        frame.render_widget(Clear, floating.rect);
        render_floating(frame, state, floating.kind, floating.rect);
    }
}

fn render_docked(frame: &mut Frame<'_>, state: &AppState, layout: &CockpitLayout) {
    for panel in Panel::ALL {
        let Some(area) = layout.panel_rect(panel) else {
            continue;
        };
        match panel {
            Panel::Watchlist => render_watchlist(frame, state, area),
            Panel::Quote => render_quote(frame, state, area),
            Panel::History => render_history(frame, state, area),
            Panel::Evidence => render_evidence(frame, state, area),
            Panel::Research => render_research(frame, state, area),
            Panel::ProviderHealth => render_provider_health(frame, state, area),
            Panel::TaskLog => render_task_log(frame, state, area),
        }
    }
}

fn render_watchlist(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let items = state
        .watchlist
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            let marker = if index == state.selected_symbol {
                ">"
            } else {
                " "
            };
            let style = if index == state.selected_symbol {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, style),
                Span::raw(" "),
                Span::styled(symbol.clone(), style),
                Span::raw(" "),
                Span::styled(
                    state
                        .market_snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.quote_for(symbol))
                        .and_then(|quote| quote.price)
                        .map(format_price)
                        .unwrap_or_else(|| "-".to_string()),
                    style,
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(panel_block(Panel::Watchlist, state)),
        area,
    );
}

fn render_quote(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let quote = state
        .market_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.quote_for(symbol));
    let mut text = vec![Line::from(vec![
        Span::styled(
            symbol,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.refresh.loading {
            " refreshing..."
        } else {
            " market snapshot"
        }),
    ])];
    match quote {
        Some(quote) => text.extend(quote_lines(quote)),
        None => text.push(Line::from(
            "No quote loaded yet. Waiting for the next refresh.",
        )),
    }
    if let Some(snapshot) = state.market_snapshot.as_ref() {
        if let Some(fetched_at) = snapshot.fetched_at_local.as_ref() {
            text.push(Line::from(format!("freshness: {fetched_at}")));
        }
        for error in snapshot.errors.iter().take(2) {
            text.push(Line::from(Span::styled(
                format!("provider error: {error}"),
                Style::default().fg(Color::Yellow),
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(text)
            .block(panel_block(Panel::Quote, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_history(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let block = panel_block(Panel::History, state);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(inner);

    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.history.selected_snapshot(symbol);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.history.loading() {
            " history loading..."
        } else {
            " history"
        }),
    ])];

    match snapshot {
        Some(snapshot) => {
            lines.push(Line::from(format!(
                "provider: {}  interval={}  bars={}",
                snapshot.provider,
                snapshot.interval,
                snapshot.bars.len()
            )));
            lines.push(Line::from(format!(
                "latest: {} at {}  return={}",
                snapshot
                    .latest_close
                    .map(format_price)
                    .unwrap_or_else(|| "-".to_string()),
                snapshot.latest_time.as_deref().unwrap_or("-"),
                snapshot
                    .return_pct
                    .map(|value| format!("{value:.2}%"))
                    .unwrap_or_else(|| "-".to_string())
            )));
            lines.push(Line::from(format!(
                "volume: {}  freshness: {}",
                snapshot
                    .volume
                    .map(format_volume)
                    .unwrap_or_else(|| "-".to_string()),
                snapshot.fetched_at_local.as_deref().unwrap_or("-")
            )));
            for error in snapshot.errors.iter().take(1) {
                lines.push(Line::from(Span::styled(
                    format!("history warning: {error}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }
        None => lines.push(Line::from(
            "No history loaded yet. Waiting for the selected symbol.",
        )),
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), chunks[0]);

    let sparkline = snapshot
        .map(|snapshot| {
            let closes = snapshot
                .bars
                .iter()
                .map(|bar| bar.close)
                .collect::<Vec<_>>();
            sparkline_values(&closes)
        })
        .unwrap_or_default();
    frame.render_widget(
        Sparkline::default()
            .data(&sparkline)
            .max(100)
            .style(Style::default().fg(Color::Green)),
        chunks[1],
    );
}

fn render_evidence(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.evidence.selected_snapshot(symbol);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.evidence.loading() {
            " evidence loading..."
        } else {
            " evidence"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(evidence_lines(snapshot)),
        None => lines.push(Line::from(
            "No crypto evidence loaded yet. Waiting for the selected symbol.",
        )),
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::Evidence, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_research(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.research.selected_snapshot(symbol);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.research.loading() {
            " research loading..."
        } else {
            " research"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(research_lines(snapshot)),
        None => lines.push(Line::from(
            "No research context loaded yet. Waiting for the selected symbol.",
        )),
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::Research, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_provider_health(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let items = state
        .provider_profiles
        .iter()
        .take(8)
        .map(|profile| {
            ListItem::new(Line::from(vec![
                Span::styled(profile.provider.clone(), Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::raw(profile.best_for.clone()),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(simple_block(Panel::ProviderHealth.title())),
        area,
    );
}

fn render_task_log(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let items = state
        .task_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|entry| {
            let style = match entry.level {
                TaskLevel::Info => Style::default().fg(Color::Gray),
                TaskLevel::Warning => Style::default().fg(Color::Yellow),
            };
            ListItem::new(Line::from(Span::styled(entry.message.clone(), style)))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(simple_block(Panel::TaskLog.title())),
        area,
    );
}

fn quote_lines(quote: &QuoteSnapshot) -> Vec<Line<'static>> {
    vec![
        Line::from(format!(
            "current: {} {}  chg={}  session={}",
            quote.currency.as_deref().unwrap_or(""),
            quote
                .price
                .map(format_price)
                .unwrap_or_else(|| "-".to_string()),
            quote
                .change_pct
                .map(|value| format!("{value:.2}%"))
                .unwrap_or_else(|| "-".to_string()),
            quote.session.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "provider: {}  time={}",
            quote.provider,
            quote.market_time_local.as_deref().unwrap_or("-")
        )),
        Line::from(format!(
            "regular: prev={} open={} high={} low={} volume={}",
            quote
                .regular_basis
                .previous_close
                .map(format_price)
                .unwrap_or_else(|| "-".to_string()),
            quote
                .regular_basis
                .open
                .map(format_price)
                .unwrap_or_else(|| "-".to_string()),
            quote
                .regular_basis
                .high
                .map(format_price)
                .unwrap_or_else(|| "-".to_string()),
            quote
                .regular_basis
                .low
                .map(format_price)
                .unwrap_or_else(|| "-".to_string()),
            quote
                .regular_basis
                .volume
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        )),
    ]
}

fn evidence_lines(snapshot: &CryptoQuoteEvidenceSnapshot) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!(
            "quote / {}  providers={}/{}",
            snapshot.instrument, snapshot.ok_providers, snapshot.total_providers
        )),
        Line::from(format!(
            "freshness: {}",
            snapshot.fetched_at_local.as_deref().unwrap_or("-")
        )),
    ];

    if snapshot.total_providers == 0 {
        for error in snapshot.errors.iter().take(2) {
            lines.push(Line::from(Span::styled(
                error.clone(),
                Style::default().fg(Color::Yellow),
            )));
        }
        return lines;
    }

    for provider in snapshot.providers.iter().take(4) {
        let style = if provider.ok {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        };
        lines.push(Line::from(vec![
            Span::styled(provider.provider.clone(), style),
            Span::raw(format!(
                " endpoints={}/{} required_failed={}",
                provider.ok_endpoints, provider.total_endpoints, provider.required_failed
            )),
        ]));
        if let Some(error) = provider.first_error.as_ref() {
            lines.push(Line::from(Span::styled(
                format!("  {error}"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    lines
}

fn research_lines(snapshot: &ResearchContextSnapshot) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(format!(
        "freshness: {}  news={} prediction_markets={}",
        snapshot.fetched_at_local.as_deref().unwrap_or("-"),
        snapshot.news.len(),
        snapshot.prediction_markets.len()
    ))];

    for item in snapshot.news.iter().take(3) {
        lines.push(Line::from(vec![
            Span::styled("news ", Style::default().fg(Color::Green)),
            Span::raw(compact_text(&item.title, 96)),
        ]));
    }

    for market in snapshot.prediction_markets.iter().take(3) {
        let probability = market
            .probability
            .map(|value| format!("{:.0}%", value * 100.0))
            .unwrap_or_else(|| "-".to_string());
        lines.push(Line::from(vec![
            Span::styled("poly ", Style::default().fg(Color::Magenta)),
            Span::raw(format!("{probability} {}", compact_text(&market.title, 84))),
        ]));
    }

    for error in snapshot.errors.iter().take(2) {
        lines.push(Line::from(Span::styled(
            format!("research warning: {error}"),
            Style::default().fg(Color::Yellow),
        )));
    }

    lines
}

fn compact_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut output = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

fn format_price(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.4}")
    }
}

fn format_volume(value: f64) -> String {
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

fn sparkline_values(values: &[f64]) -> Vec<u64> {
    let finite = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return Vec::new();
    }

    let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return values.iter().map(|_| 50).collect();
    }

    values
        .iter()
        .map(|value| {
            if !value.is_finite() {
                0
            } else {
                (((value - min) / (max - min)) * 100.0).round() as u64
            }
        })
        .collect()
}

fn render_status(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let errors = state
        .market_snapshot
        .as_ref()
        .map(|snapshot| snapshot.errors.len())
        .unwrap_or(0);
    let text = format!(
        " {} | focus: {} | panels: {}/{} | {} | errors: {} | j/k symbol | x close | 0 restore | : command | q quit ",
        symbol,
        state.panels.focused().title(),
        state.panels.open_count(),
        Panel::ALL.len(),
        if state.scheduler_error.is_some() {
            "scheduler error"
        } else if state.refresh.loading {
            "refreshing"
        } else {
            "ready"
        },
        errors
    );
    frame.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        area,
    );
}

fn render_floating(frame: &mut Frame<'_>, state: &AppState, kind: FloatingKind, area: Rect) {
    if kind == FloatingKind::CommandPalette {
        render_command_palette(frame, state, area);
        return;
    }

    let text = match kind {
        FloatingKind::CommandPalette => unreachable!("command palette is rendered separately"),
        FloatingKind::Help => vec![
            Line::from("agent-finance cockpit"),
            Line::from("j/k or arrows: switch selected symbol"),
            Line::from(": open command palette"),
            Line::from("Enter: execute selected command in command palette"),
            Line::from("p inspect provider details"),
            Line::from("x close focused panel"),
            Line::from("0 restore all panels"),
            Line::from("r reset layout"),
            Line::from("mouse: focus panels and drag docked column borders"),
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
            .block(simple_block(kind.title()))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_command_palette(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let selected = state.command_palette.selected;
    let visible = command_window(
        COMMANDS.len(),
        selected,
        area.height.saturating_sub(2) as usize,
    );
    let hidden_before = visible.start > 0;
    let hidden_after = visible.end < COMMANDS.len();
    let items = COMMANDS[visible.clone()]
        .iter()
        .enumerate()
        .map(|(offset, command)| {
            let index = visible.start + offset;
            let is_selected = index == selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                Span::styled(if is_selected { "> " } else { "  " }, style),
                Span::styled(command.title, style),
                Span::styled(" - ", style),
                Span::styled(command.description, style),
            ]))
        })
        .collect::<Vec<_>>();

    let title = match (hidden_before, hidden_after) {
        (true, true) => "Command Palette  Enter run  Esc close  more above/below",
        (true, false) => "Command Palette  Enter run  Esc close  more above",
        (false, true) => "Command Palette  Enter run  Esc close  more below",
        (false, false) => "Command Palette  Enter run  Esc close",
    };
    frame.render_widget(List::new(items).block(simple_block(title)), area);
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

fn panel_block(panel: Panel, state: &AppState) -> Block<'static> {
    let style = if state.panels.focused() == panel {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };
    simple_block(panel.title()).border_style(style)
}

fn simple_block(title: &'static str) -> Block<'static> {
    Block::default().title(title).borders(Borders::ALL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_values_preserve_flat_and_range_shape() {
        assert_eq!(sparkline_values(&[10.0, 10.0, 10.0]), vec![50, 50, 50]);
        assert_eq!(sparkline_values(&[10.0, 15.0, 20.0]), vec![0, 50, 100]);
    }

    #[test]
    fn command_window_keeps_selected_command_visible() {
        assert_eq!(command_window(11, 0, 7), 0..7);
        assert_eq!(command_window(11, 6, 7), 0..7);
        assert_eq!(command_window(11, 10, 7), 4..11);
        assert_eq!(command_window(11, 10, 0), 0..0);
    }
}
