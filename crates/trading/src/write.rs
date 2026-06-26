use anyhow::{Result, anyhow};
use chrono::Utc;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct TradingRuntime {
    timeout_seconds: u64,
    proxy: Option<String>,
    no_proxy: bool,
}

impl TradingRuntime {
    pub const fn new(timeout_seconds: u64) -> Self {
        Self {
            timeout_seconds,
            proxy: None,
            no_proxy: false,
        }
    }

    pub fn with_http_policy(timeout_seconds: u64, proxy: Option<String>, no_proxy: bool) -> Self {
        Self {
            timeout_seconds,
            proxy,
            no_proxy,
        }
    }

    pub fn load_profile(&self, name: &str) -> Result<agent_finance_core::Profile> {
        agent_finance_core::ProfileStore::from_default_dir()?.load(name)
    }

    pub async fn account_permissions(
        &self,
        profile: &agent_finance_core::Profile,
    ) -> Result<serde_json::Value> {
        binance_client_with_policy(profile, self.http_policy())?
            .account_permissions()
            .await
    }

    pub async fn run_signed_read(
        &self,
        profile: &agent_finance_core::Profile,
        request: agent_finance_core::SignedReadRequest,
    ) -> Result<agent_finance_core::SignedReadSnapshot> {
        crate::signed_read::run_signed_read(profile, request, self.http_policy()).await
    }

    pub async fn submit_order_intent(
        &self,
        profile: &agent_finance_core::Profile,
        intent_id: &str,
        mode: agent_finance_core::SubmitMode,
    ) -> Result<agent_finance_core::SubmitSnapshot> {
        submit_intent(
            profile,
            intent_id,
            ExpectedIntentKind::Order,
            mode,
            self.timeout_seconds,
        )
        .await
    }

    pub async fn submit_transfer_intent(
        &self,
        profile: &agent_finance_core::Profile,
        intent_id: &str,
        mode: agent_finance_core::SubmitMode,
    ) -> Result<agent_finance_core::SubmitSnapshot> {
        submit_intent(
            profile,
            intent_id,
            ExpectedIntentKind::Transfer,
            mode,
            self.timeout_seconds,
        )
        .await
    }

    pub async fn submit_futures_state_intent(
        &self,
        profile: &agent_finance_core::Profile,
        intent_id: &str,
        mode: agent_finance_core::SubmitMode,
    ) -> Result<agent_finance_core::SubmitSnapshot> {
        submit_intent(
            profile,
            intent_id,
            ExpectedIntentKind::State,
            mode,
            self.timeout_seconds,
        )
        .await
    }

    pub fn check_order_with_runtime_limits(
        &self,
        profile: &agent_finance_core::Profile,
        intent: &agent_finance_core::OrderIntent,
        live: bool,
    ) -> Result<agent_finance_core::RiskDecision> {
        check_order_with_runtime_limits(profile, intent, live)
    }

    pub fn save_intent_with_audit(
        &self,
        profile: &agent_finance_core::Profile,
        envelope: &agent_finance_core::IntentEnvelope,
        risk: &agent_finance_core::RiskDecision,
        summary: String,
    ) -> Result<std::path::PathBuf> {
        save_intent_with_audit(profile, envelope, risk, summary)
    }
}

impl TradingRuntime {
    fn http_policy(&self) -> agent_finance_binance::BinanceHttpPolicy {
        agent_finance_binance::BinanceHttpPolicy::new(
            self.timeout_seconds,
            self.proxy.as_deref(),
            self.no_proxy,
        )
    }
}

pub(crate) fn binance_client(
    profile: &agent_finance_core::Profile,
    timeout_seconds: u64,
) -> Result<agent_finance_binance::BinanceClient> {
    binance_client_with_policy(
        profile,
        agent_finance_binance::BinanceHttpPolicy::new(timeout_seconds, None, false),
    )
}

