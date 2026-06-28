use std::fmt;
use std::str::FromStr;

use ratatui::style::{Color, Style};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct ThemeConfig {
    pub accent: ThemeColor,
    pub text: ThemeColor,
    pub neutral: ThemeColor,
    pub muted: ThemeColor,
    pub success: ThemeColor,
    pub warning: ThemeColor,
    pub danger: ThemeColor,
    pub prediction: ThemeColor,
    pub chrome_background: ThemeColor,
    pub chrome_foreground: ThemeColor,
    pub selection_background: ThemeColor,
    pub selection_foreground: ThemeColor,
    pub shadow_foreground: ThemeColor,
    pub shadow_background: ThemeColor,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent: ThemeColor::Cyan,
            text: ThemeColor::White,
            neutral: ThemeColor::Gray,
            muted: ThemeColor::DarkGray,
            success: ThemeColor::Green,
            warning: ThemeColor::Yellow,
            danger: ThemeColor::Red,
            prediction: ThemeColor::Magenta,
            chrome_background: ThemeColor::DarkGray,
            chrome_foreground: ThemeColor::Gray,
            selection_background: ThemeColor::Cyan,
            selection_foreground: ThemeColor::Black,
            shadow_foreground: ThemeColor::Black,
            shadow_background: ThemeColor::DarkGray,
        }
    }
}

impl ThemeConfig {
    pub(crate) fn normalize(&mut self) {}

    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent.color())
    }

    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text.color())
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted.color())
    }

    pub fn neutral_style(&self) -> Style {
        Style::default().fg(self.neutral.color())
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success.color())
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning.color())
    }

    pub fn danger_style(&self) -> Style {
        Style::default().fg(self.danger.color())
    }

    pub fn prediction_style(&self) -> Style {
        Style::default().fg(self.prediction.color())
    }

    pub fn chrome_style(&self) -> Style {
        Style::default()
            .fg(self.chrome_foreground.color())
            .bg(self.chrome_background.color())
    }

    pub fn selected_style(&self) -> Style {
        Style::default()
            .fg(self.selection_foreground.color())
            .bg(self.selection_background.color())
    }

    pub fn shadow_style(&self) -> Style {
        Style::default()
            .fg(self.shadow_foreground.color())
            .bg(self.shadow_background.color())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThemeColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    White,
}

impl ThemeColor {
    pub const ALL: [Self; 10] = [
        Self::Black,
        Self::Red,
        Self::Green,
        Self::Yellow,
        Self::Blue,
        Self::Magenta,
        Self::Cyan,
        Self::Gray,
        Self::DarkGray,
        Self::White,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Cyan => "cyan",
            Self::Gray => "gray",
            Self::DarkGray => "dark-gray",
            Self::White => "white",
        }
    }

    pub const fn labels() -> &'static [&'static str] {
        &[
            "black",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "gray",
            "dark-gray",
            "white",
        ]
    }

    pub fn shift(self, direction: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|color| *color == self)
            .unwrap_or_default() as isize;
        let next = (index + direction).rem_euclid(Self::ALL.len() as isize) as usize;
        Self::ALL[next]
    }

    pub const fn color(self) -> Color {
        match self {
            Self::Black => Color::Black,
            Self::Red => Color::Red,
            Self::Green => Color::Green,
            Self::Yellow => Color::Yellow,
            Self::Blue => Color::Blue,
            Self::Magenta => Color::Magenta,
            Self::Cyan => Color::Cyan,
            Self::Gray => Color::Gray,
            Self::DarkGray => Color::DarkGray,
            Self::White => Color::White,
        }
    }
}

impl fmt::Display for ThemeColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl FromStr for ThemeColor {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "black" => Ok(Self::Black),
            "red" => Ok(Self::Red),
            "green" => Ok(Self::Green),
            "yellow" => Ok(Self::Yellow),
            "blue" => Ok(Self::Blue),
            "magenta" => Ok(Self::Magenta),
            "cyan" => Ok(Self::Cyan),
            "gray" | "grey" => Ok(Self::Gray),
            "dark-gray" | "dark-grey" => Ok(Self::DarkGray),
            "white" => Ok(Self::White),
            _ => Err(format!(
                "invalid TUI theme color '{}'; expected one of: {}",
                input,
                Self::labels().join(", ")
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_color_roundtrips_agent_facing_labels() {
        let encoded = toml::to_string(&ThemeConfig {
            accent: ThemeColor::Blue,
            selection_background: ThemeColor::Magenta,
            ..ThemeConfig::default()
        })
        .expect("encode");

        assert!(encoded.contains("accent = \"blue\""));
        assert!(encoded.contains("selection_background = \"magenta\""));

        let decoded = toml::from_str::<ThemeConfig>(&encoded).expect("decode");
        assert_eq!(decoded.accent, ThemeColor::Blue);
        assert_eq!(decoded.selection_background, ThemeColor::Magenta);
    }

    #[test]
    fn theme_color_accepts_hyphenated_and_alias_labels() {
        assert_eq!("dark-gray".parse::<ThemeColor>(), Ok(ThemeColor::DarkGray));
        assert_eq!("dark_grey".parse::<ThemeColor>(), Ok(ThemeColor::DarkGray));
        assert!("orange".parse::<ThemeColor>().is_err());
    }

    #[test]
    fn theme_color_shift_wraps_palette() {
        assert_eq!(ThemeColor::Black.shift(-1), ThemeColor::White);
        assert_eq!(ThemeColor::White.shift(1), ThemeColor::Black);
        assert_eq!(ThemeColor::Cyan.shift(1), ThemeColor::Gray);
    }

    #[test]
    fn partial_theme_config_uses_default_roles() {
        let theme = toml::from_str::<ThemeConfig>(
            r#"
            accent = "blue"
            "#,
        )
        .expect("decode partial theme");

        assert_eq!(theme.accent, ThemeColor::Blue);
        assert_eq!(theme.neutral, ThemeColor::Gray);
        assert_eq!(theme.warning, ThemeColor::Yellow);
        assert_eq!(theme.shadow_background, ThemeColor::DarkGray);
    }
}
