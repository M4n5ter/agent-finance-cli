use crate::command::ActionId;
use crate::config::ProviderConfig;
use crate::keymap::{KeyStroke, KeymapConfig};
use crate::model::FloatingKind;
use crate::theme::{ThemeColor, ThemeConfig};
use agent_finance_i18n::LocaleId;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct SettingsEditorState {
    selected: usize,
}

impl SettingsEditorState {
    pub fn selected(&self) -> SettingRow {
        SettingRow::ALL
            .get(self.selected)
            .copied()
            .unwrap_or(SettingRow::ALL[0])
    }

    pub fn move_selection(&mut self, direction: isize) {
        self.selected = shift_index(self.selected, SettingRow::ALL.len(), direction);
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index.min(SettingRow::ALL.len().saturating_sub(1));
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SettingRow {
    label: &'static str,
    target: SettingTarget,
}

impl SettingRow {
    pub const ALL: [Self; 12] = [
        Self::locale(),
        Self::provider("equity provider", ProviderSetting::Equity),
        Self::provider("crypto provider", ProviderSetting::Crypto),
        Self::theme("theme accent", ThemeSetting::Accent),
        Self::theme("selection background", ThemeSetting::SelectionBackground),
        Self::theme("selection foreground", ThemeSetting::SelectionForeground),
        Self::keymap(KeymapSetting::CommandPalette),
        Self::keymap(KeymapSetting::SymbolSearch),
        Self::keymap(KeymapSetting::ProviderDetails),
        Self::keymap(KeymapSetting::LiveWrites),
        Self::keymap(KeymapSetting::SaveConfig),
        Self::keymap(KeymapSetting::UndoConfig),
    ];

    const fn locale() -> Self {
        Self {
            label: "language",
            target: SettingTarget::Locale,
        }
    }

    const fn provider(label: &'static str, setting: ProviderSetting) -> Self {
        Self {
            label,
            target: SettingTarget::Provider(setting),
        }
    }

    const fn theme(label: &'static str, setting: ThemeSetting) -> Self {
        Self {
            label,
            target: SettingTarget::Theme(setting),
        }
    }

    const fn keymap(setting: KeymapSetting) -> Self {
        Self {
            label: setting.spec().label,
            target: SettingTarget::Keymap(setting),
        }
    }

    pub const fn label(self) -> &'static str {
        self.label
    }

    pub fn label_key(self) -> &'static str {
        match self.label {
            "language" => "tui-setting-language",
            "equity provider" => "tui-setting-equity-provider",
            "crypto provider" => "tui-setting-crypto-provider",
            "theme accent" => "tui-setting-theme-accent",
            "selection background" => "tui-setting-selection-background",
            "selection foreground" => "tui-setting-selection-foreground",
            "key command palette" => "tui-setting-key-command-palette",
            "key symbol search" => "tui-setting-key-symbol-search",
            "key provider details" => "tui-setting-key-provider-details",
            "key live writes" => "tui-setting-key-live-writes",
            "key save config" => "tui-setting-key-save-config",
            "key undo config" => "tui-setting-key-undo-config",
            _ => "tui-setting-unknown",
        }
    }

    pub fn value(
        self,
        locale: &LocaleId,
        providers: &ProviderConfig,
        theme: &ThemeConfig,
        keymap: &KeymapConfig,
    ) -> String {
        self.target.value(locale, providers, theme, keymap)
    }

