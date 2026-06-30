use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::research_snapshot::{PredictionMarketSnapshot, ResearchContextSnapshot};

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::command::ActionId;
use crate::model::Panel;
use crate::mouse_target::MouseTarget;
use crate::panel_action_line_view::{
    PanelActionLine, PanelActionSpan, RenderedPanelActionLine, render_panel_action_line,
};
use crate::provider_health::ProviderHealthReport;
use crate::state::AppState;
use crate::theme::ThemeConfig;

use crate::render::widgets::{compact_text, format_price, format_volume};

pub(crate) fn info_row_at_content_row(
    state: &AppState,
    panel: Panel,
    area: Rect,
    content_row: usize,
) -> Option<usize> {
    match panel {
        Panel::Quote => {
            info_line_at_content_row_after_actions(panel, &quote_lines(state), area, content_row)
        }
        Panel::History => history_info_row_at_content_row(state, area, content_row),
        Panel::Evidence => info_line_at_content_row_after_actions(
            panel,
            &evidence_panel_lines(state),
            area,
            content_row,
        ),
        Panel::Polymarket => info_line_at_content_row_after_actions(
            panel,
            &polymarket_panel_lines(state),
            area,
            content_row,
        ),
        Panel::Research => info_line_at_content_row_after_actions(
            panel,
            &research_panel_lines(state),
            area,
            content_row,
        ),
        Panel::RiskAudit => info_line_at_content_row(
            &crate::render::risk_audit::risk_audit_lines(state),
            area,
            content_row,
        ),
        Panel::ProviderHealth => {
            table_row_at_content_row(provider_health_row_count(state, area), content_row)
        }
        Panel::TaskLog => table_row_at_content_row(task_log_row_count(state, area), content_row),
        Panel::Watchlist
        | Panel::OrderTicket
        | Panel::OpenOrders
        | Panel::IntentReview
        | Panel::Account
        | Panel::TransferTicket
        | Panel::FuturesState
        | Panel::Settings
        | Panel::ProfileRisk => None,
    }
}

pub(crate) fn panel_action_line(
    state: &AppState,
    panel: Panel,
    width: u16,
    content_height: u16,
    mouse_target: Option<MouseTarget>,
) -> Option<RenderedPanelActionLine> {
    if action_row_count(panel, content_height) == 0 {
        return None;
    }
    let mut action_line = PanelActionLine::new("actions", width);
    action_line.push_visible_text("  ");
    match panel {
        Panel::Quote => {
            action_line.push_visible_action("[refresh]", ActionId::RefreshMarketSnapshot)
        }
        Panel::History => {
            action_line.push_visible_action("[refresh]", ActionId::RefreshSelectedHistory)
        }
        Panel::Evidence => {
            action_line.push_visible_action("[refresh]", ActionId::RefreshSelectedEvidence)
        }
        Panel::Polymarket | Panel::Research => {
            action_line.push_visible_action("[refresh]", ActionId::RefreshSelectedResearch)
        }
        _ => return None,
    }
    Some(render_panel_action_line(
        &action_line,
        &state.theme,
        panel,
        mouse_target,
    ))
}

pub(crate) fn panel_action_at_content_cell(
    state: &AppState,
    panel: Panel,
    area: Rect,
    content_row: usize,
    content_column: u16,
) -> Option<PanelActionSpan> {
    if content_row != 0 {
        return None;
    }
    panel_action_line(
        state,
        panel,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
        None,
    )?
    .actions
    .into_iter()
    .find(|span| (span.start..span.end).contains(&content_column))
}

fn history_info_row_at_content_row(
    state: &AppState,
    area: Rect,
    content_row: usize,
) -> Option<usize> {
    let content_row = content_row.checked_sub(action_row_count(
        Panel::History,
        area.height.saturating_sub(2),
    ))?;
    let text_height = history_text_area_height(area);
    if content_row >= text_height {
        return None;
    }
    info_line_at_content_row(&history_summary_lines(state), area, content_row)
}

