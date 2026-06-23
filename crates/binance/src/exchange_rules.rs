use agent_finance_core::{DecimalValue, Market, OrderIntent, OrderSpec};
use anyhow::{Context, Result, anyhow};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRuleCheck {
    pub allowed: bool,
    pub market: Market,
    pub symbol: String,
    pub findings: Vec<ExchangeRuleFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRuleFinding {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct SymbolExchangeRules {
    market: Market,
    symbol: String,
    status: Option<String>,
    price: Option<PriceFilter>,
    lot_size: Option<QuantityFilter>,
    market_lot_size: Option<QuantityFilter>,
    min_notional: Option<MinNotionalFilter>,
    notional: Option<NotionalFilter>,
}

#[derive(Debug, Clone, Copy)]
struct PriceFilter {
    min: Option<Decimal>,
    max: Option<Decimal>,
    tick_size: Option<Decimal>,
}

#[derive(Debug, Clone, Copy)]
struct QuantityFilter {
    min: Option<Decimal>,
    max: Option<Decimal>,
    step_size: Option<Decimal>,
}

#[derive(Debug, Clone, Copy)]
struct MinNotionalFilter {
    min: Decimal,
    apply_to_market: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
struct NotionalFilter {
    min: Option<Decimal>,
    max: Option<Decimal>,
    apply_min_to_market: Option<bool>,
    apply_max_to_market: Option<bool>,
}

impl ExchangeRuleCheck {
    fn allow(market: Market, symbol: &str) -> Self {
        Self {
            allowed: true,
            market,
            symbol: symbol.to_ascii_uppercase(),
            findings: Vec::new(),
        }
    }

    fn block(&mut self, code: &str, message: impl Into<String>) {
        self.allowed = false;
        self.findings.push(ExchangeRuleFinding {
            code: code.to_string(),
            message: message.into(),
        });
    }

    fn warn(&mut self, code: &str, message: impl Into<String>) {
        self.findings.push(ExchangeRuleFinding {
            code: code.to_string(),
            message: message.into(),
        });
    }
}

pub fn check_order_exchange_rules(
    intent: &OrderIntent,
    exchange_info: &Value,
) -> Result<ExchangeRuleCheck> {
    let rules =
        SymbolExchangeRules::from_exchange_info(intent.market, &intent.symbol, exchange_info)?;
    let mut check = ExchangeRuleCheck::allow(intent.market, &intent.symbol);
    rules.check_status(&mut check);
    rules.check_price(intent, &mut check);
    rules.check_quantity(intent, &mut check);
    rules.check_notional(intent, &mut check);
    Ok(check)
}

impl SymbolExchangeRules {
    fn from_exchange_info(market: Market, symbol: &str, exchange_info: &Value) -> Result<Self> {
        let symbol_row = find_symbol(exchange_info, symbol)?;
        Ok(Self {
            market,
            symbol: symbol.to_ascii_uppercase(),
            status: symbol_row
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string),
            price: filter(symbol_row, "PRICE_FILTER")
                .map(PriceFilter::from_filter)
                .transpose()?,
            lot_size: filter(symbol_row, "LOT_SIZE")
                .map(QuantityFilter::from_filter)
                .transpose()?,
            market_lot_size: filter(symbol_row, "MARKET_LOT_SIZE")
                .map(QuantityFilter::from_filter)
                .transpose()?,
            min_notional: filter(symbol_row, "MIN_NOTIONAL")
                .map(|filter| MinNotionalFilter::from_filter(market, filter))
                .transpose()?,
            notional: filter(symbol_row, "NOTIONAL")
                .map(NotionalFilter::from_filter)
                .transpose()?,
        })
    }

    fn check_status(&self, check: &mut ExchangeRuleCheck) {
        if !matches!(self.status.as_deref(), Some("TRADING")) {
            check.block(
                "symbol-not-trading",
                format!(
                    "symbol {} status is {}",
                    self.symbol,
                    self.status.as_deref().unwrap_or("<missing>")
                ),
            );
        }
    }

    fn check_price(&self, intent: &OrderIntent, check: &mut ExchangeRuleCheck) {
        let Some(price) = exchange_price(intent) else {
            return;
        };
        let Some(filter) = self.price else {
            check.block(
                "price-filter-missing",
                format!("symbol {} is missing PRICE_FILTER", self.symbol),
            );
            return;
        };
        check_min_max(check, "price", price, filter.min, filter.max);
        check_step(
            check,
            "price",
            price,
            step_base(self.market, filter.min),
            filter.tick_size,
            "price-tick-size",
        );
    }

    fn check_quantity(&self, intent: &OrderIntent, check: &mut ExchangeRuleCheck) {
        let Some(filter) = self.lot_size else {
            check.block(
                "lot-size-filter-missing",
                format!("symbol {} is missing LOT_SIZE", self.symbol),
            );
            return;
        };
        self.check_quantity_filter("quantity", filter, intent, check);
        if matches!(intent.spec, OrderSpec::Market { .. })
            && let Some(filter) = self.market_lot_size
        {
            self.check_quantity_filter("market-quantity", filter, intent, check);
        }
    }

    fn check_quantity_filter(
        &self,
        field: &str,
        filter: QuantityFilter,
        intent: &OrderIntent,
        check: &mut ExchangeRuleCheck,
    ) {
        check_min_max(check, field, &intent.quantity, filter.min, filter.max);
        check_step(
            check,
            field,
            &intent.quantity,
            step_base(self.market, filter.min),
            filter.step_size,
            &format!("{field}-step-size"),
        );
    }

    fn check_notional(&self, intent: &OrderIntent, check: &mut ExchangeRuleCheck) {
        let Some(notional) = exchange_rule_notional(intent, check) else {
            return;
        };
        let has_notional_filter = self.min_notional.is_some() || self.notional.is_some();
        if let Some(filter) = self.min_notional
            && filter.applies_to(intent, self.market, "MIN_NOTIONAL", check)
            && notional.0 < filter.min
        {
            check.block(
                "min-notional",
                format!(
                    "notional {notional} is below exchange MIN_NOTIONAL {}",
                    filter.min
                ),
            );
        }
        if let Some(filter) = self.notional {
            filter.check(intent, &notional, check);
        }
        if !has_notional_filter {
            check.block(
                "notional-filter-missing",
                format!(
                    "symbol {} has no applicable MIN_NOTIONAL or NOTIONAL filter",
                    self.symbol
                ),
            );
        }
    }
}

impl PriceFilter {
    fn from_filter(filter: &Value) -> Result<Self> {
        Ok(Self {
            min: filter_decimal(filter, "minPrice")?,
            max: filter_decimal(filter, "maxPrice")?,
            tick_size: filter_decimal(filter, "tickSize")?,
        })
    }
}

impl QuantityFilter {
    fn from_filter(filter: &Value) -> Result<Self> {
        Ok(Self {
            min: filter_decimal(filter, "minQty")?,
            max: filter_decimal(filter, "maxQty")?,
            step_size: filter_decimal(filter, "stepSize")?,
        })
    }
}

impl MinNotionalFilter {
    fn from_filter(market: Market, filter: &Value) -> Result<Self> {
        let min = match market {
            Market::Spot => required_filter_decimal(filter, "minNotional")?,
            Market::UsdsFutures => required_filter_decimal(filter, "notional")?,
        };
        Ok(Self {
            min,
            apply_to_market: filter_bool(filter, "applyToMarket"),
        })
    }

    fn applies_to(
        self,
        intent: &OrderIntent,
        market: Market,
        filter_name: &str,
        check: &mut ExchangeRuleCheck,
    ) -> bool {
        if !matches!(intent.spec, OrderSpec::Market { .. }) {
            return true;
        }
        match market {
            Market::UsdsFutures => true,
            Market::Spot => match self.apply_to_market {
                Some(value) => value,
                None => {
                    check.block(
                        "notional-market-flag-missing",
                        format!("{filter_name} is missing applyToMarket for a market order"),
                    );
                    false
                }
            },
        }
    }
}

impl NotionalFilter {
    fn from_filter(filter: &Value) -> Result<Self> {
        Ok(Self {
            min: filter_decimal(filter, "minNotional")?,
            max: filter_decimal(filter, "maxNotional")?,
            apply_min_to_market: filter_bool(filter, "applyMinToMarket"),
            apply_max_to_market: filter_bool(filter, "applyMaxToMarket"),
        })
    }

    fn check(self, intent: &OrderIntent, notional: &DecimalValue, check: &mut ExchangeRuleCheck) {
        let min_applies =
            self.bound_applies(intent, self.apply_min_to_market, "applyMinToMarket", check);
        let max_applies =
            self.bound_applies(intent, self.apply_max_to_market, "applyMaxToMarket", check);
        if min_applies
            && let Some(minimum) = self.min
            && notional.0 < minimum
        {
            check.block(
                "notional-below-min",
                format!("notional {notional} is below exchange minimum {minimum}"),
            );
        }
        if max_applies
            && let Some(maximum) = self.max
            && maximum > Decimal::ZERO
            && notional.0 > maximum
        {
            check.block(
                "notional-above-max",
                format!("notional {notional} is above exchange maximum {maximum}"),
            );
        }
    }

    fn bound_applies(
        self,
        intent: &OrderIntent,
        market_flag: Option<bool>,
        field: &str,
        check: &mut ExchangeRuleCheck,
    ) -> bool {
        if !matches!(intent.spec, OrderSpec::Market { .. }) {
            return true;
        }
        match market_flag {
            Some(value) => value,
            None => {
                check.block(
                    "notional-market-flag-missing",
                    format!("NOTIONAL is missing {field} for a market order"),
                );
                false
            }
        }
    }
}

fn find_symbol<'a>(exchange_info: &'a Value, symbol: &str) -> Result<&'a Value> {
    let expected = symbol.to_ascii_uppercase();
    exchange_info
        .get("symbols")
        .and_then(Value::as_array)
        .and_then(|symbols| {
            symbols.iter().find(|candidate| {
                candidate
                    .get("symbol")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case(&expected))
            })
        })
        .ok_or_else(|| anyhow!("Binance exchangeInfo did not include symbol {expected}"))
}

