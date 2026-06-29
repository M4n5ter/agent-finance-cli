use agent_finance_core::{
    FuturesStateChange, FuturesStateChangeKind, MarginType, PositionMode, SubmitMode,
};
use serde::Serialize;

use crate::ticket_text_input::TicketTextInputTarget;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct FuturesStateTicket {
    selected_field: FuturesStateTicketField,
    kind: FuturesStateChangeKind,
    symbol: Option<String>,
    leverage: Option<u8>,
    margin_type: Option<MarginType>,
    position_mode: Option<PositionMode>,
}

impl Default for FuturesStateTicket {
    fn default() -> Self {
        Self {
            selected_field: FuturesStateTicketField::Kind,
            kind: FuturesStateChangeKind::Leverage,
            symbol: None,
            leverage: None,
            margin_type: None,
            position_mode: None,
        }
    }
}

impl FuturesStateTicket {
    #[cfg(test)]
    pub fn set_leverage(&mut self, leverage: Option<u8>) {
        self.leverage = leverage;
    }

    pub fn move_field(&mut self, direction: isize) {
        self.selected_field = self.selected_field.shift(self.kind, direction);
    }

    pub(crate) fn apply_preset(&mut self, preset: FuturesStateTicketPreset) {
        if self.kind == FuturesStateChangeKind::PositionMode {
            self.kind = FuturesStateChangeKind::Leverage;
        }
        self.symbol = Some(preset.symbol);
        self.selected_field = FuturesStateTicketField::Value;
    }

    pub fn select_field(&mut self, index: usize) {
        if let Some(field) = FuturesStateTicketField::ALL.get(index)
            && field.active_for(self.kind)
        {
            self.selected_field = *field;
        }
    }

    pub fn adjust_selected_field(&mut self, direction: isize, symbol_context: Option<&str>) {
        match self.selected_field {
            FuturesStateTicketField::Kind => {
                self.kind = cycle_kind(self.kind, direction);
                if !self.selected_field.active_for(self.kind) {
                    self.selected_field = FuturesStateTicketField::Kind;
                }
            }
            FuturesStateTicketField::Symbol => {
                let symbols = symbol_presets(symbol_context);
                let current = self
                    .symbol
                    .clone()
                    .or_else(|| futures_symbol_context(symbol_context))
                    .unwrap_or_else(|| "BTCUSDT".to_string());
                self.symbol = Some(cycle_text(&symbols, &current, direction));
            }
            FuturesStateTicketField::Value => match self.kind {
                FuturesStateChangeKind::Leverage => {
                    self.leverage = cycle_optional_u8(&LEVERAGE_PRESETS, self.leverage, direction);
                }
                FuturesStateChangeKind::MarginType => {
                    self.margin_type = Some(match self.margin_type.unwrap_or(MarginType::Cross) {
                        MarginType::Cross => MarginType::Isolated,
                        MarginType::Isolated => MarginType::Cross,
                    });
                }
                FuturesStateChangeKind::PositionMode => {
                    self.position_mode =
                        Some(match self.position_mode.unwrap_or(PositionMode::OneWay) {
                            PositionMode::OneWay => PositionMode::Hedge,
                            PositionMode::Hedge => PositionMode::OneWay,
                        });
                }
            },
        }
    }

    pub fn preview(
        &self,
        symbol_context: Option<&str>,
        profile: Option<&str>,
        live_writes_enabled: bool,
        effective_mode: SubmitMode,
    ) -> FuturesStateTicketPreview {
        let mut blockers = Vec::new();
        if profile.is_none() {
            blockers.push("trading profile is required".to_string());
        }
        if effective_mode == SubmitMode::Live && !live_writes_enabled {
            blockers.push("live writes must be enabled".to_string());
        }

        let change = match self.kind {
            FuturesStateChangeKind::Leverage => {
                let symbol = self.explicit_or_context_symbol(symbol_context);
                match (symbol.clone(), self.leverage) {
                    (Some(symbol), Some(leverage)) => {
                        Some(FuturesStateChange::Leverage { symbol, leverage })
                    }
                    (None, _) => {
                        blockers.push("USD-M futures symbol is required".to_string());
                        None
                    }
                    (_, None) => {
                        blockers.push("leverage is required".to_string());
                        None
                    }
                }
            }
            FuturesStateChangeKind::MarginType => {
                let symbol = self.explicit_or_context_symbol(symbol_context);
                match (symbol.clone(), self.margin_type) {
                    (Some(symbol), Some(margin_type)) => Some(FuturesStateChange::MarginType {
                        symbol,
                        margin_type,
                    }),
                    (None, _) => {
                        blockers.push("USD-M futures symbol is required".to_string());
                        None
                    }
                    (_, None) => {
                        blockers.push("margin type is required".to_string());
                        None
                    }
                }
            }
            FuturesStateChangeKind::PositionMode => self
                .position_mode
                .map(|mode| FuturesStateChange::PositionMode { mode })
                .or_else(|| {
                    blockers.push("position mode is required".to_string());
                    None
                }),
        };

        FuturesStateTicketPreview {
            profile: profile.map(ToString::to_string),
            kind: self.kind,
            symbol: self.preview_symbol(symbol_context),
            leverage: self.leverage,
            margin_type: self.margin_type,
            position_mode: self.position_mode,
            change,
            live_writes_enabled,
            effective_mode,
            ready: blockers.is_empty(),
            blockers,
        }
    }