pub(crate) fn history_chart_area(panel_area: Rect) -> Rect {
    let inner = Rect {
        x: panel_area.x.saturating_add(1),
        y: panel_area.y.saturating_add(1),
        width: panel_area.width.saturating_sub(2),
        height: panel_area.height.saturating_sub(2),
    };
    Rect {
        x: inner.x,
        y: inner
            .y
            .saturating_add(history_text_area_height(panel_area) as u16),
        width: inner.width,
        height: inner
            .height
            .saturating_sub(history_text_area_height(panel_area) as u16),
    }
}

fn info_line_at_content_row_after_actions(
    panel: Panel,
    lines: &[Line<'_>],
    area: Rect,
    content_row: usize,
) -> Option<usize> {
    let content_row =
        content_row.checked_sub(action_row_count(panel, area.height.saturating_sub(2)))?;
    info_line_at_content_row(lines, area, content_row)
}

fn action_row_count(panel: Panel, content_height: u16) -> usize {
    (content_height >= 3
        && matches!(
            panel,
            Panel::Quote | Panel::History | Panel::Evidence | Panel::Polymarket | Panel::Research
        )) as usize
}

fn info_line_at_content_row(lines: &[Line<'_>], area: Rect, content_row: usize) -> Option<usize> {
    let width = panel_text_width(area);
    let mut visual_row = 0;
    for (index, line) in lines.iter().enumerate() {
        let line_height = wrapped_line_height(line, width);
        if content_row < visual_row + line_height {
            return Some(index);
        }
        visual_row += line_height;
    }
    None
}

fn panel_text_width(area: Rect) -> usize {
    usize::from(area.width.saturating_sub(2).max(1))
}

fn wrapped_line_height(line: &Line<'_>, width: usize) -> usize {
    Paragraph::new(vec![line.clone()])
        .wrap(Wrap { trim: true })
        .line_count(width as u16)
        .max(1)
}

pub(crate) fn quote_lines(state: &AppState) -> Vec<Line<'_>> {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let quote = state
        .market_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.quote_for(symbol));

    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            state
                .theme
                .accent_style()
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::raw(if state.refresh_loading() {
            " refreshing..."
        } else {
            " market snapshot"
        }),
    ])];
    match quote {
        Some(quote) => lines.extend(quote_detail_lines(quote)),
        None => lines.push(Line::from(
            "No quote loaded yet. Waiting for the next refresh.",
        )),
    }
    if let Some(snapshot) = state.market_snapshot.as_ref() {
        if let Some(fetched_at) = snapshot.fetched_at_local.as_ref() {
            lines.push(Line::from(format!("freshness: {fetched_at}")));
        }
        for error in snapshot.errors.iter().take(2) {
            lines.push(Line::from(Span::styled(
                format!("provider error: {error}"),
                state.theme.warning_style(),
            )));
        }
    }
    lines
}

pub(crate) fn history_summary_lines(state: &AppState) -> Vec<Line<'_>> {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    let snapshot = state.history.selected_snapshot(symbol);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            symbol,
            state
                .theme
                .accent_style()
                .add_modifier(ratatui::style::Modifier::BOLD),
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
                "provider: {}  preset={}  interval={}  bars={}",
                snapshot.provider,
                state.chart.preset(),
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

    lines
}

fn history_text_area_height(area: Rect) -> usize {
    area.height.saturating_sub(2).min(5).into()
}

pub(crate) fn evidence_panel_lines(state: &AppState) -> Vec<Line<'static>> {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    match state.evidence.selected_snapshot(symbol) {
        Some(snapshot) => {
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.evidence.loading() {
                    " evidence loading..."
                } else {
                    " evidence"
                }),
            ])];
            lines.extend(evidence_lines(snapshot, &state.theme));
            lines
        }
        None => vec![
            Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.evidence.loading() {
                    " evidence loading..."
                } else {
                    " evidence"
                }),
            ]),
            Line::from("No crypto evidence loaded yet. Waiting for the selected symbol."),
        ],
    }
}

