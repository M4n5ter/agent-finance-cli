use std::fmt;
use std::str::FromStr;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::command::{ActionId, action_by_id, action_id};
use crate::model::{FloatingKind, Panel};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeymapConfig {
    pub overrides: Vec<KeyBinding>,
    pub normal: Vec<KeyBinding>,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self::from_overrides(Vec::new())
    }
}

impl KeymapConfig {
    pub fn from_overrides(overrides: Vec<KeyBinding>) -> Self {
        let overrides = normalize_bindings(overrides);
        let normal = merge_normal_bindings(&overrides);
        Self { overrides, normal }
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub fn normalize(&mut self) {
        *self = Self::from_overrides(std::mem::take(&mut self.overrides));
    }

    pub fn normal_action(&self, key: KeyEvent) -> Option<ActionId> {
        self.normal
            .iter()
            .find(|binding| binding.key.matches(key))
            .map(|binding| binding.action)
    }
}

impl Serialize for KeymapConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        KeymapConfigSerde {
            overrides: self.overrides.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeymapConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let config = KeymapConfigSerde::deserialize(deserializer)?;
        Ok(Self::from_overrides(config.overrides))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct KeymapConfigSerde {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    overrides: Vec<KeyBinding>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct KeyBinding {
    #[serde(
        serialize_with = "serialize_key_stroke",
        deserialize_with = "deserialize_key_stroke"
    )]
    pub key: KeyStroke,
    #[serde(
        serialize_with = "serialize_action",
        deserialize_with = "deserialize_action"
    )]
    pub action: ActionId,
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct KeyStroke {
    code: KeyStrokeCode,
    modifiers: KeyModifiers,
}

impl KeyStroke {
    pub const fn new(code: KeyStrokeCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::empty(),
        }
    }

    pub const fn with_modifiers(code: KeyStrokeCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    fn matches(self, key: KeyEvent) -> bool {
        if !self.code.matches(key.code) {
            return false;
        }
        if self.code == KeyStrokeCode::BackTab && self.modifiers.is_empty() {
            return key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        }
        if matches!(self.code, KeyStrokeCode::Char(_)) && self.modifiers.is_empty() {
            return key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT;
        }
        key.modifiers == self.modifiers
    }
}

impl fmt::Display for KeyStroke {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            formatter.write_str("ctrl-")?;
        }
        self.code.fmt(formatter)
    }
}

impl FromStr for KeyStroke {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input.trim().to_ascii_lowercase();
        let (modifiers, key) = if let Some(rest) = trimmed.strip_prefix("ctrl-") {
            (KeyModifiers::CONTROL, rest)
        } else {
            (KeyModifiers::empty(), trimmed.as_str())
        };
        let code = key.parse()?;
        Ok(Self::with_modifiers(code, modifiers))
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum KeyStrokeCode {
    Char(char),
    Down,
    Up,
    Tab,
    BackTab,
    Esc,
    F(u8),
}

impl KeyStrokeCode {
    const fn matches(self, code: KeyCode) -> bool {
        match (self, code) {
            (Self::Char(expected), KeyCode::Char(actual)) => expected == actual,
            (Self::Down, KeyCode::Down) => true,
            (Self::Up, KeyCode::Up) => true,
            (Self::Tab, KeyCode::Tab) => true,
            (Self::BackTab, KeyCode::BackTab) => true,
            (Self::Esc, KeyCode::Esc) => true,
            (Self::F(expected), KeyCode::F(actual)) => expected == actual,
            _ => false,
        }
    }

    fn fmt(self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Char(character) => write!(formatter, "{character}"),
            Self::Down => formatter.write_str("down"),
            Self::Up => formatter.write_str("up"),
            Self::Tab => formatter.write_str("tab"),
            Self::BackTab => formatter.write_str("shift-tab"),
            Self::Esc => formatter.write_str("esc"),
            Self::F(number) => write!(formatter, "f{number}"),
        }
    }
}

impl FromStr for KeyStrokeCode {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "down" => Ok(Self::Down),
            "up" => Ok(Self::Up),
            "tab" => Ok(Self::Tab),
            "shift-tab" | "backtab" => Ok(Self::BackTab),
            "esc" | "escape" => Ok(Self::Esc),
            _ if input
                .strip_prefix('f')
                .and_then(|number| number.parse::<u8>().ok())
                .is_some_and(|number| (1..=12).contains(&number)) =>
            {
                Ok(Self::F(
                    input[1..].parse().expect("validated function key number"),
                ))
            }
            _ if input.chars().count() == 1 => {
                Ok(Self::Char(input.chars().next().expect("one character")))
            }
            _ => Err(format!("unsupported TUI key binding '{}'", input)),
        }
    }
}

fn default_normal_bindings() -> Vec<KeyBinding> {
    [
        ("j", ActionId::SelectSymbolBy(1)),
        ("down", ActionId::SelectSymbolBy(1)),
        ("k", ActionId::SelectSymbolBy(-1)),
        ("up", ActionId::SelectSymbolBy(-1)),
        ("h", ActionId::OpenFloating(FloatingKind::Help)),
        ("f1", ActionId::OpenFloating(FloatingKind::Help)),
        (":", ActionId::OpenFloating(FloatingKind::CommandPalette)),
        ("p", ActionId::OpenFloating(FloatingKind::ProviderDetails)),
        ("esc", ActionId::CloseFocusedFloating),
        ("r", ActionId::ResetLayout),
        ("x", ActionId::CloseFocusedPanel),
        ("0", ActionId::RestorePanels),
        ("tab", ActionId::FocusPanelBy(1)),
        ("shift-tab", ActionId::FocusPanelBy(-1)),
        ("z", ActionId::ToggleFocusedZoom),
        ("]", ActionId::ShiftWorkspace(1)),
        ("[", ActionId::ShiftWorkspace(-1)),
        ("1", ActionId::FocusPanel(Panel::Watchlist)),
        ("2", ActionId::FocusPanel(Panel::Quote)),
        ("3", ActionId::FocusPanel(Panel::History)),
        ("4", ActionId::FocusPanel(Panel::Evidence)),
        ("5", ActionId::FocusPanel(Panel::Polymarket)),
        ("6", ActionId::FocusPanel(Panel::Research)),
    ]
    .into_iter()
    .map(|(key, action)| KeyBinding {
        key: key.parse().expect("default key bindings are valid"),
        action,
    })
    .collect()
}

