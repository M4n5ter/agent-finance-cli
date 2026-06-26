use super::*;
use crate::command::ActionId;
use crate::config::MAX_LEFT_MAIN_RATIO;
use crate::model::InteractionMode;
use crate::task_log::TaskStatus;
use agent_finance_market::crypto_evidence_snapshot::CryptoQuoteEvidenceSnapshot;
use agent_finance_market::history_snapshot::HistorySnapshot;
use agent_finance_market::research_snapshot::ResearchContextSnapshot;
use agent_finance_market::snapshot::{QuoteSnapshot, RegularBasisSnapshot};

fn toggle_panel_action(panel: Panel) -> ActionId {
    ActionId::TogglePanel(panel)
}

#[test]
fn reducer_wraps_symbol_focus_across_watchlist_boundaries() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
        ..TuiConfig::default()
    });

    state.reduce(Action::Execute(ActionId::SelectSymbolBy(-1)));

    assert_eq!(state.selected_symbol(), Some("CRDO"));

    state.reduce(Action::Execute(ActionId::SelectSymbolBy(1)));

    assert_eq!(state.selected_symbol(), Some("AAPL"));
}

#[test]
fn floating_panes_use_vec_order_as_top_order() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));

    assert_eq!(state.floating[0].kind, FloatingKind::Help);
    assert_eq!(state.floating[1].kind, FloatingKind::CommandPalette);

    state.reduce(Action::CloseFocusedFloating);

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn floating_panes_can_be_focused_and_resized() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::ProviderDetails,
    )));
    state.reduce(Action::FocusFloating(FloatingKind::Help));

    assert_eq!(state.floating.last().unwrap().kind, FloatingKind::Help);

    let size = FloatingSize::resized(82, 63);
    state.reduce(Action::ResizeFloating {
        kind: FloatingKind::Help,
        size,
    });

    let help = state
        .floating
        .iter()
        .find(|pane| pane.kind == FloatingKind::Help)
        .unwrap();
    assert_eq!(help.size, size);
}

#[test]
fn interaction_mode_follows_top_floating_pane() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.interaction_mode(), InteractionMode::Normal);

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    assert_eq!(state.interaction_mode(), InteractionMode::Help);

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    assert_eq!(state.interaction_mode(), InteractionMode::Command);

    state.reduce(Action::CloseFocusedFloating);
    assert_eq!(state.interaction_mode(), InteractionMode::Help);
}

#[test]
fn workspace_switching_keeps_focus_visible() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::Evidence));

    assert_eq!(state.panels.focused(), Panel::Evidence);

    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.visible_panels().contains(&state.panels.focused()));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
}

#[test]
fn pane_focus_navigation_wraps_visible_workspace_panels() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Research,
        },
        ..TuiConfig::default()
    });

    assert_eq!(state.panels.focused(), Panel::Watchlist);
    state.reduce(Action::FocusPanelBy(1));
    assert_eq!(state.panels.focused(), Panel::Quote);
    state.reduce(Action::FocusPanelBy(-1));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    state.reduce(Action::FocusPanelBy(-1));
    assert_eq!(state.panels.focused(), Panel::TaskLog);
}

#[test]
fn pane_focus_navigation_uses_workspace_declared_order() {
    let mut state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Providers,
        },
        ..TuiConfig::default()
    });

    assert_eq!(
        state.workspace_panels(),
        vec![
            Panel::Watchlist,
            Panel::ProviderHealth,
            Panel::TaskLog,
            Panel::Quote
        ]
    );
    state.reduce(Action::FocusPanelBy(1));
    assert_eq!(state.panels.focused(), Panel::ProviderHealth);
}

#[test]
fn pane_zoom_limits_visible_panels_without_trapping_focus_navigation() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Focus(Panel::History));
    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    assert_eq!(state.visible_panels(), vec![Panel::History]);

    state.reduce(Action::FocusPanelBy(1));
    assert!(state.zoomed);
    assert_eq!(state.panels.focused(), Panel::ProviderHealth);
    assert_eq!(state.visible_panels(), vec![Panel::ProviderHealth]);

    state.reduce(Action::ToggleFocusedZoom);
    assert!(!state.zoomed);
    assert!(state.visible_panels().len() > 1);
}

