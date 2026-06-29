use std::str::FromStr;

use agent_finance_core::{
    DecimalValue, Market, OrderKind, OrderSide, OrderSpec, SubmitMode, TimeInForce,
};
use serde::Serialize;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct OrderTicket {
    selected_field: OrderTicketField,
    market: Market,
    side: OrderSide,
    kind: OrderKind,
    quantity: Option<String>,
    price: Option<String>,
    time_in_force: TimeInForce,
    reduce_only: bool,
}

impl Default for OrderTicket {
    fn default() -> Self {
        Self {
            selected_field: OrderTicketField::Market,
            market: Market::Spot,
            side: OrderSide::Buy,
            kind: OrderKind::PostOnlyLimit,
            quantity: None,
            price: None,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
        }
    }
}

impl OrderTicket {
    pub fn market(&self) -> Market {
        self.market
    }

    pub fn side(&self) -> OrderSide {
        self.side
    }

    pub fn kind(&self) -> OrderKind {
        self.kind
    }

    pub fn time_in_force(&self) -> TimeInForce {
        self.time_in_force
    }

    pub fn reduce_only(&self) -> bool {
        self.reduce_only
    }

    pub(crate) fn set_quantity_text(&mut self, quantity: Option<String>) {
        self.quantity = quantity;
    }

    pub(crate) fn set_price_text(&mut self, price: Option<String>) {
        self.price = price;
    }

    #[cfg(test)]
    pub fn set_reduce_only(&mut self, reduce_only: bool) {
        self.reduce_only = reduce_only;
    }

    pub fn move_field(&mut self, direction: isize) {
        self.selected_field = self.selected_field.shift(direction);
    }

    pub fn select_field(&mut self, index: usize) {
        if let Some(field) = OrderTicketField::ALL.get(index) {
            self.selected_field = *field;
        }
    }

    pub fn capture_reference_price(&mut self, price: f64) {
        self.price = Some(format_reference_price(price));
        self.selected_field = OrderTicketField::Price;
    }

    pub fn adjust_selected_field(&mut self, direction: isize, reference_price: Option<f64>) {
        match self.selected_field {
            OrderTicketField::Market => {
                self.market = match self.market {
                    Market::Spot => Market::UsdsFutures,
                    Market::UsdsFutures => Market::Spot,
                };
                self.normalize_for_market();
            }
            OrderTicketField::Side => {
                self.side = match self.side {
                    OrderSide::Buy => OrderSide::Sell,
                    OrderSide::Sell => OrderSide::Buy,
                };
            }
            OrderTicketField::Kind => {
                self.kind = order_kind_shift(self.kind, direction, self.market);
                self.normalize_for_kind();
            }
            OrderTicketField::Quantity => {
                self.quantity =
                    cycle_optional_text(&QUANTITY_PRESETS, self.quantity.as_deref(), direction);
            }
            OrderTicketField::Price => {
                self.price = if let Some(price) = reference_price {
                    Some(format_reference_price(price))
                } else {
                    cycle_optional_text(&PRICE_PRESETS, self.price.as_deref(), direction)
                };
            }
            OrderTicketField::TimeInForce => {
                self.time_in_force = time_in_force_shift(self.time_in_force, direction);
            }
            OrderTicketField::ReduceOnly => {
                self.reduce_only = !self.reduce_only;
            }
        }
    }

    pub fn preview(
        &self,
        symbol: Option<&str>,
        profile: Option<&str>,
        live_writes_enabled: bool,
        effective_mode: SubmitMode,
        reference_price: Option<f64>,
    ) -> OrderTicketPreview {
        let mut blockers = Vec::new();
        if profile.is_none() {
            blockers.push("trading profile is required".to_string());
        }
        if symbol.is_none() {
            blockers.push("selected symbol is required".to_string());
        }
        let quantity = match parse_optional_decimal("quantity", self.quantity.as_deref()) {
            Ok(Some(quantity)) => Some(quantity),
            Ok(None) => {
                blockers.push("quantity is required".to_string());
                None
            }
            Err(error) => {
                blockers.push(error);
                None
            }
        };
        let order_spec = match self.order_spec(reference_price) {
            Ok(spec) => Some(spec),
            Err(error) => {
                blockers.push(error);
                None
            }
        };
        if effective_mode == SubmitMode::Live && !live_writes_enabled {
            blockers.push("live writes must be enabled".to_string());
        }

        OrderTicketPreview {
            symbol: symbol.map(ToString::to_string),
            profile: profile.map(ToString::to_string),
            market: self.market,
            side: self.side,
            kind: self.kind,
            quantity: self.quantity.clone(),
            price: self.effective_price(reference_price),
            time_in_force: self.time_in_force,
            reduce_only: self.reduce_only,
            parsed_quantity: quantity.clone(),
            order_spec,
            live_writes_enabled,
            effective_mode,
            ready: blockers.is_empty() && quantity.is_some(),
            blockers,
        }
    }