    pub fn adjust(
        self,
        locale: &mut LocaleId,
        providers: &mut ProviderConfig,
        theme: &mut ThemeConfig,
        keymap: &mut KeymapConfig,
        direction: isize,
    ) -> Option<SettingChange> {
        self.target
            .adjust(locale, providers, theme, keymap, direction)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SettingChange {
    pub section: &'static str,
    pub requires_provider_reload: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SettingTarget {
    Locale,
    Provider(ProviderSetting),
    Theme(ThemeSetting),
    Keymap(KeymapSetting),
}

impl SettingTarget {
    fn value(
        self,
        locale: &LocaleId,
        providers: &ProviderConfig,
        theme: &ThemeConfig,
        keymap: &KeymapConfig,
    ) -> String {
        match self {
            Self::Locale => locale.display_name().to_string(),
            Self::Provider(setting) => setting.value(providers),
            Self::Theme(setting) => setting.value(theme),
            Self::Keymap(setting) => setting.value(keymap),
        }
    }

    fn adjust(
        self,
        locale: &mut LocaleId,
        providers: &mut ProviderConfig,
        theme: &mut ThemeConfig,
        keymap: &mut KeymapConfig,
        direction: isize,
    ) -> Option<SettingChange> {
        match self {
            Self::Locale => adjust_locale(locale, direction),
            Self::Provider(setting) => setting.adjust(providers, direction),
            Self::Theme(setting) => setting.adjust(theme, direction),
            Self::Keymap(setting) => setting.adjust(keymap, direction),
        }
    }
}

fn adjust_locale(locale: &mut LocaleId, direction: isize) -> Option<SettingChange> {
    let current = LocaleId::ALL
        .iter()
        .position(|candidate| candidate == locale)
        .unwrap_or_default();
    let next = LocaleId::ALL[shift_index(current, LocaleId::ALL.len(), direction)];
    if *locale == next {
        return None;
    }
    *locale = next;
    Some(SettingChange {
        section: "locale",
        requires_provider_reload: false,
    })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ProviderSetting {
    Equity,
    Crypto,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum KeymapSetting {
    CommandPalette,
    SymbolSearch,
    ProviderDetails,
    LiveWrites,
    SaveConfig,
    UndoConfig,
}

impl KeymapSetting {
    fn value(self, keymap: &KeymapConfig) -> String {
        let spec = self.spec();
        let action = spec.action;
        let effective = keymap
            .normal_key_for(action)
            .map(|key| key.to_string())
            .unwrap_or_else(|| "-".to_string());
        if let Some(override_key) = keymap.override_key_for(action) {
            format!("{effective} override={override_key}")
        } else {
            format!("{effective} default")
        }
    }

    fn adjust(self, keymap: &mut KeymapConfig, direction: isize) -> Option<SettingChange> {
        let spec = self.spec();
        let choices = spec.choices;
        let current = keymap
            .override_key_for(spec.action)
            .or_else(|| keymap.normal_key_for(spec.action));
        let next = match current.and_then(|key| choices.iter().position(|choice| choice == &key)) {
            Some(current_index) => choices[shift_index(current_index, choices.len(), direction)],
            None if direction < 0 => choices[choices.len() - 1],
            None => choices[0],
        };
        if Some(next) == current {
            return None;
        }

        if Some(next) == default_key_for(spec.action) {
            keymap.clear_action_override(spec.action);
        } else {
            keymap.set_action_override(spec.action, next);
        }
        Some(SettingChange {
            section: "keymap",
            requires_provider_reload: false,
        })
    }

    const fn spec(self) -> &'static KeymapSettingSpec {
        match self {
            Self::CommandPalette => &KEY_COMMAND_PALETTE,
            Self::SymbolSearch => &KEY_SYMBOL_SEARCH,
            Self::ProviderDetails => &KEY_PROVIDER_DETAILS,
            Self::LiveWrites => &KEY_LIVE_WRITES,
            Self::SaveConfig => &KEY_SAVE_CONFIG,
            Self::UndoConfig => &KEY_UNDO_CONFIG,
        }
    }
}

struct KeymapSettingSpec {
    label: &'static str,
    action: ActionId,
    choices: &'static [KeyStroke],
}

fn default_key_for(action: ActionId) -> Option<KeyStroke> {
    KeymapConfig::default().normal_key_for(action)
}

const KEY_COMMAND_PALETTE_CHOICES: [KeyStroke; 3] = [
    key(':'),
    ctrl_key('p'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(2)),
];
const KEY_SYMBOL_SEARCH_CHOICES: [KeyStroke; 3] = [
    key('/'),
    ctrl_key('f'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(3)),
];
const KEY_PROVIDER_DETAILS_CHOICES: [KeyStroke; 3] = [
    key('p'),
    key('i'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(4)),
];
const KEY_LIVE_WRITES_CHOICES: [KeyStroke; 3] = [
    ctrl_key('l'),
    key('l'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(5)),
];
const KEY_SAVE_CONFIG_CHOICES: [KeyStroke; 3] = [
    ctrl_key('s'),
    key('s'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(6)),
];
const KEY_UNDO_CONFIG_CHOICES: [KeyStroke; 3] = [
    key('u'),
    ctrl_key('z'),
    KeyStroke::new(crate::keymap::KeyStrokeCode::F(7)),
];

const KEY_COMMAND_PALETTE: KeymapSettingSpec = KeymapSettingSpec {
    label: "key command palette",
    action: ActionId::OpenFloating(FloatingKind::CommandPalette),
    choices: &KEY_COMMAND_PALETTE_CHOICES,
};
const KEY_SYMBOL_SEARCH: KeymapSettingSpec = KeymapSettingSpec {
    label: "key symbol search",
    action: ActionId::OpenFloating(FloatingKind::SymbolSearch),
    choices: &KEY_SYMBOL_SEARCH_CHOICES,
};
const KEY_PROVIDER_DETAILS: KeymapSettingSpec = KeymapSettingSpec {
    label: "key provider details",
    action: ActionId::OpenFloating(FloatingKind::ProviderDetails),
    choices: &KEY_PROVIDER_DETAILS_CHOICES,
};
const KEY_LIVE_WRITES: KeymapSettingSpec = KeymapSettingSpec {
    label: "key live writes",
    action: ActionId::ToggleLiveWrites,
    choices: &KEY_LIVE_WRITES_CHOICES,
};
const KEY_SAVE_CONFIG: KeymapSettingSpec = KeymapSettingSpec {
    label: "key save config",
    action: ActionId::SaveConfig,
    choices: &KEY_SAVE_CONFIG_CHOICES,
};
const KEY_UNDO_CONFIG: KeymapSettingSpec = KeymapSettingSpec {
    label: "key undo config",
    action: ActionId::UndoConfigChange,
    choices: &KEY_UNDO_CONFIG_CHOICES,
};

const fn key(character: char) -> KeyStroke {
    KeyStroke::new(crate::keymap::KeyStrokeCode::Char(character))
}

const fn ctrl_key(character: char) -> KeyStroke {
    KeyStroke::with_modifiers(
        crate::keymap::KeyStrokeCode::Char(character),
        crossterm::event::KeyModifiers::CONTROL,
    )
}

impl ProviderSetting {
    fn value(self, providers: &ProviderConfig) -> String {
        match self {
            Self::Equity => providers.equity.to_string(),
            Self::Crypto => providers.crypto.to_string(),
        }
    }

    fn adjust(self, providers: &mut ProviderConfig, direction: isize) -> Option<SettingChange> {
        let changed = match self {
            Self::Equity => providers.adjust_equity(direction),
            Self::Crypto => providers.adjust_crypto(direction),
        };
        changed.then_some(SettingChange {
            section: "providers",
            requires_provider_reload: true,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ThemeSetting {
    Accent,
    SelectionBackground,
    SelectionForeground,
}

impl ThemeSetting {
    fn value(self, theme: &ThemeConfig) -> String {
        self.color(theme).to_string()
    }

    fn adjust(self, theme: &mut ThemeConfig, direction: isize) -> Option<SettingChange> {
        let color = self.color_mut(theme);
        let next = color.shift(direction);
        if *color == next {
            return None;
        }
        *color = next;
        Some(SettingChange {
            section: "theme",
            requires_provider_reload: false,
        })
    }

    const fn color(self, theme: &ThemeConfig) -> ThemeColor {
        match self {
            Self::Accent => theme.accent,
            Self::SelectionBackground => theme.selection_background,
            Self::SelectionForeground => theme.selection_foreground,
        }
    }

    fn color_mut(self, theme: &mut ThemeConfig) -> &mut ThemeColor {
        match self {
            Self::Accent => &mut theme.accent,
            Self::SelectionBackground => &mut theme.selection_background,
            Self::SelectionForeground => &mut theme.selection_foreground,
        }
    }
}

fn shift_index(current: usize, len: usize, direction: isize) -> usize {
    debug_assert!(len > 0);
    (current as isize + direction).rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_rows_wrap_selection() {
        let mut state = SettingsEditorState::default();

        state.move_selection(-1);
        assert_eq!(state.selected().label(), "key undo config");

        state.move_selection(1);
        assert_eq!(state.selected().label(), "language");

        state.move_selection(2);
        assert_eq!(state.selected().label(), "crypto provider");
    }

    #[test]
    fn locale_setting_cycles_supported_locales() {
        let mut locale = LocaleId::EnUs;

        let change = adjust_locale(&mut locale, 1).expect("locale should change");

        assert_eq!(change.section, "locale");
        assert_eq!(locale, LocaleId::ZhCn);
        adjust_locale(&mut locale, -1);
        assert_eq!(locale, LocaleId::EnUs);
    }

    #[test]
    fn keymap_setting_uses_first_choice_when_action_has_no_default_key() {
        let mut keymap = KeymapConfig::default();

        let change = KeymapSetting::SaveConfig
            .adjust(&mut keymap, 1)
            .expect("setting change");

        assert_eq!(change.section, "keymap");
        assert_eq!(
            keymap
                .override_key_for(ActionId::SaveConfig)
                .map(|key| key.to_string()),
            Some("ctrl-s".to_string())
        );
    }

    #[test]
    fn keymap_setting_clears_override_when_cycle_returns_to_default_key() {
        let mut keymap = KeymapConfig::default();

        KeymapSetting::CommandPalette.adjust(&mut keymap, 1);
        assert_eq!(
            keymap
                .override_key_for(ActionId::OpenFloating(FloatingKind::CommandPalette))
                .map(|key| key.to_string()),
            Some("ctrl-p".to_string())
        );

        KeymapSetting::CommandPalette.adjust(&mut keymap, -1);

        assert!(
            keymap
                .override_key_for(ActionId::OpenFloating(FloatingKind::CommandPalette))
                .is_none()
        );
        assert!(keymap.overrides.is_empty());
    }
}