#[test]
fn zoom_does_not_turn_hidden_open_panels_into_focus_actions() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Focus(Panel::History));
    state.reduce(Action::ToggleFocusedZoom);
    assert_eq!(state.visible_panels(), vec![Panel::History]);
    assert!(state.is_open_in_workspace(Panel::Quote));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Quote)));

    assert!(!state.panels.contains(Panel::Quote));
    assert!(!state.zoomed);
    assert_eq!(state.panels.focused(), Panel::History);
}

#[test]
fn workspace_and_layout_restore_leave_zoom_mode() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));
    assert!(!state.zoomed);

    state.reduce(Action::ToggleFocusedZoom);
    assert!(state.zoomed);
    state.reduce(Action::RestorePanels);
    assert!(!state.zoomed);
}

#[test]
fn inconsistent_persisted_workspace_config_is_normalized_on_load() {
    let state = AppState::from_config(TuiConfig {
        workspace: WorkspaceConfig {
            current: WorkspaceKind::Research,
        },
        panels: PanelConfig {
            open: vec![Panel::History, Panel::Evidence],
            focused: Panel::History,
        },
        ..TuiConfig::default()
    });

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.panels.contains(Panel::History));
    assert!(state.panels.contains(Panel::Evidence));
    assert!(state.panels.contains(Panel::Watchlist));
    assert_eq!(state.panels.focused(), Panel::Watchlist);
    assert_eq!(state.visible_panels(), vec![Panel::Watchlist]);
}

#[test]
fn closing_every_visible_workspace_panel_reopens_workspace_default() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));

    for panel in WorkspaceKind::Research.panels() {
        state.reduce(Action::Focus(*panel));
        state.reduce(Action::CloseFocusedPanel);
    }

    assert!(!state.visible_panels().is_empty());
    assert_eq!(
        state.panels.focused(),
        WorkspaceKind::Research.default_panel()
    );
    assert!(
        state
            .visible_panels()
            .contains(&WorkspaceKind::Research.default_panel())
    );
}

#[test]
fn focusing_hidden_panel_moves_to_a_workspace_that_can_show_it() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.workspace, WorkspaceKind::Overview);

    state.reduce(Action::Focus(Panel::Polymarket));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert_eq!(state.panels.focused(), Panel::Polymarket);
    assert!(state.visible_panels().contains(&Panel::Polymarket));
}

#[test]
fn command_palette_show_panel_routes_to_visible_workspace() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::SetWorkspace(WorkspaceKind::Research));
    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Polymarket)));
    assert!(!state.panels.contains(Panel::Polymarket));

    state.reduce(Action::SetWorkspace(WorkspaceKind::Overview));
    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Polymarket)));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert_eq!(state.panels.focused(), Panel::Polymarket);
    assert!(state.visible_panels().contains(&Panel::Polymarket));
}

#[test]
fn command_palette_toggle_hidden_open_panel_routes_to_visible_workspace() {
    let mut state = AppState::from_config(TuiConfig::default());
    assert_eq!(state.workspace, WorkspaceKind::Overview);
    assert!(state.panels.contains(Panel::Research));
    assert!(!state.visible_panels().contains(&Panel::Research));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::Research)));

    assert_eq!(state.workspace, WorkspaceKind::Research);
    assert!(state.panels.contains(Panel::Research));
    assert_eq!(state.panels.focused(), Panel::Research);
    assert!(state.visible_panels().contains(&Panel::Research));
}

#[test]
fn command_palette_executes_workspace_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::SetWorkspace(
        WorkspaceKind::Crypto,
    )));

    assert_eq!(state.workspace, WorkspaceKind::Crypto);
    assert!(state.floating.is_empty());
    assert!(state.visible_panels().contains(&state.panels.focused()));
}

#[test]
fn command_palette_wraps_selection_and_executes_overlay_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::MoveCommandSelection(-1));
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::CloseCommandPalette)
    );

    for character in "open help".chars() {
        state.reduce(Action::EditCommandQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::OpenFloating(FloatingKind::Help))
    );

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn command_palette_query_filters_executable_actions() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    for character in "crypto".chars() {
        state.reduce(Action::EditCommandQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }

    assert_eq!(state.command_palette.query(), "crypto");
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::SetWorkspace(WorkspaceKind::Crypto))
    );
}

