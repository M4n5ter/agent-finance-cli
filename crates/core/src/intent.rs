use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::data_dir;
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentEnvelope {
    pub id: String,
    pub hash: String,
    pub metadata: IntentMetadata,
    pub kind: IntentKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntentStatus {
    Created,
    Submitting,
    Submitted,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMetadata {
    pub created_at_utc: DateTime<Utc>,
    pub expires_at_utc: DateTime<Utc>,
    pub status: IntentStatus,
}

#[derive(Debug, Clone, Serialize)]
struct IntentHashMaterial<'a> {
    created_at_utc: DateTime<Utc>,
    expires_at_utc: DateTime<Utc>,
    status: IntentStatus,
    kind: &'a IntentKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum IntentKind {
    Order(OrderIntent),
    Cancel(CancelIntent),
    Transfer(TransferIntent),
    FuturesState(FuturesStateIntent),
}

impl IntentKind {
    pub fn required_profile_permissions(&self) -> ProfilePermissionSet {
        match self {
            Self::Order(intent) => intent.required_profile_permissions(),
            Self::Cancel(intent) => intent.required_profile_permissions(),
            Self::Transfer(intent) => intent.required_profile_permissions(),
            Self::FuturesState(intent) => intent.required_profile_permissions(),
        }
    }
}

impl IntentEnvelope {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.metadata.expires_at_utc
    }
}

pub struct IntentStore {
    root: PathBuf,
}

impl IntentStore {
    pub fn from_default_dir() -> Result<Self> {
        Ok(Self {
            root: data_dir()?.join("intents"),
        })
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn save(&self, envelope: &IntentEnvelope) -> Result<PathBuf> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create {}", self.root.display()))?;
        let path = self.path(&envelope.id);
        let mut envelope = envelope.clone();
        envelope.hash = hash_material(&envelope.metadata, &envelope.kind)?;
        let content = serde_json::to_string_pretty(&envelope)?;
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(path)
    }

    pub fn load(&self, id: &str) -> Result<IntentEnvelope> {
        let path = self.path(id);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read intent {}", path.display()))?;
        let envelope: IntentEnvelope = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse intent {}", path.display()))?;
        if envelope.id != id {
            return Err(anyhow!("intent id mismatch in {}", path.display()));
        }
        let expected_hash = hash_material(&envelope.metadata, &envelope.kind)?;
        if envelope.hash != expected_hash {
            return Err(anyhow!(
                "intent {id} hash mismatch; payload may have been edited"
            ));
        }
        Ok(envelope)
    }

    pub fn path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    pub fn load_for_submit(&self, id: &str) -> Result<IntentEnvelope> {
        let envelope = self.load(id)?;
        if envelope.metadata.status != IntentStatus::Created {
            return Err(anyhow!(
                "intent {id} is not submittable; status is {:?}",
                envelope.metadata.status
            ));
        }
        if envelope.is_expired() {
            return Err(anyhow!("intent {id} has expired"));
        }
        Ok(envelope)
    }

    pub fn mark_submitted(&self, id: &str) -> Result<IntentEnvelope> {
        let mut envelope = self.load(id)?;
        ensure_status_progression(id, envelope.metadata.status, IntentStatus::Submitted)?;
        envelope.metadata.status = IntentStatus::Submitted;
        self.save(&envelope)?;
        self.remove_lock(id)?;
        Ok(envelope)
    }

    pub fn mark_failed(&self, id: &str) -> Result<IntentEnvelope> {
        let mut envelope = self.load(id)?;
        ensure_status_progression(id, envelope.metadata.status, IntentStatus::Failed)?;
        envelope.metadata.status = IntentStatus::Failed;
        self.save(&envelope)?;
        self.remove_lock(id)?;
        Ok(envelope)
    }

    pub fn claim_for_submit(&self, id: &str) -> Result<IntentEnvelope> {
        let lock_path = self.lock_path(id);
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create {}", self.root.display()))?;
        let mut lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .with_context(|| format!("intent {id} is already being submitted"))?;
        writeln!(lock, "{}", Utc::now().to_rfc3339())?;

        match self.claim_loaded_intent(id) {
            Ok(envelope) => Ok(envelope),
            Err(error) => {
                let _ = fs::remove_file(&lock_path);
                Err(error)
            }
        }
    }

    fn claim_loaded_intent(&self, id: &str) -> Result<IntentEnvelope> {
        let mut envelope = self.load_for_submit(id)?;
        envelope.metadata.status = IntentStatus::Submitting;
        self.save(&envelope)?;
        Ok(envelope)
    }

    fn remove_lock(&self, id: &str) -> Result<()> {
        match fs::remove_file(self.lock_path(id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("failed to remove intent lock {id}")),
        }
    }

    fn lock_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.lock"))
    }
}

fn ensure_status_progression(id: &str, current: IntentStatus, next: IntentStatus) -> Result<()> {
    if next < current {
        return Err(anyhow!(
            "intent {id} cannot move from status {current:?} back to {next:?}"
        ));
    }
    Ok(())
}

pub fn create_order_intent(intent: OrderIntent, ttl_seconds: i64) -> Result<IntentEnvelope> {
    create(IntentKind::Order(intent), ttl_seconds)
}

pub fn create_cancel_intent(intent: CancelIntent, ttl_seconds: i64) -> Result<IntentEnvelope> {
    create(IntentKind::Cancel(intent), ttl_seconds)
}

pub fn create_transfer_intent(intent: TransferIntent, ttl_seconds: i64) -> Result<IntentEnvelope> {
    create(IntentKind::Transfer(intent), ttl_seconds)
}

