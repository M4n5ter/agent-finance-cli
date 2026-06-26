use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::research_snapshot::{PredictionMarketSnapshot, ResearchContextSnapshot};
use agent_finance_market::snapshot::QuoteSnapshot;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap};

use crate::layout::{self, CockpitLayout};
use crate::model::{Panel, TaskLevel};
use crate::pane_status::{TuiPaneStatus, pane_health};
use crate::provider_health::ProviderHealthReport;
use crate::state::AppState;

mod chrome;
mod history;
mod provider_health;

use chrome::{render_floating, render_status};

pub fn render(frame: &mut Frame<'_>, state: &AppState) {
    let layout = layout::build(
        frame.area(),
        &state.layout,
        &state.floating,
        &state.visible_panels(),
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
            Panel::Polymarket => render_polymarket(frame, state, area),
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

    let closes = snapshot
        .map(|snapshot| {
            snapshot
                .bars
                .iter()
                .map(|bar| bar.close)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let points = history::chart_points(&closes);
    let chart = history::chart(&points);
    frame.render_widget(chart, chunks[1]);
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

fn render_polymarket(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
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
            " prediction signals loading..."
        } else {
            " prediction signals"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(prediction_market_lines(snapshot)),
        None => lines.push(Line::from(
            "No prediction market context loaded yet. Waiting for research refresh.",
        )),
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block(Panel::Polymarket, state))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_provider_health(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let report = ProviderHealthReport::from_state(state);
    let rows = if report.is_empty() {
        state
            .provider_profiles
            .iter()
            .take(8)
            .map(|profile| {
                Row::new([
                    Cell::from(profile.provider.clone())
                        .style(Style::default().fg(Color::DarkGray)),
                    Cell::from("capability"),
                    Cell::from(profile.best_for.clone()),
                    Cell::from("-"),
                ])
            })
            .collect::<Vec<_>>()
    } else {
        provider_health::table_rows(report, area.height.saturating_sub(3) as usize)
    };
    frame.render_widget(
        Table::new(rows, provider_health::table_widths())
            .header(
                Row::new(["provider", "status", "detail", "freshness"])
                    .style(Style::default().fg(Color::Cyan)),
            )
            .block(panel_block(Panel::ProviderHealth, state)),
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
        List::new(items).block(panel_block(Panel::TaskLog, state)),
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
        "freshness: {}  news={}",
        snapshot.fetched_at_local.as_deref().unwrap_or("-"),
        snapshot.news.len()
    ))];

    for item in snapshot.news.iter().take(3) {
        lines.push(Line::from(vec![
            Span::styled("news ", Style::default().fg(Color::Green)),
            Span::raw(compact_text(&item.title, 96)),
        ]));
    }

    for error in scoped_errors(snapshot, ResearchErrorScope::News)
        .into_iter()
        .take(2)
    {
        lines.push(Line::from(Span::styled(
            format!("research warning: {error}"),
            Style::default().fg(Color::Yellow),
        )));
    }

    lines
}

fn prediction_market_lines(snapshot: &ResearchContextSnapshot) -> Vec<Line<'static>> {
    let errors = scoped_errors(snapshot, ResearchErrorScope::Polymarket);
    let mut lines = vec![Line::from(format!(
        "freshness: {}  markets={}",
        snapshot.fetched_at_local.as_deref().unwrap_or("-"),
        snapshot.prediction_markets.len()
    ))];

    if !errors.is_empty() {
        lines.extend(errors.into_iter().take(2).map(|error| {
            Line::from(Span::styled(
                format!("polymarket warning: {error}"),
                Style::default().fg(Color::Yellow),
            ))
        }));
    } else if snapshot.prediction_markets.is_empty() {
        lines.push(Line::from(
            "No related Polymarket signals found for the selected symbol.",
        ));
    }

    lines.extend(
        snapshot
            .prediction_markets
            .iter()
            .take(5)
            .map(prediction_market_line),
    );

    lines
}

#[derive(Debug, Clone, Copy)]
enum ResearchErrorScope {
    News,
    Polymarket,
}

impl ResearchErrorScope {
    const fn prefix(self) -> &'static str {
        match self {
            Self::News => "news: ",
            Self::Polymarket => "polymarket: ",
        }
    }
}

fn scoped_errors(snapshot: &ResearchContextSnapshot, scope: ResearchErrorScope) -> Vec<String> {
    let prefix = scope.prefix();
    snapshot
        .errors
        .iter()
        .filter_map(|error| error.strip_prefix(prefix).map(str::to_string))
        .collect()
}