#[test]
fn command_palette_query_resets_after_close() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::EditCommandQuery(
        tui_input::InputRequest::InsertChar('z'),
    ));
    state.reduce(Action::CloseFocusedFloating);

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));

    assert_eq!(state.command_palette.query(), "");
    assert_eq!(
        state.command_palette.len(),
        crate::command::ACTION_REGISTRY
            .iter()
            .filter(|action| action.command().is_some())
            .count()
    );
    assert_eq!(
        state.command_palette.selected_action(),
        Some(ActionId::SelectSymbolBy(1))
    );
}

#[test]
fn symbol_search_selects_watchlist_symbols_and_resets_on_close() {
    let mut state = AppState::from_config(TuiConfig {
        watchlist: vec![
            "AAPL".to_string(),
            "CRDO".to_string(),
            "BTCUSDT".to_string(),
        ],
        ..TuiConfig::default()
    });
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::SymbolSearch,
    )));

    for character in "btc".chars() {
        state.reduce(Action::EditSymbolSearchQuery(
            tui_input::InputRequest::InsertChar(character),
        ));
    }
    state.reduce(Action::AcceptSymbolSearch);

    assert_eq!(state.selected_symbol(), Some("BTCUSDT"));
    assert_eq!(state.interaction_mode(), InteractionMode::Normal);
    assert_eq!(state.symbol_search.query(), "");

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::SymbolSearch,
    )));
    state.reduce(Action::EditSymbolSearchQuery(
        tui_input::InputRequest::InsertChar('c'),
    ));
    state.reduce(Action::CloseFocusedFloating);

    assert_eq!(state.symbol_search.query(), "");
}

#[test]
fn command_palette_executes_panel_focus_commands() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::FocusPanel(Panel::Research)));

    assert_eq!(state.panels.focused(), Panel::Research);
    assert!(state.floating.is_empty());
}

#[test]
fn command_palette_close_preserves_underlying_overlay() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::Execute(ActionId::CloseCommandPalette));

    assert_eq!(state.floating.len(), 1);
    assert_eq!(state.floating[0].kind, FloatingKind::Help);
}

#[test]
fn panel_lifecycle_closes_focused_panel_and_restores_all_panels() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::Research));

    state.reduce(Action::CloseFocusedPanel);

    assert!(!state.panels.contains(Panel::Research));
    assert_ne!(state.panels.focused(), Panel::Research);
    assert!(state.panels.contains(state.panels.focused()));

    state.reduce(Action::RestorePanels);

    assert!(
        Panel::ALL
            .into_iter()
            .all(|panel| state.panels.contains(panel))
    );
    assert_eq!(state.panels.open_panels().len(), Panel::ALL.len());
}

#[test]
fn panel_lifecycle_toggles_panels_without_closing_the_last_one() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert!(!state.panels.contains(Panel::History));

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert!(state.panels.contains(Panel::History));
    assert_eq!(state.panels.focused(), Panel::History);

    for panel in [
        Panel::Watchlist,
        Panel::Quote,
        Panel::ProviderHealth,
        Panel::TaskLog,
    ] {
        state.reduce(Action::Execute(toggle_panel_action(panel)));
    }
    assert_eq!(state.visible_panels(), vec![Panel::History]);

    state.reduce(Action::Execute(ActionId::TogglePanel(Panel::History)));
    assert_eq!(state.visible_panels(), vec![Panel::Watchlist]);
}

#[test]
fn state_exports_user_layout_preferences_to_config() {
    let mut state = AppState::from_config(TuiConfig::default());
    state.reduce(Action::Focus(Panel::Research));
    state.reduce(Action::CloseFocusedPanel);
    state.reduce(Action::ResizeDockedColumns {
        left_ratio: 31,
        main_ratio: 42,
    });
    state.reduce(Action::Execute(ActionId::OpenFloating(FloatingKind::Help)));
    state.reduce(Action::Execute(ActionId::OpenFloating(
        FloatingKind::CommandPalette,
    )));
    state.reduce(Action::ResizeFloating {
        kind: FloatingKind::Help,
        size: FloatingSize::resized(82, 63),
    });

    let config = state.export_config(&TuiConfig::default());

    assert_eq!(config.layout.left_ratio, 31);
    assert_eq!(config.layout.main_ratio, 42);
    assert!(!config.panels.open.contains(&Panel::Research));
    assert!(config.panels.open.contains(&Panel::Watchlist));
    assert_ne!(config.panels.focused, Panel::Research);
    assert_eq!(config.floating.panes.len(), 1);
    assert_eq!(config.floating.panes[0].kind, FloatingKind::Help);
    assert_eq!(config.floating.panes[0].size, FloatingSize::resized(82, 63));
}