pub(crate) fn research_panel_lines(state: &AppState) -> Vec<Line<'static>> {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    match state.research.selected_snapshot(symbol) {
        Some(snapshot) => {
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.research.loading() {
                    " research loading..."
                } else {
                    " research"
                }),
            ])];
            lines.extend(research_lines(snapshot, &state.theme));
            lines
        }
        None => vec![
            Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.research.loading() {
                    " research loading..."
                } else {
                    " research"
                }),
            ]),
            Line::from("No research context loaded yet. Waiting for the selected symbol."),
        ],
    }
}

pub(crate) fn polymarket_panel_lines(state: &AppState) -> Vec<Line<'static>> {
    let symbol = state.selected_symbol().unwrap_or("N/A");
    match state.research.selected_snapshot(symbol) {
        Some(snapshot) => {
            let mut lines = vec![Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.research.loading() {
                    " prediction signals loading..."
                } else {
                    " prediction signals"
                }),
            ])];
            lines.extend(prediction_market_lines(snapshot, &state.theme));
            lines
        }
        None => vec![
            Line::from(vec![
                Span::styled(
                    symbol.to_string(),
                    state
                        .theme
                        .accent_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::raw(if state.research.loading() {
                    " prediction signals loading..."
                } else {
                    " prediction signals"
                }),
            ]),
            Line::from("No prediction market context loaded yet. Waiting for research refresh."),
        ],
    }
}

fn quote_detail_lines(quote: &agent_finance_market::snapshot::QuoteSnapshot) -> Vec<Line<'static>> {
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

pub(crate) fn research_lines(
    snapshot: &ResearchContextSnapshot,
    theme: &ThemeConfig,
) -> Vec<Line<'static>> {
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

pub(crate) fn prediction_market_lines(
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
        Span::styled("market ", theme.prediction_style()),
        Span::raw(format!(
            "{probability} vol={volume} liq={liquidity} {}{url}",
            compact_text(&market.title, 62)
        )),
    ])
}

fn provider_health_row_count(state: &AppState, area: Rect) -> usize {
    let report = ProviderHealthReport::from_state(state);
    let count = if report.is_empty() {
        state.provider_profiles.iter().take(8).count()
    } else {
        report.providers.len() + report.tasks.len()
    };
    count.min(area.height.saturating_sub(3) as usize)
}

fn task_log_row_count(state: &AppState, area: Rect) -> usize {
    state
        .task_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(3) as usize)
        .count()
}

fn table_row_at_content_row(row_count: usize, content_row: usize) -> Option<usize> {
    let row_index = content_row.checked_sub(1)?;
    (row_index < row_count).then_some(content_row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_chart_rows_are_not_info_targets() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let area = Rect::new(0, 0, 80, 20);

        assert_eq!(
            info_row_at_content_row(&state, Panel::History, area, 6),
            None
        );
    }

    #[test]
    fn quote_text_rows_are_info_targets() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let area = Rect::new(0, 0, 80, 20);

        assert_eq!(info_row_at_content_row(&state, Panel::Quote, area, 0), None);
        assert_eq!(
            info_row_at_content_row(&state, Panel::Quote, area, 1),
            Some(0)
        );
    }

    #[test]
    fn wrapped_quote_text_rows_keep_the_same_info_target() {
        let state = AppState::from_config(crate::config::TuiConfig::default());
        let area = Rect::new(0, 0, 24, 20);

        assert_eq!(
            info_row_at_content_row(&state, Panel::Quote, area, 2),
            Some(1)
        );
        assert_eq!(
            info_row_at_content_row(&state, Panel::Quote, area, 3),
            Some(1)
        );
    }

    #[test]
    fn word_wrapped_info_rows_match_ratatui_visual_rows_before_next_item() {
        let lines = [Line::from("aaa aaa aaa aaa"), Line::from("next item")];
        let area = Rect::new(0, 0, 7, 20);

        assert_eq!(info_line_at_content_row(&lines, area, 3), Some(0));
        assert_eq!(info_line_at_content_row(&lines, area, 4), Some(1));
    }
}
