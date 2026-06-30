use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, Paragraph, Row, Table, Wrap};

use crate::layout::CockpitLayout;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::provider_health::ProviderHealthReport;
use crate::read_only_panel_view;
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
use super::widgets::{format_price, panel_block};

pub(super) fn render_docked(
    frame: &mut Frame<'_>,
    state: &AppState,
    layout: &CockpitLayout,
    mouse_target: Option<MouseTarget>,
) {
    for panel in Panel::ALL {
        let Some(area) = layout.panel_rect(panel) else {
            continue;
        };
        match panel {
            Panel::Watchlist => render_watchlist(frame, state, area, mouse_target),
            Panel::Quote => render_quote(frame, state, area, mouse_target),
            Panel::OrderTicket => render_order_ticket(frame, state, area, mouse_target),
            Panel::OpenOrders => render_open_orders(frame, state, area, mouse_target),
            Panel::IntentReview => render_intent_review(frame, state, area, mouse_target),
            Panel::RiskAudit => render_risk_audit(frame, state, area, mouse_target),
            Panel::Account => render_account(frame, state, area, mouse_target),
            Panel::TransferTicket => render_transfer_ticket(frame, state, area, mouse_target),
            Panel::FuturesState => render_futures_state(frame, state, area, mouse_target),
            Panel::History => render_history(frame, state, area, mouse_target),
            Panel::Evidence => render_evidence(frame, state, area, mouse_target),
            Panel::Polymarket => render_polymarket(frame, state, area, mouse_target),
            Panel::Research => render_research(frame, state, area, mouse_target),
            Panel::ProviderHealth => render_provider_health(frame, state, area, mouse_target),
            Panel::TaskLog => render_task_log(frame, state, area, mouse_target),
            Panel::Settings => render_settings(frame, state, area, mouse_target),
            Panel::ProfileRisk => render_profile_risk(frame, state, area, mouse_target),
        }
    }
}