#[test]
fn reducer_resizes_and_resets_docked_layout() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ResizeDockedColumns {
        left_ratio: 8,
        main_ratio: 80,
    });
    assert_eq!(state.layout.left_ratio, 15);
    assert_eq!(state.layout.main_ratio, 60);
    assert!(state.layout.left_ratio + state.layout.main_ratio <= MAX_LEFT_MAIN_RATIO);

    state.reduce(Action::ResetLayout);
    assert_eq!(state.layout, LayoutConfig::default());
}

#[test]
fn reducer_accepts_current_snapshot_and_ignores_stale_snapshot() {
    let mut state = AppState::from_config(TuiConfig::default());
    let current = snapshot(2, "CRDO");
    let stale = snapshot(1, "AAPL");

    state.reduce(Action::RefreshStarted(2));
    state.reduce(Action::SnapshotLoaded {
        generation: 1,
        snapshot: stale,
    });
    assert!(state.market_snapshot.is_none());
    assert!(state.refresh_loading());

    state.reduce(Action::SnapshotLoaded {
        generation: 2,
        snapshot: current,
    });
    assert_eq!(
        state
            .market_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.quote_for("CRDO"))
            .and_then(|quote| quote.price),
        Some(250.0)
    );
    assert!(!state.refresh_loading());
}

#[test]
fn reducer_clears_in_flight_refresh_on_scheduler_fatal_failure() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::RefreshStarted(1));
    state.reduce(Action::HistoryStarted {
        generation: 1,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::EvidenceStarted {
        generation: 1,
        symbol: "BTCUSDT".to_string(),
    });
    state.reduce(Action::ResearchStarted {
        generation: 1,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::SchedulerFailed(
        "scheduler runtime failed".to_string(),
    ));

    assert!(!state.refresh_loading());
    assert!(!state.history.loading());
    assert!(!state.evidence.loading());
    assert!(!state.research.loading());
    assert_eq!(
        state.scheduler_error.as_deref(),
        Some("scheduler runtime failed")
    );

    state.reduce(Action::SnapshotLoaded {
        generation: 1,
        snapshot: snapshot(1, "CRDO"),
    });
    state.reduce(Action::HistoryLoaded {
        generation: 1,
        snapshot: history_snapshot("CRDO", 250.0),
    });
    state.reduce(Action::EvidenceLoaded {
        generation: 1,
        snapshot: evidence_snapshot("BTCUSDT", 2, 3),
    });
    state.reduce(Action::ResearchLoaded {
        generation: 1,
        snapshot: research_snapshot("CRDO", 1, 1),
    });

    assert!(state.market_snapshot.is_none());
    assert!(state.history.selected_snapshot("CRDO").is_none());
    assert!(state.evidence.selected_snapshot("BTCUSDT").is_none());
    assert!(state.research.selected_snapshot("CRDO").is_none());
    assert!(state.task_log.iter().any(|entry| {
        entry.status == TaskStatus::Failed
            && entry.message == "CRDO history loading cancelled: scheduler runtime failed"
    }));
}

#[test]
fn reducer_accepts_current_history_and_ignores_stale_history() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::HistoryStarted {
        generation: 2,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::HistoryLoaded {
        generation: 1,
        snapshot: history_snapshot("AAPL", 100.0),
    });
    assert!(state.history.selected_snapshot("AAPL").is_none());
    assert!(state.history.loading());

    state.reduce(Action::HistoryLoaded {
        generation: 2,
        snapshot: history_snapshot("CRDO", 250.0),
    });
    assert_eq!(
        state
            .history
            .selected_snapshot("CRDO")
            .and_then(|snapshot| snapshot.latest_close),
        Some(250.0)
    );
    assert!(!state.history.loading());
}

