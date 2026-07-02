use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};

use crate::{LocaleId, resources};

pub type MessageArgs<'a> = &'a [(&'a str, &'a str)];

pub struct Translator {
    locale: LocaleId,
    bundles: BTreeMap<LocaleId, FluentBundle<FluentResource>>,
}

impl Translator {
    pub fn new(locale: LocaleId) -> Result<Self> {
        let bundles = LocaleId::ALL
            .into_iter()
            .map(|locale| build_bundle(locale).map(|bundle| (locale, bundle)))
            .collect::<Result<BTreeMap<_, _>>>()?;

        Ok(Self { locale, bundles })
    }

    pub fn locale(&self) -> LocaleId {
        self.locale
    }

    pub fn set_locale(&mut self, locale: LocaleId) {
        self.locale = locale;
    }

    pub fn text(&self, key: &str) -> String {
        self.text_with_args(key, &[])
    }

    pub fn text_with_args(&self, key: &str, args: MessageArgs<'_>) -> String {
        self.format(self.locale, key, args)
            .or_else(|| {
                (self.locale != LocaleId::DEFAULT)
                    .then(|| self.format(LocaleId::DEFAULT, key, args))
                    .flatten()
            })
            .unwrap_or_else(|| format!("⟦{key}⟧"))
    }

    fn format(&self, locale: LocaleId, key: &str, args: MessageArgs<'_>) -> Option<String> {
        let bundle = self.bundles.get(&locale)?;
        let message = bundle.get_message(key)?;
        let pattern = message.value()?;
        let args = fluent_args(args);
        let mut errors = Vec::new();
        let value = bundle.format_pattern(pattern, Some(&args), &mut errors);

        errors.is_empty().then(|| value.into_owned())
    }
}

fn build_bundle(locale: LocaleId) -> Result<FluentBundle<FluentResource>> {
    build_bundle_from_source(locale, resources::source(locale))
}

fn build_bundle_from_source(
    locale: LocaleId,
    source: &str,
) -> Result<FluentBundle<FluentResource>> {
    let resource = FluentResource::try_new(source.to_owned())
        .map_err(|(_, errors)| anyhow!("invalid Fluent resource for {locale}: {errors:?}"))?;
    let mut bundle = FluentBundle::new(vec![locale.fluent_id()]);
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .map_err(|errors| anyhow!("failed to add Fluent resource for {locale}: {errors:?}"))?;
    Ok(bundle)
}

fn fluent_args<'a>(args: MessageArgs<'a>) -> FluentArgs<'a> {
    let mut fluent_args = FluentArgs::new();
    for (key, value) in args {
        fluent_args.set(*key, FluentValue::from(*value));
    }
    fluent_args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_selected_locale_with_variables() {
        let translator = Translator::new(LocaleId::ZhCn).unwrap();

        assert_eq!(
            translator.text_with_args("test-greeting", &[("name", "M4n5ter")]),
            "你好，M4n5ter。"
        );
    }

    #[test]
    fn falls_back_to_english_for_missing_runtime_key() {
        let translator = Translator {
            locale: LocaleId::ZhCn,
            bundles: BTreeMap::from([
                (
                    LocaleId::EnUs,
                    build_bundle_from_source(
                        LocaleId::EnUs,
                        "test-english-only-runtime-fallback = English fallback for { $name }.",
                    )
                    .unwrap(),
                ),
                (
                    LocaleId::ZhCn,
                    build_bundle_from_source(LocaleId::ZhCn, "test-greeting = 你好，{ $name }。")
                        .unwrap(),
                ),
            ]),
        };

        assert_eq!(
            translator.text_with_args("test-english-only-runtime-fallback", &[("name", "M4n5ter")]),
            "English fallback for M4n5ter."
        );
    }

    #[test]
    fn marks_unknown_keys() {
        let translator = Translator::new(LocaleId::JaJp).unwrap();

        assert_eq!(translator.text("does-not-exist"), "⟦does-not-exist⟧");
    }
}
