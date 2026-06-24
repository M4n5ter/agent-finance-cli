use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::paths::data_dir;
use crate::profile::Profile;
use crate::risk::RiskDecision;
use crate::types::{DecimalValue, OrderIntent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp_utc: DateTime<Utc>,
    pub profile: String,
    pub provider: String,
    pub environment: String,
    pub intent_id: Option<String>,
    pub kind: AuditEventKind,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditEventKind {
    IntentCreated,
    DryRun,
    TestSubmit,
    LiveSubmit,
    Cancel,
    Transfer,
    StateChange,
    Error,
}

pub fn append_audit_event(event: &AuditEvent) -> Result<PathBuf> {
    let path = audit_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open audit log {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(event)?)
        .with_context(|| format!("failed to append audit log {}", path.display()))?;
    Ok(path)
}

pub fn read_audit_events(limit: usize) -> Result<Vec<AuditEvent>> {
    let mut events = read_all_audit_events()?;
    if events.len() > limit {
        events = events.split_off(events.len() - limit);
    }
    Ok(events)
}

pub fn read_all_audit_events() -> Result<Vec<AuditEvent>> {
    let mut events = Vec::new();
    read_audit_log(|event| {
        events.push(event);
        Ok(())
    })?;
    Ok(events)
}

fn read_audit_log(mut handle: impl FnMut(AuditEvent) -> Result<()>) -> Result<()> {
    let path = audit_path()?;
    if !path.exists() {
        return Ok(());
    }
    let file = fs::File::open(&path)
        .with_context(|| format!("failed to open audit log {}", path.display()))?;
    for line in BufReader::new(file).lines() {
        handle(serde_json::from_str(&line?)?)?;
    }
    Ok(())
}

pub fn live_order_audit_payload(
    intent: &OrderIntent,
    risk: &RiskDecision,
    response: &Value,
) -> Result<Value> {
    let notional = intent
        .notional_usdt()
        .ok_or_else(|| anyhow!("order notional overflowed"))?;
    Ok(json!({
        "risk": risk,
        "response": response,
        "order_notional_usdt": notional.to_string(),
    }))
}

pub fn daily_live_order_notional_used_today(profile: &Profile) -> Result<DecimalValue> {
    let mut total = DecimalValue::zero();
    let date = Utc::now().date_naive();
    read_audit_log(|event| {
        add_live_order_notional(profile, date, &event, &mut total)?;
        Ok(())
    })?;
    Ok(total)
}

pub fn daily_live_order_notional_from_events<'a>(
    profile: &Profile,
    date: NaiveDate,
    events: impl IntoIterator<Item = &'a AuditEvent>,
) -> Result<DecimalValue> {
    let mut total = DecimalValue::zero();
    for event in events {
        add_live_order_notional(profile, date, event, &mut total)?;
    }
    Ok(total)
}

fn add_live_order_notional(
    profile: &Profile,
    date: NaiveDate,
    event: &AuditEvent,
    total: &mut DecimalValue,
) -> Result<()> {
    if !is_live_order_event_for(profile, date, event) {
        return Ok(());
    }
    let value = live_order_notional(event)?;
    *total = total.checked_add(&value).ok_or_else(|| {
        anyhow!(
            "daily live order notional overflowed while reading audit event {}",
            audit_event_label(event)
        )
    })?;
    Ok(())
}

fn is_live_order_event_for(profile: &Profile, date: NaiveDate, event: &AuditEvent) -> bool {
    event.profile == profile.name
        && event.provider == profile.provider.provider.to_string()
        && event_environment_matches(profile, &event.environment)
        && event.kind == AuditEventKind::LiveSubmit
        && event.timestamp_utc.date_naive() == date
}

fn event_environment_matches(profile: &Profile, event_environment: &str) -> bool {
    event_environment == profile.provider.environment.to_string()
        || event_environment == format!("{:?}", profile.provider.environment)
}

fn live_order_notional(event: &AuditEvent) -> Result<DecimalValue> {
    let value = event.payload["order_notional_usdt"]
        .as_str()
        .ok_or_else(|| {
            anyhow!(
                "live-submit audit event {} is missing order_notional_usdt",
                audit_event_label(event)
            )
        })?;
    value.parse().with_context(|| {
        format!(
            "failed to parse order_notional_usdt for audit event {}",
            audit_event_label(event)
        )
    })
}

