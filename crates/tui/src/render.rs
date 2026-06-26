use ratatui::Frame;
use ratatui::widgets::Clear;

use crate::layout;
use crate::state::AppState;

mod chrome;
mod history;
mod panels;
mod provider_health;
mod widgets;

use chrome::{render_floating, render_status};
use panels::render_docked;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ActionId;
    use crate::config::TuiConfig;
    use crate::model::{FloatingKind, WorkspaceKind};
    use crate::theme::{ThemeColor, ThemeConfig};
    use agent_finance_core::{Environment, Provider, SignedReadSnapshot};
    use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
    use agent_finance_market::history_snapshot::HistorySnapshot;
    use agent_finance_market::research_snapshot::{ResearchContextSnapshot, ResearchNewsSnapshot};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
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
    fn status_bar_keeps_trading_profile_visible_at_common_width() {
        let state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            ..TuiConfig::default()
        });

        let text = render_to_text(&state, 120, 32);

        assert!(text.contains("profile: mainnet"));
    }

    #[test]
    fn account_workspace_renders_signed_account_state() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            ..TuiConfig::default()
        });
        state.reduce(crate::state::Action::AccountStarted {
            generation: 1,
            profile: "mainnet".to_string(),
        });
        state.reduce(crate::state::Action::AccountLoaded {
            generation: 1,
            snapshot: crate::AccountSnapshot::new(
                "mainnet".to_string(),
                Provider::Binance,
                Environment::Live,
                crate::account::ACCOUNT_READ_PLAN
                    .into_iter()
                    .map(|plan| {
                        SignedReadSnapshot::new(
                            "mainnet",
                            Provider::Binance,
                            Environment::Live,
                            plan.request(),
                            serde_json::json!({ "ok": true }),
                        )
                    })
                    .collect(),
                Vec::new(),
            ),
        });

        let text = render_to_text_grid(&state, 180, 40);

        assert!(text.contains("Account"));
        assert!(text.contains("provider: binance"));
        assert!(text.contains("environment: live"));
        assert!(text.contains("signed reads: 3 ok / 0 warning"));
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
    fn task_log_renders_task_queue_statuses() {
        let mut state = snapshot_state();

        state.reduce(crate::state::Action::HistoryStarted {
            generation: 1,
            symbol: "CRDO".to_string(),
        });
        state.reduce(crate::state::Action::HistoryLoaded {
            generation: 1,
            snapshot: history_snapshot("CRDO"),
        });
        state.reduce(crate::state::Action::ResearchStarted {
            generation: 2,
            symbol: "CRDO".to_string(),
        });
        let mut research = research_snapshot();
        research.errors = vec!["news: provider timeout".to_string()];
        state.reduce(crate::state::Action::ResearchLoaded {
            generation: 2,
            snapshot: research,
        });
        state.reduce(crate::state::Action::RefreshStarted(3));
        state.reduce(crate::state::Action::RefreshFailed {
            generation: 3,
            error: "provider timeout".to_string(),
        });
        state.reduce(crate::state::Action::EvidenceStarted {
            generation: 4,
            symbol: "CRDO".to_string(),
        });

        let text = render_to_text_grid(&state, 140, 40);

        assert!(text.contains("Task Log [fresh]"));
        assert!(text.contains("status"));
        assert!(text.contains("running"));
        assert!(text.contains("succeeded"));
        assert!(text.contains("warning"));
        assert!(text.contains("failed"));
        assert!(!text.contains("CRDO history loading"));
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
    fn command_palette_selection_uses_configured_theme_style() {
        let mut state = AppState::from_config(TuiConfig {
            theme: ThemeConfig {
                selection_foreground: ThemeColor::White,
                selection_background: ThemeColor::Magenta,
                ..ThemeConfig::default()
            },
            ..TuiConfig::default()
        });
        state.reduce(crate::state::Action::Execute(ActionId::OpenFloating(
            FloatingKind::CommandPalette,
        )));

        let buffer = render_to_buffer(&state, 100, 30);

        assert!(buffer.content().iter().any(|cell| {
            cell.symbol() == ">" && cell.fg == Color::White && cell.bg == Color::Magenta
        }));
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
            prediction_markets: Vec::new(),
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

    fn render_to_text_grid(state: &AppState, width: u16, height: u16) -> String {
        let buffer = render_to_buffer(state, width, height);

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

    fn render_to_buffer(state: &AppState, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, state)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn render_to_text(state: &AppState, width: u16, height: u16) -> String {
        render_to_buffer(state, width, height)
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }
}
