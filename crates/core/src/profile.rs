use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::config_dir;
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub provider: ProviderConfig,
    #[serde(default)]
    pub permissions: ProfilePermissions,
    pub risk: RiskPolicy,
}

pub struct ProfileStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProfileWritePlan {
    profile: String,
    path: PathBuf,
    backup_path: Option<PathBuf>,
    content: String,
    old_content_hash: Option<String>,
    replace_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileWriteReport {
    pub profile: String,
    pub path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

impl ProfileStore {
    pub fn from_default_dir() -> Result<Self> {
        Ok(Self {
            root: config_dir()?.join("profiles"),
        })
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.toml"))
    }

    pub fn load(&self, name: &str) -> Result<Profile> {
        validate_profile_name(name)?;
        let path = self.path(name);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read profile {}", path.display()))?;
        let profile: Profile = toml::from_str(&content)
            .with_context(|| format!("failed to parse profile {}", path.display()))?;
        if profile.name != name {
            return Err(anyhow!(
                "profile file name '{}' does not match profile.name '{}'",
                name,
                profile.name
            ));
        }
        Ok(profile)
    }

    pub fn plan_write(&self, profile: &Profile) -> Result<ProfileWritePlan> {
        validate_profile_name(&profile.name)?;
        let content = toml::to_string_pretty(profile).context("failed to encode profile TOML")?;
        let parsed: Profile =
            toml::from_str(&content).context("encoded profile TOML did not roundtrip")?;
        if parsed.name != profile.name {
            return Err(anyhow!(
                "encoded profile name '{}' does not match profile.name '{}'",
                parsed.name,
                profile.name
            ));
        }
        let path = self.path(&profile.name);
        let old_content = if path.exists() {
            Some(
                fs::read_to_string(&path)
                    .with_context(|| format!("failed to read profile {}", path.display()))?,
            )
        } else {
            None
        };
        let replace_existing = old_content.is_some();
        let backup_path = replace_existing.then(|| backup_path(&path));
        Ok(ProfileWritePlan {
            profile: profile.name.clone(),
            path,
            backup_path,
            content,
            old_content_hash: old_content.as_deref().map(content_hash),
            replace_existing,
        })
    }

    pub fn write(&self, profile: &Profile) -> Result<ProfileWriteReport> {
        let plan = self.plan_write(profile)?;
        self.commit_write_plan(plan)
    }

    pub fn commit_write_plan(&self, plan: ProfileWritePlan) -> Result<ProfileWriteReport> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("failed to create profile dir {}", self.root.display()))?;
        validate_profile_name(&plan.profile)?;
        let parsed: Profile =
            toml::from_str(&plan.content).context("planned profile TOML did not parse")?;
        if parsed.name != plan.profile {
            return Err(anyhow!(
                "planned profile content name '{}' does not match profile '{}'",
                parsed.name,
                plan.profile
            ));
        }
        if plan.path != self.path(&plan.profile) {
            return Err(anyhow!(
                "profile write plan path '{}' does not match profile '{}'",
                plan.path.display(),
                plan.profile
            ));
        }
        if plan.backup_path.is_some() != plan.replace_existing {
            return Err(anyhow!(
                "profile write plan backup state does not match replacement state"
            ));
        }
        match (plan.replace_existing, fs::read_to_string(&plan.path)) {
            (true, Ok(current_content)) => {
                let current_hash = content_hash(&current_content);
                let Some(expected_hash) = &plan.old_content_hash else {
                    return Err(anyhow!("profile write plan is missing old content hash"));
                };
                if &current_hash != expected_hash {
                    return Err(anyhow!(
                        "profile {} changed after write plan; rebuild the plan before replacing it",
                        plan.path.display()
                    ));
                }
            }
            (true, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(anyhow!(
                    "profile {} disappeared before write commit",
                    plan.path.display()
                ));
            }
            (true, Err(error)) => {
                return Err(error)
                    .with_context(|| format!("failed to read profile {}", plan.path.display()));
            }
            (false, Ok(_)) => {
                return Err(anyhow!(
                    "profile {} appeared after write plan; rebuild the plan before replacing it",
                    plan.path.display()
                ));
            }
            (false, Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            (false, Err(error)) => {
                return Err(error)
                    .with_context(|| format!("failed to read profile {}", plan.path.display()));
            }
        }
        if let Some(backup_path) = &plan.backup_path {
            fs::copy(&plan.path, backup_path).with_context(|| {
                format!(
                    "failed to back up profile {} to {}",
                    plan.path.display(),
                    backup_path.display()
                )
            })?;
        }
        let temp_path = temp_profile_path(&plan.path);
        fs::write(&temp_path, &plan.content)
            .with_context(|| format!("failed to write temp profile {}", temp_path.display()))?;
        fs::rename(&temp_path, &plan.path).with_context(|| {
            format!(
                "failed to move temp profile {} to {}",
                temp_path.display(),
                plan.path.display()
            )
        })?;
        Ok(ProfileWriteReport {
            profile: plan.profile,
            path: plan.path,
            backup_path: plan.backup_path,
        })
    }
}

impl ProfileWritePlan {
    pub fn profile(&self) -> &str {
        &self.profile
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.trim() != name || name.is_empty() || matches!(name, "." | "..") {
        return Err(anyhow!("profile name must be a non-empty file-safe label"));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(anyhow!(
            "profile name '{name}' must contain only ASCII letters, digits, '.', '_' or '-'"
        ));
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.toml");
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.fZ");
    path.with_file_name(format!("{file_name}.bak-{timestamp}"))
}

fn temp_profile_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.toml");
    path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
}

fn content_hash(content: &str) -> String {
    hex::encode(Sha256::digest(content.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rust_decimal::Decimal;

    use super::*;

    #[test]
    fn write_new_profile_roundtrips_without_backup() {
        let root = TempProfileRoot::new("new-profile");
        let store = ProfileStore::from_root(root.path());
        let plan = store.plan_write(&profile("mainnet")).expect("plan profile");
        assert_eq!(plan.profile(), "mainnet");
        assert!(plan.content().contains("name = \"mainnet\""));
        let report = store.commit_write_plan(plan).expect("write profile");

        assert_eq!(report.profile, "mainnet");
        assert_eq!(report.backup_path, None);
        let loaded = store.load("mainnet").expect("load written profile");
        assert_eq!(loaded.name, "mainnet");
        assert_eq!(loaded.provider.api_secret_env, "BINANCE_PRIVATE_KEY");
    }

    #[test]
    fn replacing_profile_creates_backup_with_previous_content() {
        let root = TempProfileRoot::new("replace-profile");
        let store = ProfileStore::from_root(root.path());
        store.write(&profile("mainnet")).expect("initial write");
        let mut next = profile("mainnet");
        next.risk.allow_live = true;

        let report = store.write(&next).expect("replacement write");

        let backup_path = report.backup_path.expect("backup path");
        let backup = fs::read_to_string(backup_path).expect("backup content");
        assert!(backup.contains("allow_live = false"));
        let loaded = store.load("mainnet").expect("load replacement");
        assert!(loaded.risk.allow_live);
    }

    #[test]
    fn commit_fails_closed_when_new_profile_appears_after_planning() {
        let root = TempProfileRoot::new("appeared-profile");
        let store = ProfileStore::from_root(root.path());
        let plan = store.plan_write(&profile("mainnet")).expect("plan profile");
        fs::create_dir_all(root.path()).expect("profile dir");
        fs::write(store.path("mainnet"), "name = \"other\"\n").expect("racing profile");

        let error = store
            .commit_write_plan(plan)
            .expect_err("racing profile should not be replaced without backup");

        assert!(error.to_string().contains("appeared after write plan"));
    }

    #[test]
    fn commit_fails_closed_when_existing_profile_changes_after_planning() {
        let root = TempProfileRoot::new("changed-profile");
        let store = ProfileStore::from_root(root.path());
        store.write(&profile("mainnet")).expect("initial write");
        let mut next = profile("mainnet");
        next.risk.allow_live = true;
        let plan = store.plan_write(&next).expect("plan profile");
        fs::write(store.path("mainnet"), "name = \"mainnet\"\n").expect("changed profile");

        let error = store
            .commit_write_plan(plan)
            .expect_err("changed profile should not be replaced after confirmation");

        assert!(error.to_string().contains("changed after write plan"));
    }

    #[test]
    fn write_rejects_path_like_profile_names() {
        let root = TempProfileRoot::new("bad-profile");
        let store = ProfileStore::from_root(root.path());
        let error = store
            .write(&profile("../mainnet"))
            .expect_err("path traversal must be rejected");

        assert!(error.to_string().contains("profile name"));
    }

    #[test]
    fn load_rejects_path_like_profile_names() {
        let root = TempProfileRoot::new("bad-load-profile");
        let store = ProfileStore::from_root(root.path());

        let error = store
            .load("../mainnet")
            .expect_err("path traversal must be rejected");

        assert!(error.to_string().contains("profile name"));
    }

    fn profile(name: &str) -> Profile {
        Profile {
            name: name.to_string(),
            provider: ProviderConfig {
                provider: Provider::Binance,
                environment: Environment::Testnet,
                api_key_env: "BINANCE_API_KEY".to_string(),
                api_secret_env: "BINANCE_PRIVATE_KEY".to_string(),
                spot_base_url: None,
                usds_futures_base_url: None,
                sapi_base_url: None,
            },
            permissions: ProfilePermissions {
                spot_trading: true,
                usds_futures: false,
                universal_transfer: false,
            },
            risk: RiskPolicy {
                allow_live: false,
                max_daily_order_notional_usdt: None,
                allowed_symbols: BTreeMap::from([(
                    "BTCUSDT".to_string(),
                    SymbolPolicy {
                        markets: vec![Market::Spot],
                        order_kinds: vec![OrderKind::Limit],
                        max_order_notional_usdt: DecimalValue(
                            "25".parse::<Decimal>().expect("decimal"),
                        ),
                    },
                )]),
                allowed_transfers: Vec::new(),
                allowed_futures_state_changes: Vec::new(),
            },
        }
    }

    struct TempProfileRoot {
        path: PathBuf,
    }

    impl TempProfileRoot {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            Self {
                path: std::env::temp_dir().join(format!("agent-finance-profile-{label}-{nanos}")),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempProfileRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