fn audit_event_label(event: &AuditEvent) -> String {
    event
        .intent_id
        .as_deref()
        .unwrap_or("<missing-intent-id>")
        .to_string()
}

pub struct AuditScopeLock {
    path: PathBuf,
}

impl AuditScopeLock {
    pub fn acquire(scope: &str) -> Result<Self> {
        let path = audit_lock_path(scope)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("audit scope {scope} is already locked"))?;
        writeln!(file, "{}", Utc::now().to_rfc3339())?;
        Ok(Self { path })
    }
}

impl Drop for AuditScopeLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn audit_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("audit").join("events.jsonl"))
}

fn audit_lock_path(scope: &str) -> Result<PathBuf> {
    Ok(data_dir()?
        .join("audit")
        .join("locks")
        .join(format!("{}.lock", sanitize_scope(scope))))
}

fn sanitize_scope(scope: &str) -> String {
    let mut sanitized = scope
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect::<String>();
    sanitized.truncate(160);
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        trim_edge_separators(&sanitized).to_string()
    }
}

fn trim_edge_separators(value: &str) -> &str {
    let trimmed = value.trim_matches(['.', '_', '-']);
    if trimmed.is_empty() {
        "default"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_lock_scope_for_paths() {
        assert_eq!(
            sanitize_scope("profile/binance:Live 2026-06-23"),
            "profile_binance_Live_2026-06-23"
        );
        assert_eq!(sanitize_scope("..."), "default");
    }

    #[test]
    fn daily_usage_reads_typed_live_order_payloads() {
        let profile = test_profile();
        let intent = test_order(&profile);
        let risk = RiskDecision::allow();
        let event = AuditEvent {
            timestamp_utc: Utc::now(),
            profile: profile.name.clone(),
            provider: profile.provider.provider.to_string(),
            environment: profile.provider.environment.to_string(),
            intent_id: Some("intent-1".to_string()),
            kind: AuditEventKind::LiveSubmit,
            summary: "submitted intent".to_string(),
            payload: live_order_audit_payload(&intent, &risk, &json!({"ok": true}))
                .expect("payload"),
        };

        let used =
            daily_live_order_notional_from_events(&profile, Utc::now().date_naive(), [&event])
                .expect("daily usage");
        assert_eq!(used.to_string(), "5");
    }

    #[test]
    fn daily_usage_fails_closed_on_missing_notional() {
        let profile = test_profile();
        let event = AuditEvent {
            timestamp_utc: Utc::now(),
            profile: profile.name.clone(),
            provider: profile.provider.provider.to_string(),
            environment: profile.provider.environment.to_string(),
            intent_id: Some("intent-1".to_string()),
            kind: AuditEventKind::LiveSubmit,
            summary: "submitted intent".to_string(),
            payload: json!({ "response": { "ok": true } }),
        };

        assert!(
            daily_live_order_notional_from_events(&profile, Utc::now().date_naive(), [&event])
                .is_err(),
            "matching live-submit events without notional must fail closed"
        );
    }

    fn test_profile() -> Profile {
        Profile {
            name: "default".to_string(),
            provider: crate::types::ProviderConfig {
                provider: crate::types::Provider::Binance,
                environment: crate::types::Environment::Live,
                api_key_env: "KEY".to_string(),
                api_secret_env: "SECRET".to_string(),
                spot_base_url: None,
                usds_futures_base_url: None,
                sapi_base_url: None,
            },
            permissions: crate::types::ProfilePermissions {
                spot_trading: true,
                usds_futures: true,
                universal_transfer: false,
            },
            risk: crate::types::RiskPolicy {
                allow_live: true,
                max_daily_order_notional_usdt: Some("50".parse().expect("decimal")),
                allowed_symbols: Default::default(),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: Vec::new(),
            },
        }
    }

    fn test_order(profile: &Profile) -> OrderIntent {
        OrderIntent {
            profile: profile.name.clone(),
            provider: profile.provider.provider,
            environment: profile.provider.environment,
            market: crate::types::Market::Spot,
            symbol: "BTCUSDT".to_string(),
            side: crate::types::OrderSide::Buy,
            quantity: "0.0001".parse().expect("quantity"),
            spec: crate::types::OrderSpec::Limit {
                price: "50000".parse().expect("price"),
                time_in_force: crate::types::TimeInForce::Gtc,
            },
            reduce_only: false,
            position_side: None,
            client_order_id: "client-1".to_string(),
        }
    }
}