fn exchange_price(intent: &OrderIntent) -> Option<&DecimalValue> {
    match &intent.spec {
        OrderSpec::Market { .. } => None,
        OrderSpec::Limit { price, .. } => Some(price),
        OrderSpec::PostOnlyLimit { price } => Some(price),
        OrderSpec::StopLoss { stop_price } | OrderSpec::TakeProfit { stop_price } => {
            Some(stop_price)
        }
    }
}

fn exchange_rule_notional(
    intent: &OrderIntent,
    check: &mut ExchangeRuleCheck,
) -> Option<DecimalValue> {
    match &intent.spec {
        OrderSpec::Market { .. } => {
            check.warn(
                "notional-not-checked",
                "market order notional depends on exchange execution price and is not locally checked from valuation_price",
            );
            None
        }
        OrderSpec::Limit { .. }
        | OrderSpec::PostOnlyLimit { .. }
        | OrderSpec::StopLoss { .. }
        | OrderSpec::TakeProfit { .. } => {
            let Some(notional) = intent.notional_usdt() else {
                check.block("order-notional-overflow", "order notional overflowed");
                return None;
            };
            Some(notional)
        }
    }
}

fn check_min_max(
    check: &mut ExchangeRuleCheck,
    field: &str,
    value: &DecimalValue,
    minimum: Option<Decimal>,
    maximum: Option<Decimal>,
) {
    if let Some(minimum) = minimum
        && minimum > Decimal::ZERO
        && value.0 < minimum
    {
        check.block(
            &format!("{field}-below-min"),
            format!("{field} {value} is below exchange minimum {minimum}"),
        );
    }
    if let Some(maximum) = maximum
        && maximum > Decimal::ZERO
        && value.0 > maximum
    {
        check.block(
            &format!("{field}-above-max"),
            format!("{field} {value} is above exchange maximum {maximum}"),
        );
    }
}