#[test]
fn reducer_accepts_current_evidence_and_ignores_stale_evidence() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::EvidenceStarted {
        generation: 2,
        symbol: "BTCUSDT".to_string(),
    });
    state.reduce(Action::EvidenceLoaded {
        generation: 1,
        snapshot: evidence_snapshot("ETHUSDT", 1, 2),
    });
    assert!(state.evidence.selected_snapshot("ETHUSDT").is_none());
    assert!(state.evidence.loading());

    state.reduce(Action::EvidenceLoaded {
        generation: 2,
        snapshot: evidence_snapshot("BTCUSDT", 2, 3),
    });
    assert_eq!(
        state
            .evidence
            .selected_snapshot("BTCUSDT")
            .map(|snapshot| (snapshot.ok_providers, snapshot.total_providers)),
        Some((2, 3))
    );
    assert!(!state.evidence.loading());
}

#[test]
fn reducer_accepts_current_research_and_ignores_stale_research() {
    let mut state = AppState::from_config(TuiConfig::default());

    state.reduce(Action::ResearchStarted {
        generation: 2,
        symbol: "CRDO".to_string(),
    });
    state.reduce(Action::ResearchLoaded {
        generation: 1,
        snapshot: research_snapshot("AAPL", 1, 1),
    });
    assert!(state.research.selected_snapshot("AAPL").is_none());
    assert!(state.research.loading());

    state.reduce(Action::ResearchLoaded {
        generation: 2,
        snapshot: research_snapshot("CRDO", 2, 3),
    });
    assert_eq!(
        state
            .research
            .selected_snapshot("CRDO")
            .map(|snapshot| (snapshot.news.len(), snapshot.prediction_markets.len())),
        Some((2, 3))
    );
    assert!(!state.research.loading());
}

fn snapshot(_generation: u64, symbol: &str) -> MarketSnapshot {
    MarketSnapshot {
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        quotes: vec![QuoteSnapshot {
            symbol: symbol.to_string(),
            price: Some(250.0),
            currency: Some("USD".to_string()),
            provider: "test".to_string(),
            session: Some("regular".to_string()),
            market_time_local: None,
            change_pct: Some(1.0),
            aliases: Vec::new(),
            regular_basis: RegularBasisSnapshot {
                previous_close: Some(247.0),
                open: None,
                high: None,
                low: None,
                volume: None,
            },
        }],
        errors: Vec::new(),
    }
}

fn history_snapshot(symbol: &str, latest_close: f64) -> HistorySnapshot {
    HistorySnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        provider: "test".to_string(),
        interval: "1d".to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        latest_close: Some(latest_close),
        latest_time: Some("2026-06-25".to_string()),
        return_pct: Some(1.0),
        volume: Some(10_000.0),
        bars: Vec::new(),
        errors: Vec::new(),
    }
}

fn evidence_snapshot(
    symbol: &str,
    ok_providers: usize,
    total_providers: usize,
) -> CryptoQuoteEvidenceSnapshot {
    CryptoQuoteEvidenceSnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        instrument: "spot".to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        ok_providers,
        total_providers,
        providers: Vec::new(),
        errors: Vec::new(),
    }
}

fn research_snapshot(
    symbol: &str,
    news_count: usize,
    prediction_count: usize,
) -> ResearchContextSnapshot {
    ResearchContextSnapshot {
        requested_symbol: symbol.to_string(),
        symbol: symbol.to_string(),
        fetched_at_local: Some("2026-06-25 09:30:00".to_string()),
        news: (0..news_count)
            .map(
                |index| agent_finance_market::research_snapshot::ResearchNewsSnapshot {
                    title: format!("headline {index}"),
                    provider: "test".to_string(),
                    module: "news".to_string(),
                },
            )
            .collect(),
        prediction_markets: (0..prediction_count)
            .map(
                |index| agent_finance_market::research_snapshot::PredictionMarketSnapshot {
                    title: format!("market {index}"),
                    probability: Some(0.5),
                    volume: Some(1000.0),
                    liquidity: None,
                    market_url: None,
                },
            )
            .collect(),
        errors: Vec::new(),
    }
}
