use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

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

impl ProfileStore {
    pub fn from_default_dir() -> Result<Self> {
        Ok(Self {
            root: config_dir()?.join("profiles"),
        })
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.toml"))
    }

    pub fn load(&self, name: &str) -> Result<Profile> {
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
}
