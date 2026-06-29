use tui_input::{Input, InputRequest};

#[derive(Debug, Clone)]
pub(crate) struct TicketTextInputState {
    target: TicketTextInputTarget,
    input: Input,
}

impl Default for TicketTextInputState {
    fn default() -> Self {
        Self {
            target: TicketTextInputTarget::OrderQuantity,
            input: Input::default(),
        }
    }
}

impl TicketTextInputState {
    pub(crate) fn reset(&mut self, target: TicketTextInputTarget, value: Option<&str>) {
        self.target = target;
        self.input = value.unwrap_or_default().into();
    }

    pub(crate) fn edit_query(&mut self, request: InputRequest) {
        self.input.handle(request);
    }

    pub(crate) fn target(&self) -> TicketTextInputTarget {
        self.target
    }

    pub(crate) fn query(&self) -> &str {
        self.input.value()
    }

    pub(crate) fn committed_value(&self) -> Option<String> {
        let value = self.input.value().trim();
        (!value.is_empty()).then(|| value.to_string())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TicketTextInputTarget {
    OrderQuantity,
    OrderPrice,
    TransferAmount,
    FuturesLeverage,
}

impl TicketTextInputTarget {
    pub(crate) const fn kind(self) -> TicketTextInputKind {
        match self {
            Self::OrderQuantity | Self::OrderPrice => TicketTextInputKind::Order,
            Self::TransferAmount => TicketTextInputKind::Transfer,
            Self::FuturesLeverage => TicketTextInputKind::FuturesState,
        }
    }

    pub(crate) const fn ticket_label(self) -> &'static str {
        match self {
            Self::OrderQuantity | Self::OrderPrice => "order ticket",
            Self::TransferAmount => "transfer ticket",
            Self::FuturesLeverage => "futures state ticket",
        }
    }

    pub(crate) const fn field_label(self) -> &'static str {
        match self {
            Self::OrderQuantity => "quantity",
            Self::OrderPrice => "price",
            Self::TransferAmount => "amount",
            Self::FuturesLeverage => "leverage",
        }
    }

    pub(crate) const fn placeholder(self) -> &'static str {
        match self {
            Self::OrderQuantity => "0.05",
            Self::OrderPrice => "204.00",
            Self::TransferAmount => "5",
            Self::FuturesLeverage => "3",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TicketTextInputKind {
    Order,
    Transfer,
    FuturesState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_value_trims_and_allows_blank_to_clear() {
        let mut state = TicketTextInputState::default();

        state.reset(TicketTextInputTarget::OrderPrice, Some(" 204 "));

        assert_eq!(state.committed_value().as_deref(), Some("204"));

        state.reset(TicketTextInputTarget::OrderPrice, Some(" "));

        assert_eq!(state.committed_value(), None);
    }
}
