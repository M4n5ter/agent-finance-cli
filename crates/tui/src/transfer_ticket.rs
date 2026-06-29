use std::str::FromStr;

use agent_finance_core::{DecimalValue, SubmitMode, TransferDirection};
use serde::Serialize;

use crate::ticket_text_input::TicketTextInputTarget;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TransferTicket {
    selected_field: TransferTicketField,
    direction: TransferDirection,
    asset: String,
    amount: Option<String>,
}

impl Default for TransferTicket {
    fn default() -> Self {
        Self {
            selected_field: TransferTicketField::Direction,
            direction: TransferDirection::SpotToUsdsFutures,
            asset: "USDT".to_string(),
            amount: None,
        }
    }
}

impl TransferTicket {
    pub(crate) fn set_amount_text(&mut self, amount: Option<String>) {
        self.amount = amount;
    }

    pub(crate) fn apply_preset(&mut self, preset: TransferTicketPreset) {
        self.direction = preset.direction;
        self.asset = preset.asset;
        self.amount = Some(preset.amount);
        self.selected_field = TransferTicketField::Amount;
    }

    pub fn move_field(&mut self, direction: isize) {
        self.selected_field = self.selected_field.shift(direction);
    }

    pub fn select_field(&mut self, index: usize) {
        if let Some(field) = TransferTicketField::ALL.get(index) {
            self.selected_field = *field;
        }
    }

    pub fn adjust_selected_field(&mut self, direction: isize) {
        match self.selected_field {
            TransferTicketField::Direction => {
                self.direction = match self.direction {
                    TransferDirection::SpotToUsdsFutures => TransferDirection::UsdsFuturesToSpot,
                    TransferDirection::UsdsFuturesToSpot => TransferDirection::SpotToUsdsFutures,
                };
            }
            TransferTicketField::Asset => {
                self.asset = cycle_text(&ASSET_PRESETS, &self.asset, direction);
            }
            TransferTicketField::Amount => {
                self.amount =
                    cycle_optional_text(&AMOUNT_PRESETS, self.amount.as_deref(), direction);
            }
        }
    }

    pub fn preview(
        &self,
        profile: Option<&str>,
        live_writes_enabled: bool,
        effective_mode: SubmitMode,
    ) -> TransferTicketPreview {
        let mut blockers = Vec::new();
        if profile.is_none() {
            blockers.push("trading profile is required".to_string());
        }
        let amount = match parse_optional_decimal("amount", self.amount.as_deref()) {
            Ok(Some(amount)) => Some(amount),
            Ok(None) => {
                blockers.push("amount is required".to_string());
                None
            }
            Err(error) => {
                blockers.push(error);
                None
            }
        };
        if effective_mode == SubmitMode::Live && !live_writes_enabled {
            blockers.push("live writes must be enabled".to_string());
        }

        TransferTicketPreview {
            profile: profile.map(ToString::to_string),
            direction: self.direction,
            asset: self.asset.clone(),
            amount: self.amount.clone(),
            parsed_amount: amount.clone(),
            live_writes_enabled,
            effective_mode,
            ready: blockers.is_empty() && amount.is_some(),
            blockers,
        }
    }

