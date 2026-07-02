use crate::config::{TuiConfig, WorkspaceConfig};
use crate::model::DockedPanels;

use super::AppState;

const CONFIG_UNDO_LIMIT: usize = 32;

#[derive(Debug, Clone, Default)]
pub(super) struct LocalConfigHistory {
    stack: Vec<LocalConfigSnapshot>,
}

impl LocalConfigHistory {
    pub(super) fn available(&self) -> bool {
        !self.stack.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.stack.clear();
    }

    pub(super) fn pop(&mut self) -> Option<LocalConfigSnapshot> {
        self.stack.pop()
    }

    pub(super) fn push(&mut self, snapshot: LocalConfigSnapshot) {
        if self.stack.last().is_some_and(|last| last == &snapshot) {
            return;
        }
        self.stack.push(snapshot);
        if self.stack.len() > CONFIG_UNDO_LIMIT {
            self.stack.remove(0);
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct LocalConfigSnapshot {
    pub(super) config: TuiConfig,
    pub(super) config_changes: Vec<String>,
    pub(super) trading_profile_edited: bool,
    pub(super) pending_provider_preferences_update: bool,
    pub(super) pending_config_save: bool,
}

pub(crate) struct LocalConfigEdit<R> {
    pub(super) section: &'static str,
    pub(super) result: R,
}

impl<R> LocalConfigEdit<R> {
    pub(super) fn new(section: &'static str, result: R) -> Self {
        Self { section, result }
    }
}

impl AppState {
    pub(super) fn edit_local_config<R>(
        &mut self,
        mutate: impl FnOnce(&mut Self) -> Option<LocalConfigEdit<R>>,
    ) -> Option<R> {
        let snapshot = self.local_config_snapshot();
        let Some(edit) = mutate(self) else {
            let current = self.local_config_snapshot();
            debug_assert!(
                snapshot.matches_noop_edit(&current),
                "local config edit returned None after mutating config state"
            );
            return None;
        };
        self.commit_config_edit(snapshot, edit.section);
        Some(edit.result)
    }

    pub(super) fn local_config_snapshot(&self) -> LocalConfigSnapshot {
        LocalConfigSnapshot {
            config: self.export_config(&TuiConfig::default()),
            config_changes: self.config_changes.clone(),
            trading_profile_edited: self.trading_profile_edited,
            pending_provider_preferences_update: self.pending_provider_preferences_update,
            pending_config_save: self.pending_config_save,
        }
    }

    fn push_config_undo(&mut self, snapshot: LocalConfigSnapshot) {
        if self.local_config_snapshot() != snapshot {
            self.config_undo_history.push(snapshot);
        }
    }

    fn commit_config_edit(&mut self, snapshot: LocalConfigSnapshot, section: &str) {
        if self.local_config_snapshot() == snapshot {
            return;
        }
        self.push_config_undo(snapshot);
        self.mark_config_changed(section);
    }

    fn mark_config_changed(&mut self, section: &str) {
        if !self.config_changes.iter().any(|change| change == section) {
            self.config_changes.push(section.to_string());
        }
    }

    pub(super) fn restore_local_config_snapshot(&mut self, snapshot: LocalConfigSnapshot) {
        let TuiConfig {
            watchlist,
            locale,
            workspace: WorkspaceConfig { current: _ },
            layout,
            panels,
            floating,
            refresh: _,
            chart,
            providers,
            trading,
            theme,
            keymap,
        } = snapshot.config;
        let selected_symbol = self.selected_symbol().map(ToString::to_string);
        let workspace = self.workspace;
        let focused_panel = self.panels.focused();
        let transient_floatings = self
            .floating
            .iter()
            .copied()
            .filter(|pane| !pane.kind.persistent())
            .collect::<Vec<_>>();

        self.watchlist = watchlist;
        self.locale = locale
            .current
            .unwrap_or(agent_finance_i18n::LocaleId::DEFAULT);
        self.selected_symbol = selected_symbol
            .as_deref()
            .and_then(|symbol| {
                self.watchlist
                    .iter()
                    .position(|candidate| candidate == symbol)
            })
            .unwrap_or_else(|| {
                self.selected_symbol
                    .min(self.watchlist.len().saturating_sub(1))
            });
        self.config_changes = snapshot.config_changes;
        self.workspace = workspace;
        self.layout = layout;
        self.panels = DockedPanels::from_open_focused(panels.open, focused_panel);
        self.floating = transient_floatings;
        self.floating.extend(floating.panes);
        self.keymap = keymap;
        self.providers = providers;
        self.chart.set_preset(chart.preset);
        self.theme = theme;
        self.trading_profile = trading.default_profile;
        self.trading_profile_edited = snapshot.trading_profile_edited;
        self.pending_provider_preferences_update = snapshot.pending_provider_preferences_update;
        self.pending_config_save = snapshot.pending_config_save;
        self.ensure_visible_focus();
    }
}

impl LocalConfigSnapshot {
    fn matches_noop_edit(&self, other: &Self) -> bool {
        let mut left = self.clone();
        let mut right = other.clone();
        right.config.workspace = left.config.workspace.clone();
        right.config.panels.focused = left.config.panels.focused;
        left.config
            .floating
            .panes
            .sort_by_key(|pane| pane.kind.title());
        right
            .config
            .floating
            .panes
            .sort_by_key(|pane| pane.kind.title());
        left == right
    }
}
