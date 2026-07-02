use crate::LocaleId;

pub(crate) const EN_US: &str = include_str!("../locales/en-US/agent-finance.ftl");
pub(crate) const ZH_CN: &str = include_str!("../locales/zh-CN/agent-finance.ftl");
pub(crate) const JA_JP: &str = include_str!("../locales/ja-JP/agent-finance.ftl");
pub(crate) const KO_KR: &str = include_str!("../locales/ko-KR/agent-finance.ftl");

pub(crate) fn source(locale: LocaleId) -> &'static str {
    match locale {
        LocaleId::EnUs => EN_US,
        LocaleId::ZhCn => ZH_CN,
        LocaleId::JaJp => JA_JP,
        LocaleId::KoKr => KO_KR,
    }
}

pub(crate) fn sources() -> impl Iterator<Item = (LocaleId, &'static str)> {
    LocaleId::ALL
        .into_iter()
        .map(|locale| (locale, source(locale)))
}
