use crate::model::{
    FloatingKind, FloatingPane, FloatingSize, InteractionMode, Panel, WorkspaceKind,
};

use super::AppState;

impl AppState {
    pub fn selected_symbol(&self) -> Option<&str> {
        self.watchlist.get(self.selected_symbol).map(String::as_str)
    }

    pub(super) fn select_symbol_search_symbol(&mut self, index: usize) {
        if index < self.watchlist.len() {
            self.selected_symbol = index;
            self.close_floating(FloatingKind::SymbolSearch);
        }
    }

    pub(super) fn selected_quote_price(&self) -> Option<f64> {
        self.market_snapshot
            .as_ref()
            .and_then(|snapshot| {
                self.selected_symbol()
                    .and_then(|symbol| snapshot.quote_for(symbol))
            })
            .and_then(|quote| quote.price)
    }

    pub fn visible_panels(&self) -> Vec<Panel> {
        self.layout_panels()
    }

    /// Panels that should be rendered in the current layout.
    fn layout_panels(&self) -> Vec<Panel> {
        let panels = self.workspace_panels();
        if self.zoomed
            && let Some(focused) = panels
                .iter()
                .copied()
                .find(|panel| *panel == self.panels.focused())
        {
            return vec![focused];
        }
        panels
    }

    /// Open panels in the current workspace, independent of zoom.
    pub(super) fn workspace_panels(&self) -> Vec<Panel> {
        self.workspace
            .panels()
            .iter()
            .copied()
            .filter(|panel| self.panels.contains(*panel))
            .collect()
    }

    pub(super) fn is_open_in_workspace(&self, panel: Panel) -> bool {
        self.panels.contains(panel) && self.workspace_contains(panel)
    }

    pub fn interaction_mode(&self) -> InteractionMode {
        match self.floating.last().map(|pane| pane.kind) {
            Some(FloatingKind::CommandPalette) => InteractionMode::Command,
            Some(FloatingKind::Help) => InteractionMode::Help,
            Some(
                FloatingKind::LiveWritesConfirmation
                | FloatingKind::StagedExecutionConfirmation
                | FloatingKind::ProviderDetails,
            ) => InteractionMode::Inspect,
            Some(
                FloatingKind::SymbolSearch
                | FloatingKind::WatchlistAdd
                | FloatingKind::TradingProfile
                | FloatingKind::TicketTextInput,
            ) => InteractionMode::Search,
            None => InteractionMode::Normal,
        }
    }

    pub(super) fn shift_symbol(&mut self, direction: isize) {
        if self.watchlist.is_empty() {
            self.selected_symbol = 0;
            return;
        }

        let len = self.watchlist.len() as isize;
        let selected = self.selected_symbol as isize;
        self.selected_symbol = (selected + direction).rem_euclid(len) as usize;
    }

    pub(super) fn clear_zoom(&mut self) {
        self.zoomed = false;
    }

    pub(super) fn focus_panel_by(&mut self, direction: isize) {
        let visible = self.workspace_panels();
        if visible.is_empty() {
            self.ensure_visible_focus();
            return;
        }
        let current = visible
            .iter()
            .position(|panel| *panel == self.panels.focused())
            .unwrap_or(0) as isize;
        let next = (current + direction).rem_euclid(visible.len() as isize) as usize;
        self.panels.focus(visible[next]);
    }

    pub(super) fn close_floating(&mut self, kind: FloatingKind) {
        self.track_layout_change(|state| state.close_floating_untracked(kind));
    }

    pub(super) fn open_floating(&mut self, kind: FloatingKind) {
        self.track_layout_change(|state| {
            state.close_floating_untracked(kind);
            state.reset_floating_state(kind);
            state.floating.push(FloatingPane::new(kind));
        });
    }

    pub(super) fn close_top_floating(&mut self) {
        self.track_layout_change(|state| {
            if let Some(pane) = state.floating.pop() {
                state.reset_floating_state(pane.kind);
            }
        });
    }

    pub(super) fn close_text_input_floatings_except(&mut self, except: FloatingKind) {
        for kind in FloatingKind::ALL
            .into_iter()
            .filter(|kind| kind.text_input() && *kind != except)
        {
            self.close_floating(kind);
        }
    }