pub fn create_futures_state_intent(
    intent: FuturesStateIntent,
    ttl_seconds: i64,
) -> Result<IntentEnvelope> {
    create(IntentKind::FuturesState(intent), ttl_seconds)
}

fn create(kind: IntentKind, ttl_seconds: i64) -> Result<IntentEnvelope> {
    if ttl_seconds <= 0 {
        return Err(anyhow!("intent TTL must be positive"));
    }
    let created_at_utc = Utc::now();
    let expires_at_utc = created_at_utc + TimeDelta::seconds(ttl_seconds);
    let metadata = IntentMetadata {
        created_at_utc,
        expires_at_utc,
        status: IntentStatus::Created,
    };
    let hash = hash_material(&metadata, &kind)?;
    let timestamp = created_at_utc.to_rfc3339_opts(SecondsFormat::Secs, true);
    let id = format!(
        "{}-{}",
        timestamp
            .replace([':', '-'], "")
            .replace('T', "-")
            .replace('Z', ""),
        &hash[..12]
    );
    Ok(IntentEnvelope {
        id,
        hash,
        metadata,
        kind,
    })
}

fn hash_material(metadata: &IntentMetadata, kind: &IntentKind) -> Result<String> {
    let material = IntentHashMaterial {
        created_at_utc: metadata.created_at_utc,
        expires_at_utc: metadata.expires_at_utc,
        status: metadata.status,
        kind,
    };
    let body = serde_json::to_vec(&material)?;
    Ok(hex::encode(Sha256::digest(&body)))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn submitted_intent_is_not_loaded_for_submit_again() {
        let store = IntentStore::from_root(temp_dir("submitted"));
        let envelope = create_cancel_intent(
            CancelIntent {
                profile: "default".to_string(),
                provider: Provider::Binance,
                environment: Environment::Testnet,
                market: Market::Spot,
                symbol: "BTCUSDT".to_string(),
                target: OrderIdentifier::ClientOrderId {
                    client_order_id: "af-test".to_string(),
                },
            },
            300,
        )
        .expect("intent");
        let id = envelope.id.clone();
        store.save(&envelope).expect("save intent");

        store.mark_submitted(&id).expect("mark submitted");

        let error = store
            .load_for_submit(&id)
            .expect_err("submitted intent should be rejected");
        assert!(
            format!("{error:#}").contains("not submittable"),
            "unexpected error: {error:#}"
        );
        let _ = fs::remove_dir_all(store.root);
    }

    #[test]
    fn claimed_intent_cannot_be_claimed_again() {
        let store = IntentStore::from_root(temp_dir("claimed"));
        let envelope = create_cancel_intent(
            CancelIntent {
                profile: "default".to_string(),
                provider: Provider::Binance,
                environment: Environment::Testnet,
                market: Market::Spot,
                symbol: "BTCUSDT".to_string(),
                target: OrderIdentifier::ClientOrderId {
                    client_order_id: "af-test".to_string(),
                },
            },
            300,
        )
        .expect("intent");
        let id = envelope.id.clone();
        store.save(&envelope).expect("save intent");

        let claimed = store.claim_for_submit(&id).expect("claim intent");
        assert_eq!(claimed.metadata.status, IntentStatus::Submitting);

        let error = store
            .claim_for_submit(&id)
            .expect_err("claimed intent should reject a second claim");
        assert!(
            format!("{error:#}").contains("already being submitted"),
            "unexpected error: {error:#}"
        );
        let _ = fs::remove_dir_all(store.root);
    }

    #[test]
    fn edited_intent_payload_is_rejected_on_load() {
        let store = IntentStore::from_root(temp_dir("tampered"));
        let envelope = create_cancel_intent(
            CancelIntent {
                profile: "default".to_string(),
                provider: Provider::Binance,
                environment: Environment::Testnet,
                market: Market::Spot,
                symbol: "BTCUSDT".to_string(),
                target: OrderIdentifier::ClientOrderId {
                    client_order_id: "af-test".to_string(),
                },
            },
            300,
        )
        .expect("intent");
        let id = envelope.id.clone();
        let path = store.save(&envelope).expect("save intent");
        let content = fs::read_to_string(&path).expect("read intent");
        fs::write(&path, content.replace("BTCUSDT", "ETHUSDT")).expect("tamper intent");

        let error = store.load(&id).expect_err("tampered payload should fail");

        assert!(
            format!("{error:#}").contains("hash mismatch"),
            "unexpected error: {error:#}"
        );
        let _ = fs::remove_dir_all(store.root);
    }

    #[test]
    fn submitted_status_cannot_be_edited_back_to_created() {
        let store = IntentStore::from_root(temp_dir("status-rollback"));
        let envelope = create_cancel_intent(
            CancelIntent {
                profile: "default".to_string(),
                provider: Provider::Binance,
                environment: Environment::Testnet,
                market: Market::Spot,
                symbol: "BTCUSDT".to_string(),
                target: OrderIdentifier::ClientOrderId {
                    client_order_id: "af-test".to_string(),
                },
            },
            300,
        )
        .expect("intent");
        let id = envelope.id.clone();
        let path = store.save(&envelope).expect("save intent");
        store.mark_submitted(&id).expect("mark submitted");
        let content = fs::read_to_string(&path).expect("read intent");
        fs::write(
            &path,
            content.replace("\"status\": \"submitted\"", "\"status\": \"created\""),
        )
        .expect("rollback status");

        let error = store
            .load(&id)
            .expect_err("status rollback should fail hash validation");

        assert!(
            format!("{error:#}").contains("hash mismatch"),
            "unexpected error: {error:#}"
        );
        let _ = fs::remove_dir_all(store.root);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-core-{name}-{nanos}"))
    }
}
