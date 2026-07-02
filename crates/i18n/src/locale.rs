use std::{env, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LocaleId {
    EnUs,
    ZhCn,
    JaJp,
    KoKr,
}

impl LocaleId {
    pub const DEFAULT: Self = Self::EnUs;
    pub const ALL: [Self; 4] = [Self::EnUs, Self::ZhCn, Self::JaJp, Self::KoKr];

    pub fn parse(input: &str) -> Option<Self> {
        let normalized = normalize_locale(input);
        match normalized.as_str() {
            "en" | "en-us" => Some(Self::EnUs),
            "zh" | "zh-cn" | "zh-hans" | "zh-hans-cn" => Some(Self::ZhCn),
            "ja" | "ja-jp" => Some(Self::JaJp),
            "ko" | "ko-kr" => Some(Self::KoKr),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
            Self::JaJp => "ja-JP",
            Self::KoKr => "ko-KR",
        }
    }

    pub fn language_alias(self) -> &'static str {
        match self {
            Self::EnUs => "en",
            Self::ZhCn => "zh",
            Self::JaJp => "ja",
            Self::KoKr => "ko",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::EnUs => "English",
            Self::ZhCn => "简体中文",
            Self::JaJp => "日本語",
            Self::KoKr => "한국어",
        }
    }

    pub fn fluent_id(self) -> unic_langid::LanguageIdentifier {
        self.as_str()
            .parse()
            .expect("supported locale identifiers must be valid BCP-47 tags")
    }
}

impl fmt::Display for LocaleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleSource {
    Cli,
    Config,
    Env,
    System,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedLocale {
    pub source: LocaleSource,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleResolution {
    pub locale: LocaleId,
    pub source: LocaleSource,
    pub rejected: Vec<RejectedLocale>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocaleSources<'a> {
    pub cli: Option<&'a str>,
    pub config: Option<&'a str>,
    pub env: Option<&'a str>,
    pub system: Option<&'a str>,
}

impl LocaleSources<'static> {
    pub fn from_environment(cli: Option<&str>, config: Option<&str>) -> LocaleResolution {
        let env_locale = env::var("AGENT_FINANCE_LOCALE").ok();
        let system_locale = sys_locale::get_locale();

        resolve_candidates([
            (LocaleSource::Cli, cli),
            (LocaleSource::Config, config),
            (LocaleSource::Env, env_locale.as_deref()),
            (LocaleSource::System, system_locale.as_deref()),
        ])
    }
}

impl<'a> LocaleSources<'a> {
    pub fn resolve(self) -> LocaleResolution {
        resolve_candidates([
            (LocaleSource::Cli, self.cli),
            (LocaleSource::Config, self.config),
            (LocaleSource::Env, self.env),
            (LocaleSource::System, self.system),
        ])
    }
}

fn resolve_candidates<'a>(
    candidates: impl IntoIterator<Item = (LocaleSource, Option<&'a str>)>,
) -> LocaleResolution {
    let mut rejected = Vec::new();

    for (source, candidate) in candidates {
        let Some(value) = candidate.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };

        if let Some(locale) = LocaleId::parse(value) {
            return LocaleResolution {
                locale,
                source,
                rejected,
            };
        }

        rejected.push(RejectedLocale {
            source,
            value: value.to_owned(),
        });
    }

    LocaleResolution {
        locale: LocaleId::DEFAULT,
        source: LocaleSource::Default,
        rejected,
    }
}

fn normalize_locale(input: &str) -> String {
    input
        .trim()
        .replace('_', "-")
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_locales_and_aliases() {
        assert_eq!(LocaleId::parse("en"), Some(LocaleId::EnUs));
        assert_eq!(LocaleId::parse("en_US.UTF-8"), Some(LocaleId::EnUs));
        assert_eq!(LocaleId::parse("zh"), Some(LocaleId::ZhCn));
        assert_eq!(LocaleId::parse("zh-Hans-CN"), Some(LocaleId::ZhCn));
        assert_eq!(LocaleId::parse("ja_JP"), Some(LocaleId::JaJp));
        assert_eq!(LocaleId::parse("ko-KR"), Some(LocaleId::KoKr));
        assert_eq!(LocaleId::parse("fr-FR"), None);
    }

    #[test]
    fn resolves_locale_by_precedence() {
        let resolution = LocaleSources {
            cli: Some("ja"),
            config: Some("zh"),
            env: Some("ko"),
            system: Some("en"),
        }
        .resolve();

        assert_eq!(resolution.locale, LocaleId::JaJp);
        assert_eq!(resolution.source, LocaleSource::Cli);
        assert!(resolution.rejected.is_empty());
    }

    #[test]
    fn records_invalid_candidates_and_falls_through() {
        let resolution = LocaleSources {
            cli: Some("fr"),
            config: Some("zh"),
            env: Some("ko"),
            system: Some("en"),
        }
        .resolve();

        assert_eq!(resolution.locale, LocaleId::ZhCn);
        assert_eq!(resolution.source, LocaleSource::Config);
        assert_eq!(
            resolution.rejected,
            vec![RejectedLocale {
                source: LocaleSource::Cli,
                value: "fr".to_owned()
            }]
        );
    }
}