    pub fn selected_field_label(&self) -> &'static str {
        self.selected_field.label()
    }

    pub(crate) fn selected_field(&self) -> OrderTicketField {
        self.selected_field
    }

    pub(crate) fn selected_text_input(&self) -> Option<(OrderTicketField, Option<&str>)> {
        match self.selected_field {
            OrderTicketField::Quantity => Some((self.selected_field, self.quantity_text())),
            OrderTicketField::Price => Some((self.selected_field, self.price_text())),
            _ => None,
        }
    }

    pub(crate) fn quantity_text(&self) -> Option<&str> {
        self.quantity.as_deref()
    }

    pub(crate) fn price_text(&self) -> Option<&str> {
        self.price.as_deref()
    }

    fn effective_price(&self, reference_price: Option<f64>) -> Option<String> {
        self.price
            .clone()
            .or_else(|| reference_price.map(format_reference_price))
    }

    fn order_spec(&self, reference_price: Option<f64>) -> Result<OrderSpec, String> {
        let effective_price = self.effective_price(reference_price);
        let parsed_price = parse_optional_decimal("price", effective_price.as_deref())?;
        let (price, valuation_price, stop_price, time_in_force) = match self.kind {
            OrderKind::Market => (None, parsed_price, None, None),
            OrderKind::Limit => (parsed_price, None, None, Some(self.time_in_force)),
            OrderKind::PostOnlyLimit => (parsed_price, None, None, None),
            OrderKind::StopLoss | OrderKind::TakeProfit => (None, None, parsed_price, None),
        };
        OrderSpec::new(
            self.market,
            self.kind,
            price,
            valuation_price,
            stop_price,
            time_in_force,
        )
        .map_err(|error| error.to_string())
    }

    fn normalize_for_market(&mut self) {
        if self.market == Market::UsdsFutures && self.kind == OrderKind::PostOnlyLimit {
            self.kind = OrderKind::Limit;
        }
    }

    fn normalize_for_kind(&mut self) {
        if matches!(self.kind, OrderKind::Market) {
            self.price = None;
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrderTicketField {
    Market,
    Side,
    Kind,
    Quantity,
    Price,
    TimeInForce,
    ReduceOnly,
}

impl OrderTicketField {
    pub const COUNT: usize = Self::ALL.len();

    const ALL: [Self; 7] = [
        Self::Market,
        Self::Side,
        Self::Kind,
        Self::Quantity,
        Self::Price,
        Self::TimeInForce,
        Self::ReduceOnly,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Side => "side",
            Self::Kind => "kind",
            Self::Quantity => "quantity",
            Self::Price => "price",
            Self::TimeInForce => "time in force",
            Self::ReduceOnly => "reduce only",
        }
    }

    pub(crate) fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|field| *field == self)
            .expect("order ticket field is part of ALL")
    }

    fn shift(self, direction: isize) -> Self {
        let index = self.index() as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct OrderTicketPreview {
    pub symbol: Option<String>,
    pub profile: Option<String>,
    pub market: Market,
    pub side: OrderSide,
    pub kind: OrderKind,
    pub quantity: Option<String>,
    pub price: Option<String>,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    pub parsed_quantity: Option<DecimalValue>,
    pub order_spec: Option<OrderSpec>,
    pub live_writes_enabled: bool,
    pub effective_mode: SubmitMode,
    pub ready: bool,
    pub blockers: Vec<String>,
}

const QUANTITY_PRESETS: [&str; 5] = ["0.001", "0.01", "0.05", "0.1", "1"];
const PRICE_PRESETS: [&str; 4] = ["100", "250", "500", "1000"];

fn order_kind_shift(current: OrderKind, direction: isize, market: Market) -> OrderKind {
    let kinds = match market {
        Market::Spot => &[
            OrderKind::Market,
            OrderKind::Limit,
            OrderKind::PostOnlyLimit,
            OrderKind::StopLoss,
            OrderKind::TakeProfit,
        ][..],
        Market::UsdsFutures => &[OrderKind::Market, OrderKind::Limit][..],
    };
    let index = kinds.iter().position(|kind| *kind == current).unwrap_or(0) as isize;
    let next = (index + direction).rem_euclid(kinds.len() as isize) as usize;
    kinds[next]
}

fn time_in_force_shift(current: TimeInForce, direction: isize) -> TimeInForce {
    let values = [TimeInForce::Gtc, TimeInForce::Ioc, TimeInForce::Fok];
    let index = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0) as isize;
    let next = (index + direction).rem_euclid(values.len() as isize) as usize;
    values[next]
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

fn format_reference_price(price: f64) -> String {
    if price >= 100.0 {
        format!("{price:.2}")
    } else if price >= 1.0 {
        format!("{price:.4}")
    } else {
        format!("{price:.8}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn futures_market_removes_spot_only_order_kind() {
        let mut ticket = OrderTicket {
            kind: OrderKind::PostOnlyLimit,
            ..OrderTicket::default()
        };

        ticket.adjust_selected_field(1, None);

        assert_eq!(ticket.market(), Market::UsdsFutures);
        assert_eq!(ticket.kind(), OrderKind::Limit);
    }

    #[test]
    fn preview_requires_profile_symbol_quantity_and_price_for_limit_like_orders() {
        let ticket = OrderTicket::default();

        let preview = ticket.preview(None, None, false, SubmitMode::DryRun, None);

        assert!(!preview.ready);
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.contains("profile"))
        );
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.contains("symbol"))
        );
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.contains("quantity"))
        );
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.contains("price"))
        );
    }

    #[test]
    fn reference_price_can_satisfy_price_preview_without_mutating_ticket() {
        let mut ticket = OrderTicket::default();
        ticket.set_quantity_text(Some("0.1".to_string()));

        let preview = ticket.preview(
            Some("BTCUSDT"),
            Some("mainnet"),
            false,
            SubmitMode::DryRun,
            Some(123.456),
        );

        assert!(preview.ready);
        assert_eq!(preview.price.as_deref(), Some("123.46"));
        assert_eq!(ticket.price, None);
    }

    #[test]
    fn capture_reference_price_fixes_price_and_focuses_price_field() {
        let mut ticket = OrderTicket::default();

        ticket.capture_reference_price(123.456);

        let preview = ticket.preview(
            Some("BTCUSDT"),
            Some("mainnet"),
            false,
            SubmitMode::DryRun,
            Some(999.0),
        );
        assert_eq!(preview.price.as_deref(), Some("123.46"));
        assert_eq!(ticket.selected_field_label(), "price");
    }

    #[test]
    fn preview_uses_core_order_spec_contract_for_ready_state() {
        let mut ticket = OrderTicket::default();
        ticket.set_quantity_text(Some("0.1".to_string()));
        ticket.set_price_text(Some("123".to_string()));

        let preview = ticket.preview(
            Some("BTCUSDT"),
            Some("mainnet"),
            false,
            SubmitMode::DryRun,
            None,
        );

        assert!(preview.ready);

        ticket.selected_field = OrderTicketField::Kind;
        ticket.adjust_selected_field(1, None);
        let stop_loss = ticket.preview(
            Some("BTCUSDT"),
            Some("mainnet"),
            false,
            SubmitMode::DryRun,
            None,
        );

        assert!(stop_loss.ready);
        assert_eq!(stop_loss.kind, OrderKind::StopLoss);

        ticket.set_price_text(None);
        let missing_stop_price = ticket.preview(
            Some("BTCUSDT"),
            Some("mainnet"),
            false,
            SubmitMode::DryRun,
            None,
        );

        assert!(!missing_stop_price.ready);
        assert!(
            missing_stop_price
                .blockers
                .iter()
                .any(|blocker| blocker.contains("stop-loss order requires stop price"))
        );
    }
}