    pub(super) fn close_text_input_floatings(&mut self) {
        for kind in FloatingKind::ALL
            .into_iter()
            .filter(|kind| kind.text_input())
        {
            self.close_floating(kind);
        }
    }

    pub(super) fn reset_open_floating_state(&mut self) {
        let kinds = self
            .floating
            .iter()
            .map(|pane| pane.kind)
            .collect::<Vec<_>>();
        for kind in kinds {
            self.reset_floating_state(kind);
        }
    }

    fn close_floating_untracked(&mut self, kind: FloatingKind) {
        let had_pane = self.floating.iter().any(|pane| pane.kind == kind);
        self.floating.retain(|pane| pane.kind != kind);
        if had_pane {
            self.reset_floating_state(kind);
        }
    }

    pub(super) fn reset_floating_state(&mut self, kind: FloatingKind) {
        match kind {
            FloatingKind::CommandPalette => self.command_palette.reset(),
            FloatingKind::SymbolSearch => self.symbol_search.reset(&self.watchlist),
            FloatingKind::WatchlistAdd => self.watchlist_add.reset(),
            FloatingKind::TradingProfile => {
                self.profile_editor.reset(self.trading_profile.as_deref())
            }
            FloatingKind::TicketTextInput => {
                let Some((target, value)) = self.selected_ticket_text_input() else {
                    return;
                };
                self.ticket_text_input.reset(target, value.as_deref());
            }
            FloatingKind::Help
            | FloatingKind::LiveWritesConfirmation
            | FloatingKind::StagedExecutionConfirmation
            | FloatingKind::ProviderDetails => {}
        }
    }

    pub(super) fn focus_floating(&mut self, kind: FloatingKind) {
        self.track_layout_change(|state| {
            if let Some(index) = state.floating.iter().position(|pane| pane.kind == kind) {
                let pane = state.floating.remove(index);
                state.floating.push(pane);
            }
        });
    }

    pub(super) fn resize_floating(&mut self, kind: FloatingKind, size: FloatingSize) {
        self.track_layout_change(|state| {
            if let Some(pane) = state.floating.iter_mut().find(|pane| pane.kind == kind) {
                pane.size = size;
            }
        });
    }

    pub(super) fn focus_panel(&mut self, panel: Panel) {
        self.track_layout_change(|state| state.focus_panel_untracked(panel));
    }

    pub(super) fn set_workspace(&mut self, workspace: WorkspaceKind) {
        self.workspace = workspace;
        self.clear_zoom();
        self.apply_workspace_entry_policy();
        self.ensure_visible_focus();
    }

    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        self.track_layout_change(|state| {
            if state.is_open_in_workspace(panel) {
                state.panels.toggle(panel);
                state.clear_zoom();
                state.ensure_visible_focus();
            } else {
                state.focus_panel_untracked(panel);
            }
        });
    }

    pub(super) fn ensure_visible_focus(&mut self) {
        let visible_panels = self.workspace_panels();
        if visible_panels.contains(&self.panels.focused()) {
            return;
        }
        self.clear_zoom();

        if let Some(panel) = visible_panels.first().copied() {
            self.panels.focus(panel);
            return;
        }

        self.panels.open_panel(self.workspace.default_panel());
    }

    pub(super) fn apply_workspace_entry_policy(&mut self) {
        let Some(panel) = self.workspace.entry_focus_panel() else {
            return;
        };
        if self.panels.contains(panel) {
            self.panels.focus(panel);
        } else {
            self.panels.open_panel(panel);
        }
    }

    fn workspace_contains(&self, panel: Panel) -> bool {
        self.workspace.panels().contains(&panel)
    }

    fn focus_panel_untracked(&mut self, panel: Panel) {
        if !self.workspace_contains(panel)
            && let Some(workspace) = WorkspaceKind::ALL
                .iter()
                .copied()
                .find(|workspace| workspace.panels().contains(&panel))
        {
            self.workspace = workspace;
        }
        if self.panels.contains(panel) {
            self.panels.focus(panel);
        } else {
            self.panels.open_panel(panel);
        }
        self.clear_zoom();
        self.ensure_visible_focus();
    }
}
