use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub command_model: String,
    pub providers: Vec<ProviderCapability>,
    pub safety_model: Vec<String>,
}

impl CapabilityReport {
    pub fn new(providers: Vec<ProviderCapability>) -> Self {
        Self {
            command_model: "capability-first".to_string(),
            providers,
            safety_model: vec![
                "Secrets are referenced by environment variable name; they are never stored in profile or audit logs.".to_string(),
                "Live writes require profile allow_live, declared profile permissions, whitelist checks, intent id, and explicit --live.".to_string(),
                "Audit logging is append-only JSONL in the user data directory.".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapability {
    pub provider: String,
    pub capabilities: Vec<Capability>,
}

impl ProviderCapability {
    pub fn new(provider: impl Into<String>, capabilities: Vec<Capability>) -> Self {
        Self {
            provider: provider.into(),
            capabilities,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub access: String,
    pub markets: Vec<String>,
    pub notes: Vec<String>,
}

impl Capability {
    pub fn new(
        name: impl Into<String>,
        access: impl Into<String>,
        markets: Vec<String>,
        notes: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            access: access.into(),
            markets,
            notes,
        }
    }
}