    pub fn selected_field_label(&self) -> &'static str {
        self.selected_field.label()
    }

    pub(crate) fn selected_text_input(&self) -> Option<(TicketTextInputTarget, Option<String>)> {
        match (self.selected_field, self.kind) {
            (FuturesStateTicketField::Value, FuturesStateChangeKind::Leverage) => Some((
                TicketTextInputTarget::FuturesLeverage,
                self.leverage.map(|leverage| leverage.to_string()),
            )),
            _ => None,
        }
    }

    pub(crate) fn set_leverage_text(&mut self, value: Option<&str>) -> Result<(), String> {
        self.leverage = value
            .map(|value| {
                value
                    .parse::<u8>()
                    .map_err(|error| format!("leverage: {error}"))
            })
            .transpose()?;
        self.selected_field = FuturesStateTicketField::Value;
        Ok(())
    }

    pub(crate) fn apply_text_input(
        &mut self,
        target: TicketTextInputTarget,
        value: Option<String>,
    ) -> Result<(), String> {
        match target {
            TicketTextInputTarget::FuturesLeverage => self.set_leverage_text(value.as_deref()),
            _ => Err(format!(
                "{} does not target futures state ticket",
                target.field_label()
            )),
        }
    }

    fn explicit_or_context_symbol(&self, symbol_context: Option<&str>) -> Option<String> {
        self.symbol
            .clone()
            .or_else(|| futures_symbol_context(symbol_context))
    }

    fn preview_symbol(&self, symbol_context: Option<&str>) -> Option<String> {
        match self.kind {
            FuturesStateChangeKind::Leverage | FuturesStateChangeKind::MarginType => {
                self.explicit_or_context_symbol(symbol_context)
            }
            FuturesStateChangeKind::PositionMode => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FuturesStateTicketPreset {
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FuturesStateTicketField {
    Kind,
    Symbol,
    Value,
}

impl FuturesStateTicketField {
    pub const MAX_COUNT: usize = Self::ALL.len();

    const ALL: [Self; 3] = [Self::Kind, Self::Symbol, Self::Value];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Symbol => "scope",
            Self::Value => "value",
        }
    }

    fn shift(self, kind: FuturesStateChangeKind, direction: isize) -> Self {
        let fields = active_fields(kind);
        let index = fields.iter().position(|field| *field == self).unwrap_or(0) as isize;
        let next = (index + direction).rem_euclid(fields.len() as isize) as usize;
        fields[next]
    }

    fn active_for(self, kind: FuturesStateChangeKind) -> bool {
        active_fields(kind).contains(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuturesStateTicketPreview {
    pub profile: Option<String>,
    pub kind: FuturesStateChangeKind,
    pub symbol: Option<String>,
    pub leverage: Option<u8>,
    pub margin_type: Option<MarginType>,
    pub position_mode: Option<PositionMode>,
    pub change: Option<FuturesStateChange>,
    pub live_writes_enabled: bool,
    pub effective_mode: SubmitMode,
    pub ready: bool,
    pub blockers: Vec<String>,
}

impl FuturesStateTicketPreview {
    pub fn scope_label(&self) -> String {
        match self.kind {
            FuturesStateChangeKind::Leverage | FuturesStateChangeKind::MarginType => {
                self.symbol.clone().unwrap_or_else(|| "-".to_string())
            }
            FuturesStateChangeKind::PositionMode => "account-wide".to_string(),
        }
    }
}

const LEVERAGE_PRESETS: [u8; 6] = [1, 2, 3, 5, 10, 20];

fn cycle_kind(current: FuturesStateChangeKind, direction: isize) -> FuturesStateChangeKind {
    const KINDS: [FuturesStateChangeKind; 3] = [
        FuturesStateChangeKind::Leverage,
        FuturesStateChangeKind::MarginType,
        FuturesStateChangeKind::PositionMode,
    ];
    let index = KINDS
        .iter()
        .position(|candidate| *candidate == current)
        .unwrap_or(0) as isize;
    let next = (index + direction).rem_euclid(KINDS.len() as isize) as usize;
    KINDS[next]
}

fn cycle_optional_u8(values: &[u8], current: Option<u8>, direction: isize) -> Option<u8> {
    let index = current
        .and_then(|value| values.iter().position(|candidate| *candidate == value))
        .map(|index| index as isize)
        .unwrap_or(if direction >= 0 { -1 } else { 0 });
    let next = index + direction;
    if next < 0 || next >= values.len() as isize {
        return None;
    }
    Some(values[next as usize])
}

fn cycle_text(values: &[String], current: &str, direction: isize) -> String {
    let index = values
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(current))
        .map(|index| index as isize)
        .unwrap_or(0);
    let next = (index + direction).rem_euclid(values.len() as isize) as usize;
    values[next].clone()
}

fn symbol_presets(symbol_context: Option<&str>) -> Vec<String> {
    let mut symbols =
        vec![futures_symbol_context(symbol_context).unwrap_or_else(|| "BTCUSDT".to_string())];
    for symbol in ["BTCUSDT", "ETHUSDT", "SOLUSDT"] {
        if !symbols.iter().any(|candidate| candidate == symbol) {
            symbols.push(symbol.to_string());
        }
    }
    symbols
}

fn futures_symbol_context(symbol_context: Option<&str>) -> Option<String> {
    symbol_context
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| symbol.ends_with("USDT") || symbol.ends_with("USDC"))
}

fn active_fields(kind: FuturesStateChangeKind) -> &'static [FuturesStateTicketField] {
    match kind {
        FuturesStateChangeKind::Leverage | FuturesStateChangeKind::MarginType => {
            &FuturesStateTicketField::ALL
        }
        FuturesStateChangeKind::PositionMode => &[
            FuturesStateTicketField::Kind,
            FuturesStateTicketField::Value,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn futures_state_ticket_requires_explicit_value() {
        let ticket = FuturesStateTicket::default();

        let preview = ticket.preview(Some("ETHUSDT"), Some("mainnet"), false, SubmitMode::DryRun);

        assert!(!preview.ready);
        assert_eq!(preview.symbol.as_deref(), Some("ETHUSDT"));
        assert_eq!(preview.blockers, vec!["leverage is required"]);
    }

    #[test]
    fn futures_state_ticket_blocks_non_futures_symbol_without_silent_fallback() {
        let mut ticket = FuturesStateTicket::default();
        ticket.set_leverage(Some(2));

        let preview = ticket.preview(Some("AAPL"), Some("mainnet"), false, SubmitMode::DryRun);

        assert!(!preview.ready);
        assert_eq!(preview.symbol, None);
        assert_eq!(
            preview.blockers,
            vec!["USD-M futures symbol is required".to_string()]
        );
    }

    #[test]
    fn futures_state_ticket_builds_symbol_scoped_leverage_change() {
        let mut ticket = FuturesStateTicket::default();

        ticket.move_field(2);
        ticket.adjust_selected_field(1, Some("ethusdt"));

        let preview = ticket.preview(Some("ethusdt"), Some("mainnet"), false, SubmitMode::DryRun);

        assert!(preview.ready);
        assert_eq!(
            preview.change,
            Some(FuturesStateChange::Leverage {
                symbol: "ETHUSDT".to_string(),
                leverage: 1,
            })
        );
    }

    #[test]
    fn futures_state_ticket_preset_sets_symbol_scoped_value_focus() {
        let mut ticket = FuturesStateTicket::default();
        ticket.adjust_selected_field(2, None);

        ticket.apply_preset(FuturesStateTicketPreset {
            symbol: "ETHUSDT".to_string(),
        });

        let preview = ticket.preview(None, Some("mainnet"), false, SubmitMode::DryRun);
        assert_eq!(preview.kind, FuturesStateChangeKind::Leverage);
        assert_eq!(preview.symbol.as_deref(), Some("ETHUSDT"));
        assert_eq!(ticket.selected_field_label(), "value");
        assert!(!preview.ready);
        assert_eq!(preview.blockers, vec!["leverage is required"]);
    }

    #[test]
    fn futures_state_ticket_builds_account_scoped_position_mode_change() {
        let mut ticket = FuturesStateTicket::default();

        ticket.adjust_selected_field(-1, Some("BTCUSDT"));
        ticket.move_field(1);
        ticket.adjust_selected_field(1, Some("BTCUSDT"));

        let preview = ticket.preview(Some("BTCUSDT"), Some("mainnet"), false, SubmitMode::DryRun);

        assert!(preview.ready);
        assert_eq!(preview.symbol, None);
        assert_eq!(preview.scope_label(), "account-wide");
        assert_eq!(
            preview.change,
            Some(FuturesStateChange::PositionMode {
                mode: PositionMode::Hedge,
            })
        );
    }

    #[test]
    fn futures_state_ticket_skips_symbol_field_for_position_mode() {
        let mut ticket = FuturesStateTicket::default();

        ticket.adjust_selected_field(-1, Some("BTCUSDT"));
        ticket.move_field(1);

        assert_eq!(ticket.selected_field_label(), "value");
    }
}