pub(crate) fn binance_client_with_policy(
    profile: &agent_finance_core::Profile,
    policy: agent_finance_binance::BinanceHttpPolicy,
) -> Result<agent_finance_binance::BinanceClient> {
    let credentials = agent_finance_binance::BinanceCredentials::from_env(
        &profile.provider.api_key_env,
        &profile.provider.api_secret_env,
    )?;
    agent_finance_binance::BinanceClient::with_http_policy(
        credentials,
        binance_endpoints(profile),
        policy,
    )
}

fn binance_endpoints(
    profile: &agent_finance_core::Profile,
) -> agent_finance_binance::BinanceEndpoints {
    agent_finance_binance::BinanceEndpoints::new(
        profile.provider.environment,
        profile.provider.spot_base_url.clone(),
        profile.provider.usds_futures_base_url.clone(),
        profile.provider.sapi_base_url.clone(),
    )
}

#[derive(Debug, Clone, Copy)]
enum WriteMode {
    DryRun,
    Test,
    Live,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedIntentKind {
    Order,
    Transfer,
    State,
}

impl ExpectedIntentKind {
    fn validate(self, intent: &agent_finance_core::IntentKind) -> Result<()> {
        match (self, intent) {
            (
                Self::Order,
                agent_finance_core::IntentKind::Order(_)
                | agent_finance_core::IntentKind::Cancel(_),
            )
            | (Self::Transfer, agent_finance_core::IntentKind::Transfer(_))
            | (Self::State, agent_finance_core::IntentKind::FuturesState(_)) => Ok(()),
            (Self::Order, agent_finance_core::IntentKind::Transfer(_)) => {
                Err(anyhow!("order submit cannot submit a transfer intent"))
            }
            (
                Self::Transfer,
                agent_finance_core::IntentKind::Order(_)
                | agent_finance_core::IntentKind::Cancel(_)
                | agent_finance_core::IntentKind::FuturesState(_),
            ) => Err(anyhow!("transfer submit can only submit a transfer intent")),
            (Self::Order, agent_finance_core::IntentKind::FuturesState(_)) => {
                Err(anyhow!("order submit cannot submit a futures state intent"))
            }
            (
                Self::State,
                agent_finance_core::IntentKind::Order(_)
                | agent_finance_core::IntentKind::Cancel(_)
                | agent_finance_core::IntentKind::Transfer(_),
            ) => Err(anyhow!(
                "state submit can only submit a futures state intent"
            )),
        }
    }
}

impl WriteMode {
    fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }

    fn consumes_intent(self) -> bool {
        matches!(self, Self::Live)
    }

    fn audit_kind(
        self,
        intent: &agent_finance_core::IntentKind,
    ) -> agent_finance_core::AuditEventKind {
        match (self, intent) {
            (Self::DryRun, _) => agent_finance_core::AuditEventKind::DryRun,
            (Self::Test, _) => agent_finance_core::AuditEventKind::TestSubmit,
            (Self::Live, agent_finance_core::IntentKind::Cancel(_)) => {
                agent_finance_core::AuditEventKind::Cancel
            }
            (Self::Live, agent_finance_core::IntentKind::Transfer(_)) => {
                agent_finance_core::AuditEventKind::Transfer
            }
            (Self::Live, agent_finance_core::IntentKind::FuturesState(_)) => {
                agent_finance_core::AuditEventKind::StateChange
            }
            (Self::Live, agent_finance_core::IntentKind::Order(_)) => {
                agent_finance_core::AuditEventKind::LiveSubmit
            }
        }
    }

    fn binance_mode(self) -> Option<agent_finance_binance::BinanceRequestMode> {
        match self {
            Self::DryRun => None,
            Self::Test => Some(agent_finance_binance::BinanceRequestMode::Test),
            Self::Live => Some(agent_finance_binance::BinanceRequestMode::Live),
        }
    }

    fn snapshot_mode(self) -> agent_finance_core::SubmitMode {
        match self {
            Self::DryRun => agent_finance_core::SubmitMode::DryRun,
            Self::Test => agent_finance_core::SubmitMode::Test,
            Self::Live => agent_finance_core::SubmitMode::Live,
        }
    }
}

