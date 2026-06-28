use crate::config::ProviderConfig;
use crate::theme::{ThemeColor, ThemeConfig};

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
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SettingRow {
    label: &'static str,
    target: SettingTarget,
}

impl SettingRow {
    pub const ALL: [Self; 5] = [
        Self::provider("equity provider", ProviderSetting::Equity),
        Self::provider("crypto provider", ProviderSetting::Crypto),
        Self::theme("theme accent", ThemeSetting::Accent),
        Self::theme("selection background", ThemeSetting::SelectionBackground),
        Self::theme("selection foreground", ThemeSetting::SelectionForeground),
    ];

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

    pub const fn label(self) -> &'static str {
        self.label
    }

    pub fn value(self, providers: &ProviderConfig, theme: &ThemeConfig) -> String {
        self.target.value(providers, theme)
    }

    pub fn adjust(
        self,
        providers: &mut ProviderConfig,
        theme: &mut ThemeConfig,
        direction: isize,
    ) -> Option<SettingChange> {
        self.target.adjust(providers, theme, direction)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SettingChange {
    pub section: &'static str,
    pub requires_provider_reload: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SettingTarget {
    Provider(ProviderSetting),
    Theme(ThemeSetting),
}

impl SettingTarget {
    fn value(self, providers: &ProviderConfig, theme: &ThemeConfig) -> String {
        match self {
            Self::Provider(setting) => setting.value(providers),
            Self::Theme(setting) => setting.value(theme),
        }
    }

    fn adjust(
        self,
        providers: &mut ProviderConfig,
        theme: &mut ThemeConfig,
        direction: isize,
    ) -> Option<SettingChange> {
        match self {
            Self::Provider(setting) => setting.adjust(providers, direction),
            Self::Theme(setting) => setting.adjust(theme, direction),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ProviderSetting {
    Equity,
    Crypto,
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
        assert_eq!(state.selected().label(), "selection foreground");

        state.move_selection(1);
        assert_eq!(state.selected().label(), "equity provider");

        state.move_selection(2);
        assert_eq!(state.selected().label(), "theme accent");
    }
}
