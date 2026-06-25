use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEnvelope {
    fetched_at_utc: String,
    payload: Value,
}

pub fn read_json(namespace: &str, key: &str, ttl_seconds: u64) -> Option<(String, Value)> {
    let path = cache_path(namespace, key).ok()?;
    let metadata = fs::metadata(&path).ok()?;
    let modified = metadata.modified().ok()?;
    if SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::MAX)
        > Duration::from_secs(ttl_seconds)
    {
        return None;
    }
    let payload = fs::read_to_string(path).ok()?;
    let envelope = serde_json::from_str::<CacheEnvelope>(&payload).ok()?;
    Some((envelope.fetched_at_utc, envelope.payload))
}

pub fn write_json(namespace: &str, key: &str, fetched_at_utc: &str, payload: &Value) -> Result<()> {
    let path = cache_path(namespace, key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache directory {}", parent.display()))?;
    }
    let envelope = CacheEnvelope {
        fetched_at_utc: fetched_at_utc.to_string(),
        payload: payload.clone(),
    };
    fs::write(&path, serde_json::to_string_pretty(&envelope)?)
        .with_context(|| format!("failed to write cache file {}", path.display()))
}

fn cache_path(namespace: &str, key: &str) -> Result<PathBuf> {
    Ok(agent_finance_cache_root()?
        .join(safe_segment(namespace))
        .join(format!("{}.json", safe_segment(key))))
}

pub fn agent_finance_cache_root() -> Result<PathBuf> {
    let root = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .context("HOME or XDG_CACHE_HOME is required for cache")?;
    Ok(root.join("agent-finance"))
}

fn safe_segment(value: &str) -> String {
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(value.as_bytes());
    if encoded.is_empty() {
        "_".to_string()
    } else {
        encoded
    }
}
