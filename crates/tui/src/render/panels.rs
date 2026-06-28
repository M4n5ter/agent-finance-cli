use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::research_snapshot::{PredictionMarketSnapshot, ResearchContextSnapshot};
use agent_finance_market::snapshot::QuoteSnapshot;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, Paragraph, Row, Table, Wrap};

use crate::layout::CockpitLayout;
use crate::model::Panel;
use crate::provider_health::ProviderHealthReport;
use crate::state::AppState;
use crate::task_log::TaskStatus;
use crate::theme::ThemeConfig;

use super::account::render_account;
use super::futures_state::render_futures_state;
use super::history;
use super::intent_review::render_intent_review;
use super::open_orders::render_open_orders;
use super::order_ticket::render_order_ticket;
use super::profile_risk::render_profile_risk;
use super::provider_health;
use super::risk_audit::render_risk_audit;
use super::settings::render_settings;
use super::transfer_ticket::render_transfer_ticket;
use super::widgets::{compact_text, format_price, format_volume, panel_block};

pub(super) fn render_docked(frame: &mut Frame<'_>, state: &AppState, layout: &CockpitLayout) {
    for panel in Panel::ALL {
        let Some(area) = layout.panel_rect(panel) else {
            continue;
        };
        match panel {
            Panel::Watchlist => render_watchlist(frame, state, area),
            Panel::Quote => render_quote(frame, state, area),
            Panel::OrderTicket => render_order_ticket(frame, state, area),
            Panel::OpenOrders => render_open_orders(frame, state, area),
            Panel::IntentReview => render_intent_review(frame, state, area),
            Panel::RiskAudit => render_risk_audit(frame, state, area),
            Panel::Account => render_account(frame, state, area),
            Panel::TransferTicket => render_transfer_ticket(frame, state, area),
            Panel::FuturesState => render_futures_state(frame, state, area),
            Panel::History => render_history(frame, state, area),
            Panel::Evidence => render_evidence(frame, state, area),
            Panel::Polymarket => render_polymarket(frame, state, area),
            Panel::Research => render_research(frame, state, area),
            Panel::ProviderHealth => render_provider_health(frame, state, area),
            Panel::TaskLog => render_task_log(frame, state, area),
            Panel::Settings => render_settings(frame, state, area),
            Panel::ProfileRisk => render_profile_risk(frame, state, area),
        }
    }
}

fn render_watchlist(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let mut items = state
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
                state.theme.accent_style().add_modifier(Modifier::BOLD)
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
    items.push(ListItem::new(Line::from("")));
    let watchlist_hint = if state
        .config_changes
        .iter()
        .any(|change| change == "watchlist")
    {
        "a add  d delete  left/right move  u undo  config: watchlist"
    } else {
        "a add  d delete  left/right move  u undo"
    };
    items.push(ListItem::new(Line::from(Span::styled(
        watchlist_hint,
        state.theme.muted_style(),
    ))));
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
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.refresh_loading() {
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
                state.theme.warning_style(),
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
            state.theme.accent_style().add_modifier(Modifier::BOLD),
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
                    state.theme.warning_style(),
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
    let chart = history::chart(&points, &state.theme);
    frame.render_widget(chart, chunks[1]);
}

fn render_evidence(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.evidence.selected_snapshot(symbol);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.evidence.loading() {
            " evidence loading..."
        } else {
            " evidence"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(evidence_lines(snapshot, &state.theme)),
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
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.research.loading() {
            " research loading..."
        } else {
            " research"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(research_lines(snapshot, &state.theme)),
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
            state.theme.accent_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if state.research.loading() {
            " prediction signals loading..."
        } else {
            " prediction signals"
        }),
    ])];

    match snapshot {
        Some(snapshot) => lines.extend(prediction_market_lines(snapshot, &state.theme)),
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
                    Cell::from(profile.provider.clone()).style(state.theme.muted_style()),
                    Cell::from("capability"),
                    Cell::from(profile.best_for.clone()),
                    Cell::from("-"),
                ])
            })
            .collect::<Vec<_>>()
    } else {
        provider_health::table_rows(report, area.height.saturating_sub(3) as usize, &state.theme)
    };
    frame.render_widget(
        Table::new(rows, provider_health::table_widths())
            .header(
                Row::new(["provider", "status", "detail", "freshness"])
                    .style(state.theme.accent_style()),
            )
            .block(panel_block(Panel::ProviderHealth, state)),
        area,
    );
}

