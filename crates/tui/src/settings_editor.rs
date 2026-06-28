use crate::config::ProviderConfig;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct SettingsEditorState {
    selected: SettingsRow,
}

impl SettingsEditorState {
    pub const fn selected(&self) -> SettingsRow {
        self.selected
    }

    pub fn move_selection(&mut self, direction: isize) {
        self.selected = self.selected.shift(direction);
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum SettingsRow {
    #[default]
    EquityProvider,
    CryptoProvider,
}

impl SettingsRow {
    pub const ALL: [Self; 2] = [Self::EquityProvider, Self::CryptoProvider];

    pub const fn label(self) -> &'static str {
        match self {
            Self::EquityProvider => "equity provider",
            Self::CryptoProvider => "crypto provider",
        }
    }

    pub fn value(self, providers: &ProviderConfig) -> String {
        match self {
            Self::EquityProvider => providers.equity.to_string(),
            Self::CryptoProvider => providers.crypto.to_string(),
        }
    }

    pub fn adjust(self, providers: &mut ProviderConfig, direction: isize) -> bool {
        match self {
            Self::EquityProvider => providers.adjust_equity(direction),
            Self::CryptoProvider => providers.adjust_crypto(direction),
        }
    }

    fn shift(self, direction: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|row| *row == self)
            .unwrap_or_default() as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_rows_wrap_selection() {
        let mut state = SettingsEditorState::default();

        state.move_selection(-1);
        assert_eq!(state.selected(), SettingsRow::CryptoProvider);

        state.move_selection(1);
        assert_eq!(state.selected(), SettingsRow::EquityProvider);
    }
}