fn prediction_market_line(market: &PredictionMarketSnapshot) -> Line<'static> {
    let probability = market
        .probability
        .map(|value| format!("{:.0}%", value * 100.0))
        .unwrap_or_else(|| "-".to_string());
    let volume = market
        .volume
        .map(format_volume)
        .unwrap_or_else(|| "-".to_string());
    let liquidity = market
        .liquidity
        .map(format_volume)
        .unwrap_or_else(|| "-".to_string());
    let url = market
        .market_url
        .as_deref()
        .map(|value| format!("  {}", compact_text(value, 42)))
        .unwrap_or_default();

    Line::from(vec![
        Span::styled("poly ", Style::default().fg(Color::Magenta)),
        Span::raw(format!(
            "{probability} vol={volume} liq={liquidity} {}{url}",
            compact_text(&market.title, 72)
        )),
    ])
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

fn panel_block(panel: Panel, state: &AppState) -> Block<'static> {
    let status = pane_health(state, panel).status;
    let style = if state.panels.focused() == panel {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(status_color(status))
    };
    let title = format!("{} [{}]", panel.title(), status.label());
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(style)
}

fn status_color(status: TuiPaneStatus) -> Color {
    match status {
        TuiPaneStatus::Fresh => Color::Green,
        TuiPaneStatus::Loading => Color::Yellow,
        TuiPaneStatus::Partial | TuiPaneStatus::Empty | TuiPaneStatus::Stale => Color::Gray,
        TuiPaneStatus::Error => Color::Red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::TuiConfig;
    use crate::model::{FloatingKind, WorkspaceKind};
    use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
    use agent_finance_market::history_snapshot::HistorySnapshot;
    use agent_finance_market::research_snapshot::ResearchNewsSnapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::symbols;

    #[test]
    fn workspace_tabs_and_adaptive_status_render_without_overflow() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string(), "BTCUSDT".to_string()],
            ..TuiConfig::default()
        });
        state.reduce(crate::state::Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Crypto,
        )));

        let wide = render_to_text(&state, 120, 32);
        assert!(wide.contains("Overview"));
        assert!(wide.contains("Crypto"));
        assert!(wide.contains("mode: normal"));

        let narrow = render_to_text(&state, 48, 20);
        assert!(narrow.contains("Crypto"));
        assert!(narrow.contains("CRDO"));
        assert!(!narrow.contains("[/] workspace"));
    }

    #[test]
    fn overview_workspace_matches_snapshot_at_100x30() {
        let mut state = snapshot_state();
        state.reduce(crate::state::Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Overview,
        )));

        insta::assert_snapshot!(
            "overview_workspace_100x30",
            render_to_text_grid(&state, 100, 30)
        );
    }

    #[test]
    fn command_palette_matches_snapshot_at_140x40() {
        let mut state = snapshot_state();
        state.reduce(crate::state::Action::Execute(ActionId::OpenFloating(
            FloatingKind::CommandPalette,
        )));
        for character in "help".chars() {
            state.reduce(crate::state::Action::EditCommandQuery(
                tui_input::InputRequest::InsertChar(character),
            ));
        }

        insta::assert_snapshot!(
            "command_palette_140x40",
            render_to_text_grid(&state, 140, 40)
        );
    }

    #[test]
    fn narrow_workspace_matches_snapshot_at_48x20() {
        let mut state = snapshot_state();
        state.reduce(crate::state::Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Crypto,
        )));

        insta::assert_snapshot!(
            "narrow_workspace_48x20",
            render_to_text_grid(&state, 48, 20)
        );
    }

    #[test]
    fn panel_badges_follow_observable_load_error_stale_and_empty_states() {
        let mut state = snapshot_state();

        state.reduce(crate::state::Action::RefreshStarted(1));
        assert!(render_to_text_grid(&state, 100, 30).contains("Quote / Sessions [loading]"));

        state.reduce(crate::state::Action::RefreshFailed {
            generation: 1,
            error: "provider timeout".to_string(),
        });
        let overview = render_to_text_grid(&state, 100, 30);
        assert!(overview.contains("Quote / Sessions [error]"));
        assert!(overview.contains("Task Log [fresh]"));

        state.reduce(crate::state::Action::Execute(ActionId::SelectSymbolBy(1)));
        state.reduce(crate::state::Action::HistoryStarted {
            generation: 2,
            symbol: "BTCUSDT".to_string(),
        });
        state.reduce(crate::state::Action::HistoryLoaded {
            generation: 2,
            snapshot: history_snapshot("BTCUSDT"),
        });
        state.reduce(crate::state::Action::EvidenceStarted {
            generation: 3,
            symbol: "BTCUSDT".to_string(),
        });
        state.reduce(crate::state::Action::EvidenceLoaded {
            generation: 3,
            snapshot: evidence_snapshot("BTCUSDT"),
        });

        state.reduce(crate::state::Action::Execute(ActionId::SelectSymbolBy(1)));
        state.reduce(crate::state::Action::Execute(ActionId::SetWorkspace(
            WorkspaceKind::Crypto,
        )));
        let text = render_to_text_grid(&state, 100, 30);

        assert!(text.contains("History Chart [stale]"));
        assert!(text.contains("Crypto Evidence [empty]"));
        assert!(!text.contains("Crypto Evidence [stale]"));
    }

    #[test]
    fn floating_panes_render_with_shadow_layer() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(crate::state::Action::Execute(ActionId::OpenFloating(
            FloatingKind::CommandPalette,
        )));

        let text = render_to_text(&state, 100, 30);
        assert!(text.contains("Command"));
        assert!(text.contains("Open help"));
        assert!(text.contains(symbols::shade::DARK));
    }

    #[test]
    fn research_lines_do_not_duplicate_prediction_market_signals() {
        let snapshot = research_snapshot();
        let text = joined_lines(research_lines(&snapshot));

        assert!(text.contains("news=1"));
        assert!(text.contains("news AI optics demand"));
        assert!(!text.contains("poly "));
        assert!(!text.contains("markets=1"));
    }

    #[test]
    fn prediction_market_lines_show_probability_and_market_depth() {
        let snapshot = research_snapshot();
        let text = joined_lines(prediction_market_lines(&snapshot));

        assert!(text.contains("markets=1"));
        assert!(text.contains("63%"));
        assert!(text.contains("vol=1.50M"));
        assert!(text.contains("liq=250.00K"));
    }

    #[test]
    fn research_panels_show_only_their_scoped_errors() {
        let mut snapshot = research_snapshot();
        snapshot.prediction_markets.clear();
        snapshot.errors = vec![
            "news: provider timeout".to_string(),
            "polymarket: clob unavailable".to_string(),
        ];

        let research = joined_lines(research_lines(&snapshot));
        let polymarket = joined_lines(prediction_market_lines(&snapshot));

        assert!(research.contains("research warning: provider timeout"));
        assert!(!research.contains("clob unavailable"));
        assert!(polymarket.contains("polymarket warning: clob unavailable"));
        assert!(!polymarket.contains("provider timeout"));
        assert!(!polymarket.contains("No related Polymarket signals"));
    }

    fn research_snapshot() -> ResearchContextSnapshot {
        ResearchContextSnapshot {
            requested_symbol: "CRDO".to_string(),
            symbol: "CRDO".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            news: vec![ResearchNewsSnapshot {
                title: "AI optics demand accelerates".to_string(),
                provider: "test".to_string(),
                module: "news".to_string(),
            }],
            prediction_markets: vec![PredictionMarketSnapshot {
                title: "Will AI infrastructure stocks outperform this quarter?".to_string(),
                probability: Some(0.63),
                volume: Some(1_500_000.0),
                liquidity: Some(250_000.0),
                market_url: Some("https://polymarket.com/event/ai-infrastructure".to_string()),
            }],
            errors: Vec::new(),
        }
    }

    fn snapshot_state() -> AppState {
        AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string(), "BTCUSDT".to_string()],
            ..TuiConfig::default()
        })
    }

    fn history_snapshot(symbol: &str) -> HistorySnapshot {
        HistorySnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            provider: "test".to_string(),
            interval: "1d".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            latest_close: Some(100.0),
            latest_time: Some("2026-06-25".to_string()),
            return_pct: Some(1.0),
            volume: Some(10_000.0),
            bars: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn evidence_snapshot(symbol: &str) -> CryptoQuoteEvidenceSnapshot {
        CryptoQuoteEvidenceSnapshot {
            requested_symbol: symbol.to_string(),
            symbol: symbol.to_string(),
            instrument: "spot".to_string(),
            fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
            ok_providers: 1,
            total_providers: 1,
            providers: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn joined_lines(lines: Vec<Line<'static>>) -> String {
        lines
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_to_text_grid(state: &AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, state)).unwrap();
        let buffer = terminal.backend().buffer();

        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_to_text(state: &AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, state)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }
}