fn check_step(
    check: &mut ExchangeRuleCheck,
    field: &str,
    value: &DecimalValue,
    base: Decimal,
    step: Option<Decimal>,
    code: &str,
) {
    let Some(step) = step.filter(|step| *step > Decimal::ZERO) else {
        return;
    };
    let remainder = (value.0 - base) % step;
    if !remainder.is_zero() {
        check.block(
            code,
            format!("{field} {value} does not align with exchange step {step}"),
        );
    }
}

fn step_base(market: Market, minimum: Option<Decimal>) -> Decimal {
    match market {
        Market::Spot => Decimal::ZERO,
        Market::UsdsFutures => minimum
            .filter(|minimum| *minimum > Decimal::ZERO)
            .unwrap_or(Decimal::ZERO),
    }
}

fn filter<'a>(symbol: &'a Value, filter_type: &str) -> Option<&'a Value> {
    symbol
        .get("filters")
        .and_then(Value::as_array)
        .and_then(|filters| {
            filters.iter().find(|filter| {
                filter
                    .get("filterType")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == filter_type)
            })
        })
}

fn required_filter_decimal(filter: &Value, field: &str) -> Result<Decimal> {
    filter_decimal(filter, field)?.ok_or_else(|| {
        anyhow!(
            "Binance {} filter is missing required decimal field {field}",
            filter
                .get("filterType")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
        )
    })
}

