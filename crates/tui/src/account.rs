use agent_finance_core::{
    Environment, Provider, SignedReadRequest, SignedReadSnapshot, SignedReadSnapshotKind,
};
use serde::Serialize;

pub const ACCOUNT_READ_PLAN: [AccountReadPlan; 3] = [
    AccountReadPlan::new(SignedReadSnapshotKind::ApiPermissions, true),
    AccountReadPlan::new(SignedReadSnapshotKind::SpotBalances, false),
    AccountReadPlan::new(SignedReadSnapshotKind::UsdsFuturesPositions, false),
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AccountReadPlan {
    kind: SignedReadSnapshotKind,
    live_only: bool,
}

impl AccountReadPlan {
    pub const fn new(kind: SignedReadSnapshotKind, live_only: bool) -> Self {
        Self { kind, live_only }
    }

    pub fn request(self) -> SignedReadRequest {
        match self.kind {
            SignedReadSnapshotKind::ApiPermissions => SignedReadRequest::ApiPermissions,
            SignedReadSnapshotKind::SpotBalances => SignedReadRequest::SpotBalances,
            SignedReadSnapshotKind::UsdsFuturesPositions => SignedReadRequest::UsdsFuturesPositions,
            SignedReadSnapshotKind::OrderQuery
            | SignedReadSnapshotKind::OpenOrders
            | SignedReadSnapshotKind::TransferHistory => {
                unreachable!("account read plan only contains account-wide signed reads")
            }
        }
    }

    pub const fn kind(self) -> SignedReadSnapshotKind {
        self.kind
    }

    pub const fn live_only(self) -> bool {
        self.live_only
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountSnapshot {
    pub profile: String,
    pub provider: Provider,
    pub environment: Environment,
    pub reads: Vec<SignedReadSnapshot>,
    pub errors: Vec<AccountReadError>,
}

impl AccountSnapshot {
    pub fn new(
        profile: String,
        provider: Provider,
        environment: Environment,
        reads: Vec<SignedReadSnapshot>,
        errors: Vec<AccountReadError>,
    ) -> Self {
        Self {
            profile,
            provider,
            environment,
            reads,
            errors,
        }
    }

    pub fn read(&self, kind: SignedReadSnapshotKind) -> Option<&SignedReadSnapshot> {
        self.reads.iter().find(|read| read.kind == kind)
    }

    pub fn has_data(&self) -> bool {
        !self.reads.is_empty()
    }

    pub fn complete(&self) -> bool {
        self.errors.is_empty() && self.reads.len() == ACCOUNT_READ_PLAN.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountReadError {
    pub kind: SignedReadSnapshotKind,
    pub error: String,
}

impl AccountReadError {
    pub fn new(kind: SignedReadSnapshotKind, error: impl Into<String>) -> Self {
        Self {
            kind,
            error: error.into(),
        }
    }
}
