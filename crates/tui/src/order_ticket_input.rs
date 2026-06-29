use tui_input::{Input, InputRequest};

use crate::order_ticket::OrderTicketField;

#[derive(Debug, Clone)]
pub(crate) struct OrderTicketInputState {
    field: OrderTicketField,
    input: Input,
}

impl Default for OrderTicketInputState {
    fn default() -> Self {
        Self {
            field: OrderTicketField::Quantity,
            input: Input::default(),
        }
    }
}

impl OrderTicketInputState {
    pub(crate) fn reset(&mut self, field: OrderTicketField, value: Option<&str>) {
        self.field = field;
        self.input = value.unwrap_or_default().into();
    }

    pub(crate) fn edit_query(&mut self, request: InputRequest) {
        self.input.handle(request);
    }

    pub(crate) fn field(&self) -> OrderTicketField {
        self.field
    }

    pub(crate) fn query(&self) -> &str {
        self.input.value()
    }

    pub(crate) fn committed_value(&self) -> Option<String> {
        let value = self.input.value().trim();
        (!value.is_empty()).then(|| value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_value_trims_and_allows_blank_to_clear() {
        let mut state = OrderTicketInputState::default();

        state.reset(OrderTicketField::Price, Some(" 204 "));

        assert_eq!(state.committed_value().as_deref(), Some("204"));

        state.reset(OrderTicketField::Price, Some(" "));

        assert_eq!(state.committed_value(), None);
    }
}
