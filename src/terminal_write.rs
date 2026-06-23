use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;

pub(crate) fn load_profile(name: &str) -> Result<agent_finance_core::Profile> {
    agent_finance_core::ProfileStore::from_default_dir()?.load(name)
}

pub(crate) fn binance_client(
    profile: &agent_finance_core::Profile,
    timeout_seconds: u64,
) -> Result<agent_finance_binance::BinanceClient> {
    let credentials = agent_finance_binance::BinanceCredentials::from_env(
        &profile.provider.api_key_env,
        &profile.provider.api_secret_env,
    )?;
    agent_finance_binance::BinanceClient::new(
        credentials,
        binance_endpoints(profile),
        timeout_seconds,
    )
}

pub(crate) fn binance_endpoints(
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
pub(crate) enum WriteMode {
    DryRun,
    Test,
    Live,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpectedIntentKind {
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
    pub(crate) fn from_flags(live: bool, test: bool) -> Result<Self> {
        match (live, test) {
            (true, true) => Err(anyhow!("--live and --test are mutually exclusive")),
            (true, false) => Ok(Self::Live),
            (false, true) => Ok(Self::Test),
            (false, false) => Ok(Self::DryRun),
        }
    }

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
}

#[derive(Serialize)]
pub(crate) struct SubmitReport {
    intent_id: String,
    mode: String,
    risk: agent_finance_core::RiskDecision,
    response: SubmitExecution,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SubmitExecution {
    Plan(ExecutionPlan),
    Order(agent_finance_binance::BinanceOrderSubmitResponse),
    FuturesState(agent_finance_binance::BinanceFuturesStateSubmitResponse),
    Raw(serde_json::Value),
}

#[derive(Serialize)]
struct ExecutionPlan {
    dry_run: bool,
    mode: String,
    request: agent_finance_binance::SignedRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    exchange_rules: Option<ExchangeRulePlan>,
    note: &'static str,
}

#[derive(Serialize)]
struct ExchangeRulePlan {
    status: &'static str,
    reason: &'static str,
    request: agent_finance_binance::SignedRequest,
}

pub(crate) async fn submit_intent(
    profile: &agent_finance_core::Profile,
    intent_id: &str,
    expected_kind: ExpectedIntentKind,
    mode: WriteMode,
    timeout_seconds: u64,
) -> Result<SubmitReport> {
    let store = agent_finance_core::IntentStore::from_default_dir()?;
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
    if !mode.consumes_intent() {
        let response = execute_intent(profile, &envelope.kind, mode, timeout_seconds).await?;
        append_audit(
            profile,
            Some(envelope.id.clone()),
            mode.audit_kind(&envelope.kind),
            format!("planned intent {}", envelope.id),
            json!({ "risk": risk, "response": response }),
        )?;
        return Ok(SubmitReport {
            intent_id: envelope.id,
            mode: format!("{mode:?}"),
            risk,
            response,
        });
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
            let payload = submit_audit_payload(&envelope.kind, &risk, &response)?;
            append_audit(
                profile,
                Some(envelope.id.clone()),
                mode.audit_kind(&envelope.kind),
                format!("submitted intent {}", envelope.id),
                payload,
            )?;
            store.mark_submitted(&envelope.id)?;
            Ok(SubmitReport {
                intent_id: envelope.id,
                mode: format!("{mode:?}"),
                risk,
                response,
            })
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
    response: &SubmitExecution,
) -> Result<serde_json::Value> {
    let response = serde_json::to_value(response)?;
    match intent {
        agent_finance_core::IntentKind::Order(intent) => {
            agent_finance_core::live_order_audit_payload(intent, risk, &response)
        }
        _ => Ok(json!({ "risk": risk, "response": response })),
    }
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

pub(crate) fn check_order_with_runtime_limits(
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
        return plan_intent(profile, intent, mode);
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
            Ok(SubmitExecution::Raw(client.cancel_order(intent).await?))
        }
        agent_finance_core::IntentKind::Transfer(intent) => Ok(SubmitExecution::Raw(
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
    mode: WriteMode,
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
        dry_run: matches!(mode, WriteMode::DryRun),
        mode: format!("{mode:?}"),
        request,
        exchange_rules,
        note: "dry-run is offline and does not read Binance API credentials",
    }))
}

pub(crate) fn print_submit_report(json_output: bool, report: &SubmitReport) -> Result<()> {
    print_json_or_text(json_output, report, || {
        let findings = risk_findings_text(&report.risk);
        format!(
            "submitted intent {}\nrisk allowed: {}\n{}{}",
            report.intent_id,
            report.risk.allowed,
            findings,
            serde_json::to_string_pretty(&report.response).unwrap()
        )
    })
}

pub(crate) fn risk_findings_text(risk: &agent_finance_core::RiskDecision) -> String {
    if risk.findings.is_empty() {
        return String::new();
    }
    let mut text = String::from("risk findings:");
    for finding in &risk.findings {
        text.push_str(&format!(
            "\n- {} {}: {}",
            finding.severity, finding.code, finding.message
        ));
    }
    text.push('\n');
    text
}

pub(crate) fn save_intent_with_audit(
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

pub(crate) fn append_audit(
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

pub(crate) fn print_json_or_text<T, F>(json_output: bool, value: &T, text: F) -> Result<()>
where
    T: Serialize,
    F: FnOnce() -> String,
{
    if json_output {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", text());
    }
    Ok(())
}
