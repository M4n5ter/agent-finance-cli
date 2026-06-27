use agent_finance_core::{
    DecimalValue, FuturesStateChange, Market, OrderIdentifier, OrderKind, OrderSide, OrderSpec,
    TimeInForce, TransferDirection,
    submit::{SubmitIntentKind, SubmitMode},
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct StagedChangeRequest {
    pub id: String,
    pub subject: StagedChangeSubject,
}

impl StagedChangeRequest {
    #[cfg(test)]
    pub fn text(id: &str, intent_kind: SubmitIntentKind, summary: &str) -> Self {
        Self {
            id: id.to_string(),
            subject: StagedChangeSubject::Text {
                intent_kind,
                summary: summary.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StagedSubmitRequest {
    pub id: String,
    pub subject: StagedChangeSubject,
    pub mode: SubmitMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum StagedChangeSubject {
    OrderTicket(OrderTicketReview),
    Cancel(CancelReview),
    Transfer(TransferReview),
    FuturesState(FuturesStateReview),
    #[cfg(test)]
    Text {
        intent_kind: SubmitIntentKind,
        summary: String,
    },
}

impl StagedChangeSubject {
    pub fn intent_kind(&self) -> SubmitIntentKind {
        match self {
            Self::OrderTicket(_) => SubmitIntentKind::Order,
            Self::Cancel(_) => SubmitIntentKind::Cancel,
            Self::Transfer(_) => SubmitIntentKind::Transfer,
            Self::FuturesState(_) => SubmitIntentKind::FuturesState,
            #[cfg(test)]
            Self::Text { intent_kind, .. } => *intent_kind,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::OrderTicket(review) => review.summary(),
            Self::Cancel(review) => review.summary(),
            Self::Transfer(review) => review.summary(),
            Self::FuturesState(review) => review.summary(),
            #[cfg(test)]
            Self::Text { summary, .. } => summary.clone(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::OrderTicket(_) => "order",
            Self::Cancel(_) => "cancel",
            Self::Transfer(_) => "transfer",
            Self::FuturesState(_) => "futures-state",
            #[cfg(test)]
            Self::Text { .. } => "text",
        }
    }

    pub fn submit_request(&self, id: String, mode: SubmitMode) -> Option<StagedSubmitRequest> {
        match self {
            Self::OrderTicket(_) | Self::Cancel(_) | Self::Transfer(_) | Self::FuturesState(_) => {
                Some(StagedSubmitRequest {
                    id,
                    subject: self.clone(),
                    mode,
                })
            }
            #[cfg(test)]
            Self::Text { .. } => Some(StagedSubmitRequest {
                id,
                subject: self.clone(),
                mode,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FuturesStateReview {
    pub profile: String,
    pub change: FuturesStateChange,
    pub effective_mode: SubmitMode,
}

impl FuturesStateReview {
    pub fn summary(&self) -> String {
        format!("futures-state {}", self.change.review_label())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TransferReview {
    pub profile: String,
    pub direction: TransferDirection,
    pub asset: String,
    pub amount: String,
    pub parsed_amount: DecimalValue,
    pub effective_mode: SubmitMode,
}

impl TransferReview {
    pub fn summary(&self) -> String {
        format!("transfer {} {} {}", self.direction, self.amount, self.asset)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CancelReview {
    pub profile: String,
    pub market: Market,
    pub symbol: String,
    pub target: OrderIdentifier,
    pub effective_mode: SubmitMode,
}

impl CancelReview {
    pub fn summary(&self) -> String {
        format!(
            "cancel {} {} [{}]",
            self.market,
            self.symbol,
            self.identifier()
        )
    }

    pub fn identifier(&self) -> String {
        match &self.target {
            OrderIdentifier::OrderId { order_id } => order_id.clone(),
            OrderIdentifier::ClientOrderId { client_order_id } => client_order_id.clone(),
        }
    }

    pub fn target(&self) -> OrderIdentifier {
        self.target.clone()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct OrderTicketReview {
    pub symbol: String,
    pub profile: String,
    pub market: Market,
    pub side: OrderSide,
    pub kind: OrderKind,
    pub quantity: String,
    pub price: Option<String>,
    pub time_in_force: TimeInForce,
    pub reduce_only: bool,
    pub parsed_quantity: DecimalValue,
    pub order_spec: OrderSpec,
    pub effective_mode: SubmitMode,
}

impl OrderTicketReview {
    pub fn summary(&self) -> String {
        format!(
            "{} {} {} {} {} @ {} {}{}",
            self.side,
            self.quantity,
            self.symbol,
            self.market,
            self.kind,
            self.price.as_deref().unwrap_or("market"),
            self.time_in_force,
            if self.reduce_only { " reduce-only" } else { "" }
        )
    }
}