fn render_watchlist(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let mut items = state
        .watchlist
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            let hovered = panel_row_hovered(mouse_target, Panel::Watchlist, index);
            let marker = if index == state.selected_symbol {
                ">"
            } else {
                " "
            };
            let style = if hovered {
                state.theme.selected_style().add_modifier(Modifier::BOLD)
            } else if index == state.selected_symbol {
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

pub(super) fn panel_row_hovered(
    mouse_target: Option<MouseTarget>,
    panel: Panel,
    index: usize,
) -> bool {
    mouse_target.is_some_and(|target| target.panel_row_hovered(panel, index))
}

fn render_quote(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let block = panel_block(Panel::Quote, state);
    let inner = block.inner(area);
    frame.render_widget(
        Paragraph::new(read_only_panel_lines(
            state,
            Panel::Quote,
            inner.width,
            inner.height,
            mouse_target,
        ))
        .block(block)
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_history(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let block = panel_block(Panel::History, state);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chart_area = read_only_panel_view::history_chart_area(area);
    let text_area = Rect {
        height: chart_area.y.saturating_sub(inner.y),
        ..inner
    };

    frame.render_widget(
        Paragraph::new(read_only_panel_lines(
            state,
            Panel::History,
            text_area.width,
            text_area.height,
            mouse_target,
        ))
        .wrap(Wrap { trim: true }),
        text_area,
    );

    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.history.selected_snapshot(symbol);
    let bars = snapshot
        .map(|snapshot| snapshot.bars.as_slice())
        .unwrap_or_default();
    let hover = mouse_target.and_then(|target| target.panel_chart_hovered(Panel::History));
    let chart = history::chart(bars, &state.theme, hover);
    frame.render_widget(chart, chart_area);
}

fn render_evidence(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let block = panel_block(Panel::Evidence, state);
    let inner = block.inner(area);
    frame.render_widget(
        Paragraph::new(read_only_panel_lines(
            state,
            Panel::Evidence,
            inner.width,
            inner.height,
            mouse_target,
        ))
        .block(block)
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_research(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let block = panel_block(Panel::Research, state);
    let inner = block.inner(area);
    frame.render_widget(
        Paragraph::new(read_only_panel_lines(
            state,
            Panel::Research,
            inner.width,
            inner.height,
            mouse_target,
        ))
        .block(block)
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_polymarket(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
    let block = panel_block(Panel::Polymarket, state);
    let inner = block.inner(area);
    frame.render_widget(
        Paragraph::new(read_only_panel_lines(
            state,
            Panel::Polymarket,
            inner.width,
            inner.height,
            mouse_target,
        ))
        .block(block)
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn read_only_panel_lines(
    state: &AppState,
    panel: Panel,
    content_width: u16,
    content_height: u16,
    mouse_target: Option<MouseTarget>,
) -> Vec<Line<'static>> {
    let mut lines = read_only_panel_view::panel_action_line(
        state,
        panel,
        content_width,
        content_height,
        mouse_target,
    )
    .map(|action_line| vec![action_line.line])
    .unwrap_or_default();
    let content = match panel {
        Panel::Quote => read_only_panel_view::quote_lines(state)
            .into_iter()
            .map(owned_line)
            .collect::<Vec<_>>(),
        Panel::History => read_only_panel_view::history_summary_lines(state)
            .into_iter()
            .map(owned_line)
            .collect::<Vec<_>>(),
        Panel::Evidence => read_only_panel_view::evidence_panel_lines(state),
        Panel::Polymarket => read_only_panel_view::polymarket_panel_lines(state),
        Panel::Research => read_only_panel_view::research_panel_lines(state),
        _ => Vec::new(),
    };
    lines.extend(hover_info_lines(
        content,
        mouse_target,
        panel,
        state.theme.selected_style(),
    ));
    lines
}

fn owned_line(line: Line<'_>) -> Line<'static> {
    let mut owned = Line::from(
        line.spans
            .into_iter()
            .map(|span| Span::styled(span.content.into_owned(), span.style))
            .collect::<Vec<_>>(),
    );
    owned.style = line.style;
    owned.alignment = line.alignment;
    owned
}

fn render_provider_health(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
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
    let rows = hover_table_rows(
        rows,
        mouse_target,
        Panel::ProviderHealth,
        state.theme.selected_style(),
        1,
    );
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

fn render_task_log(
    frame: &mut Frame<'_>,
    state: &AppState,
    area: Rect,
    mouse_target: Option<MouseTarget>,
) {
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
    let rows = hover_table_rows(
        rows,
        mouse_target,
        Panel::TaskLog,
        state.theme.selected_style(),
        1,
    );
    frame.render_widget(
        Table::new(rows, [Constraint::Length(10), Constraint::Min(10)])
            .header(Row::new(["status", "event"]).style(state.theme.accent_style()))
            .block(panel_block(Panel::TaskLog, state)),
        area,
    );
}

fn hover_info_lines<'line>(
    lines: Vec<Line<'line>>,
    mouse_target: Option<MouseTarget>,
    panel: Panel,
    selected_style: Style,
) -> Vec<Line<'line>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if mouse_target.is_some_and(|target| target.panel_info_row_hovered(panel, index)) {
                line.style(selected_style)
            } else {
                line
            }
        })
        .collect()
}

fn hover_table_rows(
    rows: Vec<Row<'static>>,
    mouse_target: Option<MouseTarget>,
    panel: Panel,
    selected_style: Style,
    content_row_offset: usize,
) -> Vec<Row<'static>> {
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let content_row = index + content_row_offset;
            if mouse_target.is_some_and(|target| target.panel_info_row_hovered(panel, content_row))
            {
                row.style(selected_style)
            } else {
                row
            }
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_only_panel_view;
    use crate::theme::ThemeConfig;
    use agent_finance_market::research_snapshot::{
        PredictionMarketSnapshot, ResearchContextSnapshot, ResearchNewsSnapshot,
    };

    #[test]
    fn research_lines_do_not_duplicate_prediction_market_signals() {
        let snapshot = research_snapshot();
        let text = joined_lines(read_only_panel_view::research_lines(
            &snapshot,
            &ThemeConfig::default(),
        ));

        assert!(text.contains("news=1"));
        assert!(text.contains("news AI optics demand"));
        assert!(!text.contains("market "));
        assert!(!text.contains("markets=1"));
    }

    #[test]
    fn prediction_market_lines_show_probability_and_market_depth() {
        let snapshot = research_snapshot();
        let text = joined_lines(read_only_panel_view::prediction_market_lines(
            &snapshot,
            &ThemeConfig::default(),
        ));

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
        let research = joined_lines(read_only_panel_view::research_lines(&snapshot, &theme));
        let polymarket = joined_lines(read_only_panel_view::prediction_market_lines(
            &snapshot, &theme,
        ));

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