fn normalize_bindings(bindings: Vec<KeyBinding>) -> Vec<KeyBinding> {
    let mut normalized = Vec::new();
    for binding in bindings {
        if normalized
            .iter()
            .any(|existing: &KeyBinding| existing.key == binding.key)
        {
            continue;
        }
        normalized.push(binding);
    }
    normalized
}

fn merge_normal_bindings(overrides: &[KeyBinding]) -> Vec<KeyBinding> {
    let mut merged = overrides.to_vec();
    for binding in default_normal_bindings() {
        if merged
            .iter()
            .any(|existing: &KeyBinding| existing.key == binding.key)
        {
            continue;
        }
        merged.push(binding);
    }
    merged
}

fn serialize_key_stroke<S>(key: &KeyStroke, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&key.to_string())
}

fn deserialize_key_stroke<'de, D>(deserializer: D) -> Result<KeyStroke, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn serialize_action<S>(action: &ActionId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let id = action_id(*action).ok_or_else(|| {
        serde::ser::Error::custom(format!(
            "action {:?} is not a configurable TUI action",
            action
        ))
    })?;
    serializer.serialize_str(id)
}

fn deserialize_action<'de, D>(deserializer: D) -> Result<ActionId, D::Error>
where
    D: Deserializer<'de>,
{
    let id = String::deserialize(deserializer)?;
    action_by_id(&id)
        .ok_or_else(|| serde::de::Error::custom(format!("unknown TUI action '{}'", id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_keymap_matches_named_keys_and_modifiers() {
        let keymap = KeymapConfig::default();

        assert_eq!(
            keymap.normal_action(KeyEvent::from(KeyCode::Char('j'))),
            Some(ActionId::SelectSymbolBy(1))
        );
        assert_eq!(
            keymap.normal_action(KeyEvent::from(KeyCode::BackTab)),
            Some(ActionId::FocusPanelBy(-1))
        );
        assert_eq!(
            keymap.normal_action(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(ActionId::FocusPanelBy(-1))
        );
        assert_eq!(
            keymap.normal_action(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::SHIFT)),
            Some(ActionId::OpenFloating(FloatingKind::CommandPalette))
        );
        assert_eq!(
            keymap.normal_action(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn key_strokes_roundtrip_single_character_and_function_keys() {
        for key in ["a", "/", "f12", "ctrl-/"] {
            let stroke = key.parse::<KeyStroke>().expect("key");
            assert_eq!(stroke.to_string(), key);
        }
    }

    #[test]
    fn keymap_toml_uses_stable_action_ids() {
        let config = KeymapConfig::from_overrides(vec![KeyBinding {
            key: "ctrl-p".parse().expect("key"),
            action: ActionId::OpenFloating(FloatingKind::ProviderDetails),
        }]);

        let encoded = toml::to_string(&config).expect("encode");
        assert!(encoded.contains("[[overrides]]"));
        assert!(encoded.contains("key = \"ctrl-p\""));
        assert!(encoded.contains("action = \"open-provider-details\""));

        let decoded = toml::from_str::<KeymapConfig>(&encoded).expect("decode");
        assert_eq!(decoded, config);
    }

    #[test]
    fn keymap_rejects_unknown_actions() {
        let error = toml::from_str::<KeymapConfig>(
            r#"
            [[overrides]]
            key = "x"
            action = "does-not-exist"
            "#,
        )
        .expect_err("unknown action should fail");

        assert!(error.to_string().contains("unknown TUI action"));
    }

    #[test]
    fn keymap_keeps_first_binding_for_duplicate_key() {
        let mut config = KeymapConfig::from_overrides(vec![
            KeyBinding {
                key: "x".parse().expect("key"),
                action: ActionId::CloseFocusedPanel,
            },
            KeyBinding {
                key: "x".parse().expect("key"),
                action: ActionId::ResetLayout,
            },
        ]);

        config.normalize();

        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].action, ActionId::CloseFocusedPanel);
        assert_eq!(
            config.normal_action(KeyEvent::from(KeyCode::Char('x'))),
            Some(ActionId::CloseFocusedPanel)
        );
    }

    #[test]
    fn default_keymap_does_not_persist_effective_bindings() {
        let encoded = toml::to_string(&KeymapConfig::default()).expect("encode");

        assert!(!encoded.contains("select-next-symbol"));
        assert!(!encoded.contains("[[overrides]]"));
    }

    #[test]
    fn overrides_replace_default_keys_without_hiding_other_defaults() {
        let config = KeymapConfig::from_overrides(vec![KeyBinding {
            key: ":".parse().expect("key"),
            action: ActionId::OpenFloating(FloatingKind::ProviderDetails),
        }]);

        assert_eq!(
            config.normal_action(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::SHIFT)),
            Some(ActionId::OpenFloating(FloatingKind::ProviderDetails))
        );
        assert_eq!(
            config.normal_action(KeyEvent::from(KeyCode::Char('j'))),
            Some(ActionId::SelectSymbolBy(1))
        );
    }
}