fn filter_decimal(filter: &Value, field: &str) -> Result<Option<Decimal>> {
    filter
        .get(field)
        .and_then(Value::as_str)
        .map(|value| {
            value
                .parse::<Decimal>()
                .with_context(|| format!("invalid Binance filter decimal {field}={value}"))
        })
        .transpose()
}

fn filter_bool(filter: &Value, field: &str) -> Option<bool> {
    filter.get(field).and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::{Environment, OrderSide, Provider, TimeInForce};
    use serde_json::json;

    #[test]
    fn accepts_order_that_matches_price_quantity_and_notional_filters() {
        let intent = limit_order(Market::Spot, "0.001", "50000");
        let check =
            check_order_exchange_rules(&intent, &spot_exchange_info(true)).expect("rule check");

        assert!(check.allowed, "unexpected findings: {:?}", check.findings);
    }

    #[test]
    fn blocks_missing_price_filter_for_limit_orders() {
        let payload = json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "filters": [
                    {"filterType": "LOT_SIZE", "minQty": "0.0001", "maxQty": "10", "stepSize": "0.0001"},
                    {"filterType": "MIN_NOTIONAL", "minNotional": "10", "applyToMarket": true}
                ]
            }]
        });
        let check =
            check_order_exchange_rules(&limit_order(Market::Spot, "0.001", "50000"), &payload)
                .expect("rule check");

        assert_code(&check, "price-filter-missing");
    }

    #[test]
    fn respects_spot_market_notional_flags() {
        let intent = market_order(Market::Spot, "0.0001", "50000");
        let check =
            check_order_exchange_rules(&intent, &spot_exchange_info(false)).expect("rule check");

        assert!(check.allowed, "unexpected findings: {:?}", check.findings);
        assert_code(&check, "notional-not-checked");
        assert!(
            !check
                .findings
                .iter()
                .any(|finding| finding.code == "min-notional"),
            "market notional must not be checked from valuation price: {:?}",
            check.findings
        );
    }

    #[test]
    fn market_notional_does_not_depend_on_local_valuation_price() {
        let payload = json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "filters": [
                    {"filterType": "LOT_SIZE", "minQty": "0.0001", "maxQty": "10", "stepSize": "0.0001"},
                    {"filterType": "MIN_NOTIONAL", "minNotional": "10"}
                ]
            }]
        });
        let check =
            check_order_exchange_rules(&market_order(Market::Spot, "0.0001", "0.01"), &payload)
                .expect("rule check");

        assert!(check.allowed, "unexpected findings: {:?}", check.findings);
        assert_code(&check, "notional-not-checked");
        assert!(
            !check
                .findings
                .iter()
                .any(|finding| finding.code == "min-notional"),
            "local valuation price must not create exchange min-notional failures: {:?}",
            check.findings
        );
    }

    #[test]
    fn market_orders_check_lot_size_and_market_lot_size() {
        let payload = json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "filters": [
                    {"filterType": "LOT_SIZE", "minQty": "0.0001", "maxQty": "10", "stepSize": "0.0001"},
                    {"filterType": "MARKET_LOT_SIZE", "minQty": "0.0002", "maxQty": "0.001", "stepSize": "0.0002"},
                    {"filterType": "MIN_NOTIONAL", "minNotional": "10", "applyToMarket": true}
                ]
            }]
        });
        let check =
            check_order_exchange_rules(&market_order(Market::Spot, "0.00015", "50000"), &payload)
                .expect("rule check");

        assert_code(&check, "quantity-step-size");
        assert_code(&check, "market-quantity-below-min");
        assert_code(&check, "market-quantity-step-size");
    }

    #[test]
    fn blocks_futures_min_notional_from_notional_field() {
        let intent = limit_order(Market::UsdsFutures, "0.0001", "50000");
        let check =
            check_order_exchange_rules(&intent, &futures_exchange_info()).expect("rule check");

        assert_code(&check, "min-notional");
    }

    #[test]
    fn spot_steps_are_zero_based() {
        let intent = limit_order(Market::Spot, "0.001", "0.015");
        let check =
            check_order_exchange_rules(&intent, &spot_exchange_info(true)).expect("rule check");

        assert_code(&check, "price-tick-size");
    }

    #[test]
    fn futures_steps_are_min_offset_based() {
        let intent = limit_order(Market::UsdsFutures, "0.001", "0.015");
        let check =
            check_order_exchange_rules(&intent, &futures_exchange_info()).expect("rule check");

        assert!(
            !check
                .findings
                .iter()
                .any(|finding| finding.code == "price-tick-size"),
            "futures min-offset tick check should allow 0.015: {:?}",
            check.findings
        );
    }

    #[test]
    fn blocks_unaligned_quantity_and_low_notional() {
        let intent = limit_order(Market::Spot, "0.00015", "50000.01");
        let check =
            check_order_exchange_rules(&intent, &spot_exchange_info(true)).expect("rule check");

        assert_code(&check, "quantity-step-size");
        assert_code(&check, "min-notional");
    }

    #[test]
    fn blocks_non_trading_symbol() {
        let payload = json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "HALT",
                "filters": [
                    {"filterType": "PRICE_FILTER", "minPrice": "0.01", "maxPrice": "1000000", "tickSize": "0.01"},
                    {"filterType": "LOT_SIZE", "minQty": "0.0001", "maxQty": "10", "stepSize": "0.0001"},
                    {"filterType": "MIN_NOTIONAL", "minNotional": "10", "applyToMarket": true}
                ]
            }]
        });
        let check =
            check_order_exchange_rules(&limit_order(Market::Spot, "0.001", "50000"), &payload)
                .expect("rule check");

        assert_code(&check, "symbol-not-trading");
    }

    fn assert_code(check: &ExchangeRuleCheck, code: &str) {
        assert!(
            check.findings.iter().any(|finding| finding.code == code),
            "expected {code}, got {:?}",
            check.findings
        );
    }

    fn limit_order(market: Market, quantity: &str, price: &str) -> OrderIntent {
        order(
            market,
            quantity,
            OrderSpec::Limit {
                price: price.parse().expect("price"),
                time_in_force: TimeInForce::Gtc,
            },
        )
    }

    fn market_order(market: Market, quantity: &str, valuation_price: &str) -> OrderIntent {
        order(
            market,
            quantity,
            OrderSpec::Market {
                valuation_price: valuation_price.parse().expect("valuation price"),
            },
        )
    }

    fn order(market: Market, quantity: &str, spec: OrderSpec) -> OrderIntent {
        OrderIntent {
            profile: "default".to_string(),
            provider: Provider::Binance,
            environment: Environment::Testnet,
            market,
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            quantity: quantity.parse().expect("quantity"),
            spec,
            reduce_only: false,
            position_side: None,
            client_order_id: "af-test".to_string(),
        }
    }

    fn spot_exchange_info(apply_to_market: bool) -> Value {
        json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "filters": [
                    {
                        "filterType": "PRICE_FILTER",
                        "minPrice": "0.01",
                        "maxPrice": "1000000",
                        "tickSize": "0.01"
                    },
                    {
                        "filterType": "LOT_SIZE",
                        "minQty": "0.0001",
                        "maxQty": "10",
                        "stepSize": "0.0001"
                    },
                    {
                        "filterType": "MIN_NOTIONAL",
                        "minNotional": "10",
                        "applyToMarket": apply_to_market
                    }
                ]
            }]
        })
    }

    fn futures_exchange_info() -> Value {
        json!({
            "symbols": [{
                "symbol": "BTCUSDT",
                "status": "TRADING",
                "filters": [
                    {
                        "filterType": "PRICE_FILTER",
                        "minPrice": "0.005",
                        "maxPrice": "1000000",
                        "tickSize": "0.01"
                    },
                    {
                        "filterType": "LOT_SIZE",
                        "minQty": "0.0001",
                        "maxQty": "10",
                        "stepSize": "0.0001"
                    },
                    {
                        "filterType": "MIN_NOTIONAL",
                        "notional": "10"
                    }
                ]
            }]
        })
    }
}