fn render_task_log(frame: &mut Frame<'_>, state: &AppState, area: Rect) {
    let rows = state
        .task_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(3) as usize)
        .map(|entry| {
            let style = task_status_style(entry.status, &state.theme);
            Row::new([
                Cell::from(entry.status.label()).style(style),
                Cell::from(entry.message.clone()),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Table::new(rows, [Constraint::Length(10), Constraint::Min(10)])
            .header(Row::new(["status", "event"]).style(state.theme.accent_style()))
            .block(panel_block(Panel::TaskLog, state)),
        area,
    );
}

fn task_status_style(status: TaskStatus, theme: &ThemeConfig) -> Style {
    match status {
        TaskStatus::Info => theme.neutral_style(),
        TaskStatus::Running => theme.warning_style(),
        TaskStatus::Succeeded => theme.success_style(),
        TaskStatus::Warning => theme.warning_style(),
        TaskStatus::Failed => theme.danger_style(),
    }
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

fn evidence_lines(
    snapshot: &CryptoQuoteEvidenceSnapshot,
    theme: &ThemeConfig,
) -> Vec<Line<'static>> {
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
                theme.warning_style(),
            )));
        }
        return lines;
    }

    for provider in snapshot.providers.iter().take(4) {
        let style = if provider.ok {
            theme.success_style()
        } else {
            theme.warning_style()
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
                theme.muted_style(),
            )));
        }
    }
    lines
}

fn research_lines(snapshot: &ResearchContextSnapshot, theme: &ThemeConfig) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(format!(
        "freshness: {}  news={}",
        snapshot.fetched_at_local.as_deref().unwrap_or("-"),
        snapshot.news.len()
    ))];

    for item in snapshot.news.iter().take(3) {
        lines.push(Line::from(vec![
            Span::styled("news ", theme.success_style()),
            Span::raw(compact_text(&item.title, 96)),
        ]));
    }

    for error in scoped_errors(snapshot, ResearchErrorScope::News)
        .into_iter()
        .take(2)
    {
        lines.push(Line::from(Span::styled(
            format!("research warning: {error}"),
            theme.warning_style(),
        )));
    }

    lines
}

fn prediction_market_lines(
    snapshot: &ResearchContextSnapshot,
    theme: &ThemeConfig,
) -> Vec<Line<'static>> {
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
                theme.warning_style(),
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
            .map(|market| prediction_market_line(market, theme)),
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

fn prediction_market_line(market: &PredictionMarketSnapshot, theme: &ThemeConfig) -> Line<'static> {
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
        Span::styled("poly ", theme.prediction_style()),
        Span::raw(format!(
            "{probability} vol={volume} liq={liquidity} {}{url}",
            compact_text(&market.title, 72)
        )),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeConfig;
    use agent_finance_market::research_snapshot::ResearchNewsSnapshot;

    #[test]
    fn research_lines_do_not_duplicate_prediction_market_signals() {
        let snapshot = research_snapshot();
        let text = joined_lines(research_lines(&snapshot, &ThemeConfig::default()));

        assert!(text.contains("news=1"));
        assert!(text.contains("news AI optics demand"));
        assert!(!text.contains("poly "));
        assert!(!text.contains("markets=1"));
    }

    #[test]
    fn prediction_market_lines_show_probability_and_market_depth() {
        let snapshot = research_snapshot();
        let text = joined_lines(prediction_market_lines(&snapshot, &ThemeConfig::default()));

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

        let theme = ThemeConfig::default();
        let research = joined_lines(research_lines(&snapshot, &theme));
        let polymarket = joined_lines(prediction_market_lines(&snapshot, &theme));

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
}