impl From<agent_finance_core::SubmitMode> for WriteMode {
    fn from(mode: agent_finance_core::SubmitMode) -> Self {
        match mode {
            agent_finance_core::SubmitMode::DryRun => Self::DryRun,
            agent_finance_core::SubmitMode::Test => Self::Test,
            agent_finance_core::SubmitMode::Live => Self::Live,
        }
    }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum SubmitExecution {
    Plan(ExecutionPlan),
    Order(agent_finance_binance::BinanceOrderSubmitResponse),
    Cancel(serde_json::Value),
    Transfer(serde_json::Value),
    FuturesState(agent_finance_binance::BinanceFuturesStateSubmitResponse),
}

#[derive(serde::Serialize)]
struct ExecutionPlan {
    request: agent_finance_binance::SignedRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    exchange_rules: Option<ExchangeRulePlan>,
    note: &'static str,
}

#[derive(serde::Serialize)]
struct ExchangeRulePlan {
    status: &'static str,
    reason: &'static str,
    request: agent_finance_binance::SignedRequest,
}

async fn submit_intent(
    profile: &agent_finance_core::Profile,
    intent_id: &str,
    expected_kind: ExpectedIntentKind,
    mode: agent_finance_core::SubmitMode,
    timeout_seconds: u64,
) -> Result<agent_finance_core::SubmitSnapshot> {
    let store = agent_finance_core::IntentStore::from_default_dir()?;
    submit_intent_from_store(
        profile,
        &store,
        intent_id,
        expected_kind,
        mode.into(),
        LivePermissionSource::Binance { timeout_seconds },
        timeout_seconds,
    )
    .await
}

async fn submit_intent_from_store(
    profile: &agent_finance_core::Profile,
    store: &agent_finance_core::IntentStore,
    intent_id: &str,
    expected_kind: ExpectedIntentKind,
    mode: WriteMode,
    permission_source: LivePermissionSource,
    timeout_seconds: u64,
) -> Result<agent_finance_core::SubmitSnapshot> {
    let envelope = store.load_for_submit(intent_id)?;
    expected_kind.validate(&envelope.kind)?;
    if matches!(mode, WriteMode::Test)
        && !matches!(envelope.kind, agent_finance_core::IntentKind::Order(_))
    {
        return Err(anyhow!(
            "--test is only supported for order intents with Binance test endpoints"
        ));
    }
    let risk = check_intent(profile, &envelope.kind, mode.is_live())?;
    if !risk.allowed {
        let error = anyhow!("risk policy blocked intent submit");
        return Err(error);
    }
    check_live_permissions(profile, &envelope.kind, mode, permission_source).await?;
    if !mode.consumes_intent() {
        let response = execute_intent(profile, &envelope.kind, mode, timeout_seconds).await?;
        let snapshot = submit_snapshot(profile, &envelope, mode, risk.clone(), response)?;
        append_audit(
            profile,
            Some(envelope.id.clone()),
            mode.audit_kind(&envelope.kind),
            format!("planned intent {}", envelope.id),
            json!({ "risk": risk, "execution": snapshot.execution }),
        )?;
        return Ok(snapshot);
    }
    let envelope = store.claim_for_submit(&envelope.id)?;
    expected_kind.validate(&envelope.kind)?;
    let _audit_lock = live_order_audit_lock(profile, &envelope.kind, mode)?;
    let risk = check_intent(profile, &envelope.kind, mode.is_live())?;
    if !risk.allowed {
        let error = anyhow!("risk policy blocked claimed intent submit");
        store.mark_failed(&envelope.id)?;
        append_audit(
            profile,
            Some(envelope.id.clone()),
            agent_finance_core::AuditEventKind::Error,
            format!("blocked live intent {}", envelope.id),
            json!({ "risk": risk, "error": format!("{error:#}") }),
        )?;
        return Err(error);
    }
    let response = execute_intent(profile, &envelope.kind, mode, timeout_seconds).await;
    match response {
        Ok(response) => {
            let snapshot = submit_snapshot(profile, &envelope, mode, risk.clone(), response)?;
            let payload = submit_audit_payload(&envelope.kind, &risk, &snapshot.execution)?;
            append_audit(
                profile,
                Some(envelope.id.clone()),
                mode.audit_kind(&envelope.kind),
                format!("submitted intent {}", envelope.id),
                payload,
            )?;
            store.mark_submitted(&envelope.id)?;
            Ok(snapshot)
        }
        Err(error) => {
            store.mark_failed(&envelope.id)?;
            append_audit(
                profile,
                Some(envelope.id.clone()),
                agent_finance_core::AuditEventKind::Error,
                format!("failed to submit intent {}", envelope.id),
                json!({ "risk": risk, "error": format!("{error:#}") }),
            )?;
            Err(error)
        }
    }
}

enum LivePermissionSource {
    Binance {
        timeout_seconds: u64,
    },
    #[cfg(test)]
    Static(serde_json::Value),
}

async fn check_live_permissions(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::IntentKind,
    mode: WriteMode,
    source: LivePermissionSource,
) -> Result<()> {
    if !matches!(mode, WriteMode::Live) || !profile.provider.environment.is_live() {
        return Ok(());
    }
    let payload;
    let payload = match source {
        LivePermissionSource::Binance { timeout_seconds } => {
            payload = binance_client(profile, timeout_seconds)?
                .account_permissions()
                .await?;
            &payload
        }
        #[cfg(test)]
        LivePermissionSource::Static(ref payload) => payload,
    };
    let checks = agent_finance_binance::intent_permission_checks(intent, payload);
    if let Some(message) = agent_finance_binance::blocking_permission_error(&checks) {
        return Err(anyhow!("{message}"));
    }
    Ok(())
}

fn live_order_audit_lock(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::IntentKind,
    mode: WriteMode,
) -> Result<Option<agent_finance_core::AuditScopeLock>> {
    if !matches!(mode, WriteMode::Live)
        || !matches!(intent, agent_finance_core::IntentKind::Order(_))
    {
        return Ok(None);
    }
    let scope = format!(
        "daily-order-notional:{}:{}:{}:{}",
        profile.name,
        profile.provider.provider,
        profile.provider.environment,
        Utc::now().date_naive()
    );
    agent_finance_core::AuditScopeLock::acquire(&scope).map(Some)
}

fn submit_audit_payload(
    intent: &agent_finance_core::IntentKind,
    risk: &agent_finance_core::RiskDecision,
    execution: &agent_finance_core::SubmitExecutionSnapshot,
) -> Result<serde_json::Value> {
    match intent {
        agent_finance_core::IntentKind::Order(intent) => {
            agent_finance_core::live_order_audit_payload(intent, risk, execution)
        }
        _ => Ok(json!({ "risk": risk, "execution": execution })),
    }
}

fn submit_snapshot(
    profile: &agent_finance_core::Profile,
    envelope: &agent_finance_core::IntentEnvelope,
    mode: WriteMode,
    risk: agent_finance_core::RiskDecision,
    response: SubmitExecution,
) -> Result<agent_finance_core::SubmitSnapshot> {
    let execution = submit_execution_snapshot(response)?;
    Ok(agent_finance_core::SubmitSnapshot::from_envelope(
        profile,
        envelope,
        mode.snapshot_mode(),
        risk,
        execution,
    ))
}

fn submit_execution_snapshot(
    response: SubmitExecution,
) -> Result<agent_finance_core::SubmitExecutionSnapshot> {
    let kind = match &response {
        SubmitExecution::Plan(_) => agent_finance_core::SubmitExecutionKind::Plan,
        SubmitExecution::Order(_) => agent_finance_core::SubmitExecutionKind::OrderSubmit,
        SubmitExecution::Cancel(_) => agent_finance_core::SubmitExecutionKind::Cancel,
        SubmitExecution::Transfer(_) => agent_finance_core::SubmitExecutionKind::Transfer,
        SubmitExecution::FuturesState(_) => agent_finance_core::SubmitExecutionKind::FuturesState,
    };
    Ok(agent_finance_core::SubmitExecutionSnapshot {
        kind,
        payload: serde_json::to_value(response)?,
    })
}

fn check_intent(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::IntentKind,
    live: bool,
) -> Result<agent_finance_core::RiskDecision> {
    match intent {
        agent_finance_core::IntentKind::Order(intent) => {
            check_order_with_runtime_limits(profile, intent, live)
        }
        agent_finance_core::IntentKind::Cancel(intent) => Ok(
            agent_finance_core::check_cancel_intent(profile, intent, live),
        ),
        agent_finance_core::IntentKind::Transfer(intent) => Ok(
            agent_finance_core::check_transfer_intent(profile, intent, live),
        ),
        agent_finance_core::IntentKind::FuturesState(intent) => Ok(
            agent_finance_core::check_futures_state_intent(profile, intent, live),
        ),
    }
}

fn check_order_with_runtime_limits(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::OrderIntent,
    live: bool,
) -> Result<agent_finance_core::RiskDecision> {
    let runtime = if live {
        agent_finance_core::OrderRuntimeRisk {
            daily_order_notional_used_utc: Some(
                agent_finance_core::daily_live_order_notional_used_today(profile)?,
            ),
        }
    } else {
        agent_finance_core::OrderRuntimeRisk::default()
    };
    Ok(agent_finance_core::check_order_intent_with_runtime(
        profile, intent, live, &runtime,
    ))
}

async fn execute_intent(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::IntentKind,
    mode: WriteMode,
    timeout_seconds: u64,
) -> Result<SubmitExecution> {
    if matches!(mode, WriteMode::DryRun) {
        return plan_intent(profile, intent);
    }
    let Some(binance_mode) = mode.binance_mode() else {
        unreachable!("dry-run returned above");
    };
    let client = binance_client(profile, timeout_seconds)?;
    match intent {
        agent_finance_core::IntentKind::Order(intent) => Ok(SubmitExecution::Order(
            client.submit_order(intent, binance_mode).await?,
        )),
        agent_finance_core::IntentKind::Cancel(intent) => {
            Ok(SubmitExecution::Cancel(client.cancel_order(intent).await?))
        }
        agent_finance_core::IntentKind::Transfer(intent) => Ok(SubmitExecution::Transfer(
            client.submit_transfer(intent, binance_mode).await?,
        )),
        agent_finance_core::IntentKind::FuturesState(intent) => Ok(SubmitExecution::FuturesState(
            client.submit_futures_state(intent).await?,
        )),
    }
}

fn plan_intent(
    profile: &agent_finance_core::Profile,
    intent: &agent_finance_core::IntentKind,
) -> Result<SubmitExecution> {
    let planner = agent_finance_binance::BinancePlanner::new(binance_endpoints(profile));
    let request = match intent {
        agent_finance_core::IntentKind::Order(intent) => planner.order_request(intent, false)?,
        agent_finance_core::IntentKind::Cancel(intent) => planner.cancel_request(intent)?,
        agent_finance_core::IntentKind::Transfer(intent) => planner.transfer_request(intent)?,
        agent_finance_core::IntentKind::FuturesState(intent) => {
            planner.futures_state_request(intent)?
        }
    };
    let exchange_rules = match intent {
        agent_finance_core::IntentKind::Order(intent) => Some(ExchangeRulePlan {
            status: "not-checked",
            reason: "dry-run is offline; locally checkable exchange rules are checked before --test or --live submit",
            request: planner.exchange_info_request(intent.market, &intent.symbol),
        }),
        _ => None,
    };
    Ok(SubmitExecution::Plan(ExecutionPlan {
        request,
        exchange_rules,
        note: "dry-run is offline and does not read Binance API credentials",
    }))
}

fn save_intent_with_audit(
    profile: &agent_finance_core::Profile,
    envelope: &agent_finance_core::IntentEnvelope,
    risk: &agent_finance_core::RiskDecision,
    summary: String,
) -> Result<std::path::PathBuf> {
    let path = agent_finance_core::IntentStore::from_default_dir()?.save(envelope)?;
    append_audit(
        profile,
        Some(envelope.id.clone()),
        agent_finance_core::AuditEventKind::IntentCreated,
        summary,
        json!({ "intent": envelope, "risk": risk, "path": path }),
    )?;
    Ok(path)
}

fn append_audit(
    profile: &agent_finance_core::Profile,
    intent_id: Option<String>,
    kind: agent_finance_core::AuditEventKind,
    summary: String,
    payload: serde_json::Value,
) -> Result<()> {
    let event = agent_finance_core::AuditEvent {
        timestamp_utc: Utc::now(),
        profile: profile.name.clone(),
        provider: profile.provider.provider.to_string(),
        environment: profile.provider.environment.to_string(),
        intent_id,
        kind,
        summary,
        payload,
    };
    agent_finance_core::append_audit_event(&event)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_finance_core::intent::IntentStatus;
    use agent_finance_core::{
        Environment, FuturesStateChange, FuturesStateIntent, FuturesStatePolicy, MarginType,
        Provider, ProviderConfig, RiskPolicy,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn live_permission_failure_does_not_claim_intent() {
        let profile = test_profile();
        let intent = FuturesStateIntent {
            profile: profile.name.clone(),
            provider: Provider::Binance,
            environment: Environment::Live,
            change: FuturesStateChange::MarginType {
                symbol: "BTCUSDT".to_string(),
                margin_type: MarginType::Isolated,
            },
        };
        let envelope = agent_finance_core::create_futures_state_intent(intent, 300).unwrap();
        let intent_id = envelope.id.clone();
        let root = temp_root("permission-preclaim");
        let store = agent_finance_core::IntentStore::from_root(root.join("intents"));
        store.save(&envelope).unwrap();

        let result = submit_intent_from_store(
            &profile,
            &store,
            &intent_id,
            ExpectedIntentKind::State,
            WriteMode::Live,
            LivePermissionSource::Static(json!({
                "enableSpotAndMarginTrading": true,
                "enableFutures": false,
                "permitsUniversalTransfer": true
            })),
            10,
        )
        .await;
        let error = match result {
            Ok(_) => panic!("permission failure should block live submit"),
            Err(error) => error,
        };

        assert!(
            format!("{error:#}").contains("binance-usds-futures"),
            "error should identify the missing permission: {error:#}"
        );
        let saved = store.load(&intent_id).unwrap();
        assert_eq!(saved.metadata.status, IntentStatus::Created);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn testnet_live_submit_does_not_require_live_sapi_permission_probe() {
        let mut profile = test_profile();
        profile.provider.environment = Environment::Testnet;
        let intent = agent_finance_core::IntentKind::FuturesState(FuturesStateIntent {
            profile: profile.name.clone(),
            provider: Provider::Binance,
            environment: Environment::Testnet,
            change: FuturesStateChange::MarginType {
                symbol: "BTCUSDT".to_string(),
                margin_type: MarginType::Isolated,
            },
        });

        check_live_permissions(
            &profile,
            &intent,
            WriteMode::Live,
            LivePermissionSource::Static(json!({
                "enableSpotAndMarginTrading": false,
                "enableFutures": false,
                "permitsUniversalTransfer": false
            })),
        )
        .await
        .expect("testnet writes must not read or require live SAPI permission probes");
    }

    fn test_profile() -> agent_finance_core::Profile {
        agent_finance_core::Profile {
            name: "test".to_string(),
            provider: ProviderConfig {
                provider: Provider::Binance,
                environment: Environment::Live,
                api_key_env: "BINANCE_API_KEY".to_string(),
                api_secret_env: "BINANCE_PRIVATE_KEY".to_string(),
                spot_base_url: None,
                usds_futures_base_url: None,
                sapi_base_url: None,
            },
            permissions: agent_finance_core::ProfilePermissions {
                spot_trading: false,
                usds_futures: true,
                universal_transfer: false,
            },
            risk: RiskPolicy {
                allow_live: true,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::new(),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: vec![FuturesStatePolicy::MarginType {
                    symbol: "BTCUSDT".to_string(),
                    margin_type: MarginType::Isolated,
                }],
            },
        }
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-{name}-{nanos}"))
    }
}
