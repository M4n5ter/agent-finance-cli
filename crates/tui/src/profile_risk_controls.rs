use crossterm::event::{KeyCode, KeyEvent};

use crate::command::ActionId;
use crate::model::FloatingKind;
use crate::state::Action;

const PROFILE_RISK_HINTS: &[&str] = &["e profile", "v validate", "t stage risk"];

pub(crate) fn profile_risk_key_action(key: KeyEvent) -> Option<Action> {
    if !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char('e') => Some(Action::Execute(ActionId::OpenFloating(
            FloatingKind::TradingProfile,
        ))),
        KeyCode::Char('v') => Some(Action::Execute(ActionId::RevalidateTradingProfile)),
        KeyCode::Char('t') => Some(Action::Execute(ActionId::StageProfileLiveToggle)),
        _ => None,
    }
}

pub(crate) fn profile_risk_key_hints() -> Vec<String> {
    PROFILE_RISK_HINTS
        .iter()
        .copied()
        .chain(["q quit"])
        .map(str::to_string)
        .collect()
}

pub(crate) fn profile_risk_panel_hint() -> String {
    PROFILE_RISK_HINTS.join("  ")
}
