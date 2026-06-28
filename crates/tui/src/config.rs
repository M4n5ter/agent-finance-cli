use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use agent_finance_core::paths;
use agent_finance_market::args::{CryptoProvider, Provider};
use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::keymap::KeymapConfig;
use crate::model::{DockedPanels, FloatingPane, FloatingSize, Panel, WorkspaceKind};
use crate::theme::ThemeConfig;

pub const MIN_LEFT_RATIO: u16 = 15;
pub const MAX_LEFT_RATIO: u16 = 35;
pub const MIN_MAIN_RATIO: u16 = 35;
pub const MAX_MAIN_RATIO: u16 = 60;
pub const MIN_RIGHT_RATIO: u16 = 20;
pub const MAX_LEFT_MAIN_RATIO: u16 = 100 - MIN_RIGHT_RATIO;

#[derive(Debug, Clone)]
pub struct TuiLaunch {
    pub symbols: Vec<String>,
    pub config_path: Option<PathBuf>,
    pub no_persist: bool,
    pub workspace: Option<WorkspaceKind>,
    pub profile: Option<String>,
    pub dump_state: Option<TuiDumpOptions>,
    pub tick_rate: Duration,
    pub proxy: Option<String>,
    pub no_proxy: bool,
    pub timeout_seconds: u64,
    pub timezone: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TuiDumpOptions {
    pub wait_seconds: u64,
    pub json: bool,
}

impl TuiLaunch {
    pub fn new(symbols: Vec<String>, config_path: Option<PathBuf>, no_persist: bool) -> Self {
        Self::with_market_runtime(symbols, config_path, no_persist, None, false, 10, "UTC")
    }

    pub fn with_market_runtime(
        symbols: Vec<String>,
        config_path: Option<PathBuf>,
        no_persist: bool,
        proxy: Option<&str>,
        no_proxy: bool,
        timeout_seconds: u64,
        timezone: &str,
    ) -> Self {
        Self {
            symbols,
            config_path,
            no_persist,
            workspace: None,
            profile: None,
            dump_state: None,
            tick_rate: Duration::from_millis(250),
            proxy: proxy.map(ToString::to_string),
            no_proxy,
            timeout_seconds,
            timezone: timezone.to_string(),
        }
    }

    pub fn with_workspace(mut self, workspace: Option<WorkspaceKind>) -> Self {
        self.workspace = workspace;
        self
    }

    pub fn with_profile(mut self, profile: Option<String>) -> Self {
        self.profile = normalize_profile_name(profile);
        self
    }

    pub fn with_dump_state(mut self, dump_state: Option<TuiDumpOptions>) -> Self {
        self.dump_state = dump_state;
        self.no_persist = self.no_persist || dump_state.is_some();
        self
    }

    pub fn load_config(&self) -> Result<TuiConfig> {
        let config = if let Some(path) = self.config_path.as_deref() {
            if path.exists() {
                TuiConfig::load_from(path)?
            } else {
                TuiConfig::default()
            }
        } else {
            default_config_path()
                .filter(|path| path.exists())
                .map(|path| TuiConfig::load_from(&path))
                .transpose()?
                .unwrap_or_default()
        };

        Ok(config)
    }

    pub fn runtime_config(&self, mut config: TuiConfig) -> TuiConfig {
        let symbols = normalize_symbols(&self.symbols);
        if !symbols.is_empty() {
            config.watchlist = symbols;
        }
        if let Some(workspace) = self.workspace {
            config.workspace.current = workspace;
        }
        if let Some(profile) = self.profile.as_ref() {
            config.trading.default_profile = Some(profile.clone());
        }
        config.normalize();
        config
    }

    pub fn persistence_config(
        &self,
        mut config: TuiConfig,
        persisted: &TuiConfig,
        preserve_launch_profile_override: bool,
    ) -> TuiConfig {
        if self.profile.is_some() && preserve_launch_profile_override {
            config.trading.default_profile = persisted.trading.default_profile.clone();
        }
        config
    }