    pub fn selected_field_label(&self) -> &'static str {
        self.selected_field.label()
    }

    pub(crate) fn selected_text_input(&self) -> Option<(TicketTextInputTarget, Option<&str>)> {
        match self.selected_field {
            TransferTicketField::Amount => Some((
                TicketTextInputTarget::TransferAmount,
                self.amount.as_deref(),
            )),
            _ => None,
        }
    }

    pub(crate) fn apply_text_input(
        &mut self,
        target: TicketTextInputTarget,
        value: Option<String>,
    ) -> Result<(), String> {
        match target {
            TicketTextInputTarget::TransferAmount => {
                self.set_amount_text(value);
                self.selected_field = TransferTicketField::Amount;
                Ok(())
            }
            _ => Err(format!(
                "{} does not target transfer ticket",
                target.field_label()
            )),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct TransferTicketPreset {
    pub direction: TransferDirection,
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransferTicketField {
    Direction,
    Asset,
    Amount,
}

impl TransferTicketField {
    pub const COUNT: usize = Self::ALL.len();

    const ALL: [Self; 3] = [Self::Direction, Self::Asset, Self::Amount];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Direction => "direction",
            Self::Asset => "asset",
            Self::Amount => "amount",
        }
    }

    fn shift(self, direction: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0) as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TransferTicketPreview {
    pub profile: Option<String>,
    pub direction: TransferDirection,
    pub asset: String,
    pub amount: Option<String>,
    pub parsed_amount: Option<DecimalValue>,
    pub live_writes_enabled: bool,
    pub effective_mode: SubmitMode,
    pub ready: bool,
    pub blockers: Vec<String>,
}

const ASSET_PRESETS: [&str; 2] = ["USDT", "USDC"];
const AMOUNT_PRESETS: [&str; 6] = ["1", "5", "10", "25", "50", "100"];

fn cycle_text(values: &[&str], current: &str, direction: isize) -> String {
    let index = values
        .iter()
        .position(|candidate| *candidate == current)
        .map(|index| index as isize)
        .unwrap_or(0);
    let next = (index + direction).rem_euclid(values.len() as isize) as usize;
    values[next].to_string()
}

fn cycle_optional_text(values: &[&str], current: Option<&str>, direction: isize) -> Option<String> {
    let index = current
        .and_then(|value| values.iter().position(|candidate| *candidate == value))
        .map(|index| index as isize)
        .unwrap_or(if direction >= 0 { -1 } else { 0 });
    let next = index + direction;
    if next < 0 || next >= values.len() as isize {
        return None;
    }
    Some(values[next as usize].to_string())
}

fn parse_optional_decimal(
    label: &str,
    value: Option<&str>,
) -> Result<Option<DecimalValue>, String> {
    value
        .map(|value| DecimalValue::from_str(value).map_err(|error| format!("{label}: {error}")))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_ticket_preview_requires_profile_and_amount() {
        let ticket = TransferTicket::default();

        let preview = ticket.preview(None, false, SubmitMode::DryRun);

        assert!(!preview.ready);
        assert_eq!(
            preview.blockers,
            vec![
                "trading profile is required".to_string(),
                "amount is required".to_string()
            ]
        );
    }

    #[test]
    fn transfer_ticket_adjusts_direction_asset_and_amount() {
        let mut ticket = TransferTicket::default();

        ticket.adjust_selected_field(1);
        assert_eq!(
            ticket
                .preview(Some("mainnet"), false, SubmitMode::DryRun)
                .direction,
            TransferDirection::UsdsFuturesToSpot
        );
        ticket.move_field(1);
        ticket.adjust_selected_field(1);
        assert_eq!(
            ticket
                .preview(Some("mainnet"), false, SubmitMode::DryRun)
                .asset,
            "USDC"
        );
        ticket.move_field(1);
        ticket.adjust_selected_field(1);

        let preview = ticket.preview(Some("mainnet"), false, SubmitMode::DryRun);
        assert!(preview.ready);
        assert_eq!(preview.amount.as_deref(), Some("1"));
    }

    #[test]
    fn transfer_ticket_preset_sets_transfer_fields_and_focuses_amount() {
        let mut ticket = TransferTicket::default();

        ticket.apply_preset(TransferTicketPreset {
            direction: TransferDirection::UsdsFuturesToSpot,
            asset: "USDC".to_string(),
            amount: "4.5".to_string(),
        });

        let preview = ticket.preview(Some("mainnet"), false, SubmitMode::DryRun);
        assert!(preview.ready);
        assert_eq!(preview.direction, TransferDirection::UsdsFuturesToSpot);
        assert_eq!(preview.asset, "USDC");
        assert_eq!(preview.amount.as_deref(), Some("4.5"));
        assert_eq!(ticket.selected_field_label(), "amount");
    }
}
