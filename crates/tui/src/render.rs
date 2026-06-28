use ratatui::Frame;
use ratatui::widgets::Clear;

use crate::layout;
use crate::state::AppState;

mod account;
mod chrome;
mod history;
mod panels;
mod provider_health;
mod settings;
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
    use agent_finance_core::{
        Environment, Market, Provider, SignedReadRequest, SignedReadSnapshot,
    };
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
        assert!(wide.contains("live:off"));

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
        assert!(text.contains("dry-run"));
    }

    #[test]
    fn live_writes_confirmation_overlay_renders_explicit_gate() {
        let mut state = AppState::from_config(TuiConfig::default());
        state.reduce(crate::state::Action::Execute(ActionId::ToggleLiveWrites));

        let text = render_to_text_grid(&state, 120, 32);

        assert!(text.contains("Enable Live Writes"));
        assert!(text.contains("Enter: enable live writes for this session"));
        assert!(text.contains("Esc: keep live writes disabled"));
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
        let mut profile_snapshot = crate::profile_snapshot::test_trading_profile_snapshot();
        profile_snapshot.declared_permissions.clear();
        profile_snapshot.missing_permissions = profile_snapshot.required_permissions.clone();
        state.reduce(crate::state::Action::AccountLoaded {
            generation: 1,
            snapshot: crate::AccountSnapshot::new(
                "mainnet".to_string(),
                Provider::Binance,
                Environment::Live,
                profile_snapshot,
                crate::account::ACCOUNT_READ_PLAN
                    .into_iter()
                    .map(|plan| {
                        let request = plan.request();
                        let payload = match &request {
                            SignedReadRequest::OpenOrders {
                                market: Market::Spot,
                                ..
                            } => serde_json::json!([
                                {
                                    "symbol": "BTCUSDT",
                                    "orderId": 1001,
                                    "clientOrderId": "spot-1",
                                    "side": "BUY",
                                    "type": "LIMIT",
                                    "origQty": "0.10",
                                    "executedQty": "0.04",
                                    "price": "64000"
                                },
                                {
                                    "symbol": "ETHUSDT",
                                    "orderId": 1002,
                                    "clientOrderId": "spot-2",
                                    "side": "SELL",
                                    "type": "LIMIT",
                                    "origQty": "0.20",
                                    "executedQty": "0",
                                    "price": "3200"
                                },
                                {
                                    "symbol": "SOLUSDT",
                                    "orderId": 1003,
                                    "clientOrderId": "spot-3",
                                    "side": "BUY",
                                    "type": "LIMIT",
                                    "origQty": "1",
                                    "executedQty": "0",
                                    "price": "140"
                                }
                            ]),
                            SignedReadRequest::OpenOrders {
                                market: Market::UsdsFutures,
                                ..
                            } => serde_json::json!([
                                {
                                    "symbol": "BNBUSDT",
                                    "orderId": 2001,
                                    "clientOrderId": "futures-1",
                                    "side": "BUY",
                                    "type": "LIMIT",
                                    "origQty": "0.30",
                                    "executedQty": "0.10",
                                    "price": "600"
                                },
                                {
                                    "symbol": "XRPUSDT",
                                    "orderId": 2002,
                                    "clientOrderId": "futures-2",
                                    "side": "SELL",
                                    "type": "LIMIT",
                                    "origQty": "10",
                                    "executedQty": "0",
                                    "price": "2"
                                }
                            ]),
                            SignedReadRequest::TransferHistory { direction, .. } => {
                                let (asset, amount, status, id) = match direction {
                                    agent_finance_core::TransferDirection::SpotToUsdsFutures => {
                                        ("USDT", "12.5", "CONFIRMED", "spot-futures-1")
                                    }
                                    agent_finance_core::TransferDirection::UsdsFuturesToSpot => {
                                        ("USDC", "3", "CONFIRMED", "futures-spot-1")
                                    }
                                };
                                serde_json::json!({
                                    "total": 1,
                                    "rows": [
                                        {
                                            "asset": asset,
                                            "amount": amount,
                                            "status": status,
                                            "clientTranId": id,
                                            "timestamp": 1720000000000_u64
                                        }
                                    ]
                                })
                            }
                            _ => serde_json::json!({ "ok": true }),
                        };
                        SignedReadSnapshot::new(
                            "mainnet",
                            Provider::Binance,
                            Environment::Live,
                            request,
                            payload,
                        )
                    })
                    .collect(),
                Vec::new(),
            ),
        });

        let text = render_to_text_grid(&state, 180, 52);

        assert!(text.contains("Account"));
        assert!(text.contains("provider: binance"));
        assert!(text.contains("environment: live"));
        assert!(text.contains("risk: live:allowed"));
        assert!(text.contains("allowed symbols: btcusdt spot limit <= 50"));
        assert!(text.contains("missing profile permissions: spot_trading"));
        assert!(text.contains(&format!(
            "signed reads: {} ok / 0 warning",
            crate::account::ACCOUNT_READ_PLAN.len()
        )));
        assert!(text.contains("spot open orders: ok"));
        assert!(text.contains("USD-M open orders: ok"));
        assert!(text.contains("spot -> USD-M transfers: ok"));
        assert!(text.contains("USD-M -> spot transfers: ok"));
        assert!(text.contains("transfer ticket"));
        assert!(text.contains("direction: spot-to-usds-futures"));
        assert!(text.contains("blocked: amount is required"));
        assert!(text.contains("futures state ticket"));
        assert!(text.contains("kind: leverage"));
        assert!(text.contains("blocked: USD-M futures symbol is required"));
        assert!(text.contains("open orders (5)"));
        assert!(text.contains("> spot BUY 0.06 BTCUSDT @ 64000 [spot-1]"));
        assert!(text.contains("+1 more open orders"));
        assert!(text.contains("transfer history (2)"));
        assert!(text.contains("spot-to-usds-futures 12.5 USDT CONFIRMED [spot-futures-1]"));
        assert!(text.contains("usds-futures-to-spot 3 USDC CONFIRMED [futures-spot-1]"));

        state.selected_open_order = 4;
        let text = render_to_text_grid(&state, 180, 52);
        assert!(text.contains("+1 earlier open orders"));
        assert!(text.contains("> usds-futures SELL 10 XRPUSDT @ 2 [futures-2]"));
    }

    #[test]
    fn trade_workspace_renders_order_ticket_as_first_write_surface() {
        let state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            ..TuiConfig::default()
        });

        let text = render_to_text_grid(&state, 140, 36);

        assert!(text.contains("Order Ticket"));
        assert!(text.contains("staged order"));
        assert!(text.contains("symbol: CRDO"));
        assert!(text.contains("profile: mainnet"));
        assert!(text.contains("blocked: quantity is required"));
    }

    #[test]
    fn intent_review_renders_staged_order_change() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            ..TuiConfig::default()
        });
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));
        state.reduce(crate::state::Action::StageOrderTicket);

        let text = render_to_text_grid(&state, 160, 44);

        assert!(text.contains("Intent Review"));
        assert!(text.contains("staged intents"));
        assert!(text.contains("> ready  dry-run  order"));
        assert!(text.contains("buy 0.05 CRDO spot limit-maker @ 204"));
        assert!(text.contains("up/down/k/j select  enter submit  d/backspace close  q quit"));
    }

    #[test]
    fn staged_submit_confirmation_renders_selected_change_review() {
        let mut state = AppState::from_config(TuiConfig {
            watchlist: vec!["CRDO".to_string()],
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Trade,
            },
            ..TuiConfig::default()
        });
        state
            .order_ticket
            .set_quantity_text(Some("0.05".to_string()));
        state.order_ticket.set_price_text(Some("204".to_string()));
        state.reduce(crate::state::Action::StageOrderTicket);
        state.reduce(crate::state::Action::SubmitStagedChange);

        let text = render_to_text_grid(&state, 160, 60);

        assert!(text.contains("Confirm Staged Submit"));
        assert!(text.contains("Review the selected staged change before submitting."));
        assert!(text.contains("mode: dry-run"));
        assert!(text.contains("summary: buy 0.05 CRDO spot limit-maker @ 204"));
        assert!(text.contains("Enter: confirm submit"));
        assert!(text.contains("Esc: cancel"));
    }

    #[test]
    fn account_workspace_keeps_transfer_ticket_visible_without_snapshot() {
        let state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Account,
            },
            ..TuiConfig::default()
        });

        let text = render_to_text_grid(&state, 120, 32);

        assert!(text.contains("transfer ticket"));
        assert!(text.contains("direction: spot-to-usds-futures"));
        assert!(text.contains("blocked: amount is required"));
        assert!(text.contains("futures state ticket"));
        assert!(text.contains("blocked: USD-M futures symbol is required"));
        assert!(text.contains("No account snapshot loaded yet"));
    }

    #[test]
    fn settings_workspace_renders_configuration_cockpit() {
        let mut state = AppState::from_config(TuiConfig {
            trading: crate::config::TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            workspace: crate::config::WorkspaceConfig {
                current: WorkspaceKind::Settings,
            },
            ..TuiConfig::default()
        });
        state.config_changes.push("watchlist".to_string());

        let text = render_to_text_grid(&state, 120, 32);

        assert!(text.contains("configuration cockpit"));
        assert!(text.contains("workspace: settings"));
        assert!(text.contains("dirty config: watchlist"));
        assert!(text.contains("watchlist:"));
        assert!(text.contains("trading profile: mainnet"));
        assert!(text.contains("provider profiles:"));
        assert!(text.contains("normal key bindings:"));
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