    pub fn persist_config(&self, config: &TuiConfig) -> Result<()> {
        if self.no_persist {
            return Ok(());
        }

        let path = self
            .config_path
            .clone()
            .or_else(default_config_path)
            .context("could not resolve an agent-finance config directory")?;
        config.save_to(&path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct TuiConfig {
    #[serde(default = "default_watchlist")]
    pub watchlist: Vec<String>,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub panels: PanelConfig,
    #[serde(default)]
    pub floating: FloatingConfig,
    #[serde(default)]
    pub refresh: RefreshConfig,
    #[serde(default)]
    pub providers: ProviderConfig,
    #[serde(default, skip_serializing_if = "TradingConfig::is_empty")]
    pub trading: TradingConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default, skip_serializing_if = "KeymapConfig::is_empty")]
    pub keymap: KeymapConfig,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            watchlist: default_watchlist(),
            workspace: WorkspaceConfig::default(),
            layout: LayoutConfig::default(),
            panels: PanelConfig::default(),
            floating: FloatingConfig::default(),
            refresh: RefreshConfig::default(),
            providers: ProviderConfig::default(),
            trading: TradingConfig::default(),
            theme: ThemeConfig::default(),
            keymap: KeymapConfig::default(),
        }
    }
}

impl TuiConfig {
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read TUI config {}", path.display()))?;
        let mut config = toml::from_str::<Self>(&content)
            .with_context(|| format!("failed to parse TUI config {}", path.display()))?;
        config.normalize();
        Ok(config)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let content = toml::to_string_pretty(self).context("failed to serialize TUI config")?;
        fs::write(path, content)
            .with_context(|| format!("failed to write TUI config {}", path.display()))
    }

    pub fn normalize(&mut self) {
        self.watchlist = normalize_symbols(&self.watchlist);
        if self.watchlist.is_empty() {
            self.watchlist = default_watchlist();
        }
        self.workspace.normalize();
        self.layout.normalize();
        self.panels.normalize();
        self.floating.normalize();
        self.refresh.normalize();
        self.trading.normalize();
        self.theme.normalize();
        self.keymap.normalize();
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub current: WorkspaceKind,
}

impl WorkspaceConfig {
    fn normalize(&mut self) {}
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LayoutConfig {
    #[serde(default = "default_left_ratio")]
    pub left_ratio: u16,
    #[serde(default = "default_main_ratio")]
    pub main_ratio: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            left_ratio: default_left_ratio(),
            main_ratio: default_main_ratio(),
        }
    }
}

impl LayoutConfig {
    pub fn normalize(&mut self) {
        self.left_ratio = self.left_ratio.clamp(MIN_LEFT_RATIO, MAX_LEFT_RATIO);
        self.main_ratio = self.main_ratio.clamp(MIN_MAIN_RATIO, MAX_MAIN_RATIO);
        if self.left_ratio + self.main_ratio > MAX_LEFT_MAIN_RATIO {
            self.main_ratio = MAX_LEFT_MAIN_RATIO - self.left_ratio;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PanelConfig {
    #[serde(default = "default_open_panels")]
    pub open: Vec<Panel>,
    #[serde(default = "default_focused_panel")]
    pub focused: Panel,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            open: default_open_panels(),
            focused: default_focused_panel(),
        }
    }
}

impl PanelConfig {
    pub fn normalize(&mut self) {
        add_new_panels_to_legacy_default(&mut self.open);
        let (open, focused) =
            DockedPanels::from_open_focused(self.open.clone(), self.focused).into_parts();
        self.open = open;
        self.focused = focused;
    }
}

const PANELS_ADDED_AFTER_LEGACY_DEFAULT: [Panel; 4] = [
    Panel::Account,
    Panel::OrderTicket,
    Panel::IntentReview,
    Panel::Settings,
];

fn add_new_panels_to_legacy_default(open: &mut Vec<Panel>) {
    let legacy_default = Panel::ALL
        .iter()
        .copied()
        .filter(|panel| !PANELS_ADDED_AFTER_LEGACY_DEFAULT.contains(panel))
        .all(|panel| open.contains(&panel));
    if legacy_default {
        for panel in PANELS_ADDED_AFTER_LEGACY_DEFAULT {
            if !open.contains(&panel) {
                open.push(panel);
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct FloatingConfig {
    #[serde(default)]
    pub panes: Vec<FloatingPane>,
}

impl FloatingConfig {
    pub fn normalize(&mut self) {
        let mut normalized = Vec::new();
        for pane in &self.panes {
            if !pane.kind.persistent() {
                continue;
            }
            if normalized
                .iter()
                .any(|existing: &FloatingPane| existing.kind == pane.kind)
            {
                continue;
            }
            normalized.push(FloatingPane {
                kind: pane.kind,
                size: FloatingSize::resized(pane.size.width_ratio, pane.size.height_ratio),
            });
        }
        self.panes = normalized;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RefreshConfig {
    #[serde(default = "default_price_seconds")]
    pub price_seconds: u64,
    #[serde(default = "default_research_seconds")]
    pub research_seconds: u64,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            price_seconds: default_price_seconds(),
            research_seconds: default_research_seconds(),
        }
    }
}

impl RefreshConfig {
    fn normalize(&mut self) {
        self.price_seconds = self.price_seconds.clamp(2, 300);
        self.research_seconds = self.research_seconds.clamp(60, 86_400);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ProviderConfig {
    #[serde(
        default = "default_equity_provider",
        serialize_with = "serialize_equity_provider",
        deserialize_with = "deserialize_equity_provider"
    )]
    pub equity: EquityProvider,
    #[serde(
        default = "default_crypto_provider",
        serialize_with = "serialize_crypto_provider",
        deserialize_with = "deserialize_crypto_provider"
    )]
    pub crypto: CryptoProvider,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            equity: default_equity_provider(),
            crypto: default_crypto_provider(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct TradingConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
}

impl TradingConfig {
    pub fn normalize(&mut self) {
        self.default_profile = normalize_profile_name(self.default_profile.take());
    }

    pub fn is_empty(&self) -> bool {
        self.default_profile.is_none()
    }
}

fn default_config_path() -> Option<PathBuf> {
    paths::config_dir().ok().map(|path| path.join("tui.toml"))
}

pub(crate) fn normalize_profile_name(profile: Option<String>) -> Option<String> {
    profile
        .map(|profile| profile.trim().to_string())
        .filter(|profile| !profile.is_empty())
}

pub(crate) fn normalize_symbols(symbols: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for symbol in symbols {
        for part in symbol.split(',') {
            let symbol = part.trim().to_ascii_uppercase();
            if !symbol.is_empty() && !normalized.contains(&symbol) {
                normalized.push(symbol);
            }
        }
    }
    normalized
}

fn default_open_panels() -> Vec<Panel> {
    Panel::ALL.to_vec()
}

const fn default_focused_panel() -> Panel {
    Panel::Watchlist
}

fn default_watchlist() -> Vec<String> {
    ["AAPL", "CRDO", "BTCUSDT"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

const fn default_left_ratio() -> u16 {
    24
}

const fn default_main_ratio() -> u16 {
    46
}

const fn default_price_seconds() -> u64 {
    15
}

const fn default_research_seconds() -> u64 {
    900
}

fn default_crypto_provider() -> CryptoProvider {
    CryptoProvider::Auto
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum EquityProvider {
    #[default]
    Auto,
    Yahoo,
    YahooExtended,
    Stooq,
    Robinhood,
}

impl EquityProvider {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Yahoo => "yahoo",
            Self::YahooExtended => "yahoo-extended",
            Self::Stooq => "stooq",
            Self::Robinhood => "robinhood",
        }
    }

    const fn labels() -> &'static [&'static str] {
        &["auto", "yahoo", "yahoo-extended", "stooq", "robinhood"]
    }

    pub const fn provider(self) -> Provider {
        match self {
            Self::Auto => Provider::Auto,
            Self::Yahoo => Provider::Yahoo,
            Self::YahooExtended => Provider::YahooExtended,
            Self::Stooq => Provider::Stooq,
            Self::Robinhood => Provider::Robinhood,
        }
    }
}

impl fmt::Display for EquityProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl FromStr for EquityProvider {
    type Err = String;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "yahoo" => Ok(Self::Yahoo),
            "yahoo-extended" => Ok(Self::YahooExtended),
            "stooq" => Ok(Self::Stooq),
            "robinhood" => Ok(Self::Robinhood),
            _ => Err(format!(
                "invalid TUI equity provider '{}'; expected one of: {}",
                input,
                Self::labels().join(", ")
            )),
        }
    }
}

fn default_equity_provider() -> EquityProvider {
    EquityProvider::Auto
}

fn serialize_equity_provider<S>(
    provider: &EquityProvider,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(provider.label())
}

fn deserialize_equity_provider<'de, D>(
    deserializer: D,
) -> std::result::Result<EquityProvider, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn serialize_crypto_provider<S>(
    provider: &CryptoProvider,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(provider.label())
}

fn deserialize_crypto_provider<'de, D>(
    deserializer: D,
) -> std::result::Result<CryptoProvider, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FloatingKind;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn launch_symbols_override_and_normalize_config_watchlist() {
        let launch = TuiLaunch::new(
            vec![
                " aapl, crdo ".to_string(),
                "AAPL".to_string(),
                "btcusdt".to_string(),
            ],
            None,
            true,
        );
        let config = launch.runtime_config(TuiConfig::default());

        assert_eq!(config.watchlist, ["AAPL", "CRDO", "BTCUSDT"]);
    }

    #[test]
    fn launch_symbols_do_not_mutate_persisted_config() {
        let launch = TuiLaunch::new(vec!["TSLA".to_string()], None, true);
        let persisted = TuiConfig {
            watchlist: vec!["AAPL".to_string(), "CRDO".to_string()],
            ..TuiConfig::default()
        };

        let runtime = launch.runtime_config(persisted.clone());

        assert_eq!(persisted.watchlist, ["AAPL", "CRDO"]);
        assert_eq!(runtime.watchlist, ["TSLA"]);
    }

    #[test]
    fn launch_workspace_override_changes_runtime_workspace_only() {
        let launch =
            TuiLaunch::new(Vec::new(), None, true).with_workspace(Some(WorkspaceKind::Providers));
        let persisted = TuiConfig {
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Research,
            },
            ..TuiConfig::default()
        };

        let runtime = launch.runtime_config(persisted.clone());

        assert_eq!(persisted.workspace.current, WorkspaceKind::Research);
        assert_eq!(runtime.workspace.current, WorkspaceKind::Providers);
    }

    #[test]
    fn dump_state_launch_is_not_persisted() {
        let path = unique_temp_config_path("dump-state-no-persist");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), false).with_dump_state(Some(
            TuiDumpOptions {
                wait_seconds: 0,
                json: true,
            },
        ));

        launch
            .persist_config(&TuiConfig::default())
            .expect("dump-state persistence should be a no-op");

        assert!(!path.exists());
    }

    #[test]
    fn config_roundtrip_preserves_user_visible_preferences() {
        let mut config = TuiConfig {
            watchlist: vec!["lite".to_string(), "aaoi".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Research,
            },
            layout: LayoutConfig {
                left_ratio: 8,
                main_ratio: 90,
            },
            panels: PanelConfig {
                open: vec![Panel::Research, Panel::Watchlist, Panel::Research],
                focused: Panel::ProviderHealth,
            },
            floating: FloatingConfig {
                panes: vec![
                    FloatingPane {
                        kind: FloatingKind::Help,
                        size: FloatingSize::resized(99, 5),
                    },
                    FloatingPane {
                        kind: FloatingKind::Help,
                        size: FloatingSize::resized(40, 40),
                    },
                ],
            },
            refresh: RefreshConfig {
                price_seconds: 1,
                research_seconds: 10,
            },
            providers: ProviderConfig {
                equity: EquityProvider::Yahoo,
                crypto: CryptoProvider::Binance,
            },
            trading: TradingConfig {
                default_profile: Some("mainnet".to_string()),
            },
            theme: ThemeConfig::default(),
            keymap: KeymapConfig::default(),
        };
        config.normalize();

        let encoded = toml::to_string(&config).expect("encode");
        let decoded = toml::from_str::<TuiConfig>(&encoded).expect("decode");

        assert_eq!(decoded.watchlist, ["LITE", "AAOI"]);
        assert_eq!(decoded.workspace.current, WorkspaceKind::Research);
        assert_eq!(decoded.layout.left_ratio, 15);
        assert_eq!(decoded.layout.main_ratio, 60);
        assert_eq!(decoded.panels.open, [Panel::Watchlist, Panel::Research]);
        assert_eq!(decoded.panels.focused, Panel::Watchlist);
        assert_eq!(decoded.floating.panes.len(), 1);
        assert_eq!(
            decoded.floating.panes[0].size,
            FloatingSize::resized(95, 20)
        );
        assert_eq!(decoded.refresh.price_seconds, 2);
        assert_eq!(decoded.refresh.research_seconds, 60);
        assert_eq!(decoded.providers.equity, EquityProvider::Yahoo);
        assert_eq!(decoded.providers.crypto, CryptoProvider::Binance);
        assert_eq!(decoded.trading.default_profile.as_deref(), Some("mainnet"));
        assert_eq!(decoded.theme, ThemeConfig::default());
        assert_eq!(decoded.keymap.normal, KeymapConfig::default().normal);
        assert!(encoded.contains("equity = \"yahoo\""));
        assert!(encoded.contains("crypto = \"binance\""));
        assert!(encoded.contains("[trading]"));
        assert!(encoded.contains("default_profile = \"mainnet\""));
        assert!(encoded.contains("[theme]"));
        assert!(!encoded.contains("[keymap]"));
    }

    #[test]
    fn legacy_default_panel_config_gains_new_panels() {
        let mut config = TuiConfig {
            panels: PanelConfig {
                open: Panel::ALL
                    .into_iter()
                    .filter(|panel| !PANELS_ADDED_AFTER_LEGACY_DEFAULT.contains(panel))
                    .collect(),
                focused: Panel::Watchlist,
            },
            ..TuiConfig::default()
        };

        config.normalize();

        assert!(config.panels.open.contains(&Panel::Account));
        assert!(config.panels.open.contains(&Panel::OrderTicket));
        assert!(config.panels.open.contains(&Panel::IntentReview));
        assert!(config.panels.open.contains(&Panel::Settings));
    }

    #[test]
    fn launch_profile_override_is_runtime_only() {
        let launch =
            TuiLaunch::new(Vec::new(), None, true).with_profile(Some(" live-main ".to_string()));
        let persisted = TuiConfig {
            trading: TradingConfig {
                default_profile: Some("paper".to_string()),
            },
            ..TuiConfig::default()
        };

        let runtime = launch.runtime_config(persisted.clone());
        let export = launch.persistence_config(runtime.clone(), &persisted, true);

        assert_eq!(
            runtime.trading.default_profile.as_deref(),
            Some("live-main")
        );
        assert_eq!(persisted.trading.default_profile.as_deref(), Some("paper"));
        assert_eq!(export.trading.default_profile.as_deref(), Some("paper"));
    }

    #[test]
    fn launch_profile_override_does_not_hide_explicit_trading_config_change() {
        let launch =
            TuiLaunch::new(Vec::new(), None, true).with_profile(Some(" live-main ".to_string()));
        let persisted = TuiConfig {
            trading: TradingConfig {
                default_profile: Some("paper".to_string()),
            },
            ..TuiConfig::default()
        };
        let runtime = launch.runtime_config(persisted.clone());

        let export = launch.persistence_config(runtime.clone(), &persisted, false);

        assert_eq!(export.trading.default_profile.as_deref(), Some("live-main"));
    }

    #[test]
    fn config_rejects_equity_providers_that_do_not_support_tui_quote_and_history() {
        let error = toml::from_str::<TuiConfig>(
            r#"
            [providers]
            equity = "cnbc-extended"
            crypto = "binance"
            "#,
        )
        .expect_err("quote-only provider should not be a valid TUI equity preference");

        assert!(error.to_string().contains("invalid TUI equity provider"));
    }

    #[test]
    fn no_persist_launch_does_not_write_config_file() {
        let path = unique_temp_config_path("no-persist");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), true);

        launch
            .persist_config(&TuiConfig::default())
            .expect("no-persist should be a no-op");

        assert!(!path.exists());
    }

    #[test]
    fn explicit_missing_config_path_starts_from_default_config() {
        let path = unique_temp_config_path("missing-config");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), true);

        let config = launch.load_config().expect("missing config should default");

        assert_eq!(config, TuiConfig::default());
        assert!(!path.exists());
    }

    #[test]
    fn persist_then_load_roundtrips_runtime_layout_config() {
        let path = unique_temp_config_path("persist-roundtrip");
        let launch = TuiLaunch::new(Vec::new(), Some(path.clone()), false);
        let mut config = TuiConfig {
            watchlist: vec!["crdo".to_string(), "lite".to_string()],
            workspace: WorkspaceConfig {
                current: WorkspaceKind::Crypto,
            },
            layout: LayoutConfig {
                left_ratio: 30,
                main_ratio: 42,
            },
            panels: PanelConfig {
                open: vec![Panel::Watchlist, Panel::History],
                focused: Panel::History,
            },
            floating: FloatingConfig {
                panes: vec![
                    FloatingPane {
                        kind: FloatingKind::CommandPalette,
                        size: FloatingSize::resized(70, 40),
                    },
                    FloatingPane {
                        kind: FloatingKind::ProviderDetails,
                        size: FloatingSize::resized(61, 62),
                    },
                ],
            },
            ..TuiConfig::default()
        };
        config.normalize();

        launch.persist_config(&config).expect("persist config");
        let loaded = launch.load_config().expect("load config");
        let _ = fs::remove_file(&path);

        assert_eq!(loaded.watchlist, ["CRDO", "LITE"]);
        assert_eq!(loaded.workspace.current, WorkspaceKind::Crypto);
        assert_eq!(loaded.layout.left_ratio, 30);
        assert_eq!(loaded.layout.main_ratio, 42);
        assert_eq!(loaded.panels.open, [Panel::Watchlist, Panel::History]);
        assert_eq!(loaded.panels.focused, Panel::History);
        assert_eq!(loaded.floating.panes.len(), 1);
        assert_eq!(loaded.floating.panes[0].kind, FloatingKind::ProviderDetails);
        assert_eq!(loaded.floating.panes[0].size, FloatingSize::resized(61, 62));
    }

    fn unique_temp_config_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-finance-tui-{name}-{nanos}.toml"))
    }
}
