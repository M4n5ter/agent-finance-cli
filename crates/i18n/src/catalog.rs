use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow};
use fluent_bundle::FluentResource;

use crate::{LocaleId, resources};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSnapshot {
    locale: LocaleId,
    messages: BTreeMap<String, BTreeSet<String>>,
}

impl CatalogSnapshot {
    pub fn from_source(locale: LocaleId, source: &str) -> Result<Self> {
        FluentResource::try_new(source.to_owned())
            .map_err(|(_, errors)| anyhow!("invalid Fluent resource for {locale}: {errors:?}"))?;

        Ok(Self {
            locale,
            messages: extract_messages(source),
        })
    }

    pub fn locale(&self) -> LocaleId {
        self.locale
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(String::as_str)
    }

    pub fn variables_for(&self, key: &str) -> Option<&BTreeSet<String>> {
        self.messages.get(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIssue {
    pub locale: LocaleId,
    pub kind: CatalogIssueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogIssueKind {
    MissingKey {
        key: String,
    },
    OrphanKey {
        key: String,
    },
    VariableMismatch {
        key: String,
        expected: BTreeSet<String>,
        actual: BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogReport {
    pub issues: Vec<CatalogIssue>,
}

impl CatalogReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn ensure_clean(self) -> Result<()> {
        if self.is_clean() {
            return Ok(());
        }

        Err(anyhow!("i18n catalog validation failed: {:?}", self.issues))
    }
}

pub fn validate_builtin_catalogs() -> Result<CatalogReport> {
    let catalogs = resources::sources()
        .map(|(locale, source)| CatalogSnapshot::from_source(locale, source))
        .collect::<Result<Vec<_>>>()?;
    validate_catalogs(&catalogs)
}

pub fn validate_catalogs(catalogs: &[CatalogSnapshot]) -> Result<CatalogReport> {
    let Some(canonical) = catalogs
        .iter()
        .find(|catalog| catalog.locale == LocaleId::DEFAULT)
    else {
        return Err(anyhow!(
            "canonical {} catalog is required",
            LocaleId::DEFAULT
        ));
    };

    let mut issues = Vec::new();
    for catalog in catalogs
        .iter()
        .filter(|catalog| catalog.locale != LocaleId::DEFAULT)
    {
        collect_missing_keys(canonical, catalog, &mut issues);
        collect_orphan_keys(canonical, catalog, &mut issues);
        collect_variable_mismatches(canonical, catalog, &mut issues);
    }

    Ok(CatalogReport { issues })
}

fn collect_missing_keys(
    canonical: &CatalogSnapshot,
    catalog: &CatalogSnapshot,
    issues: &mut Vec<CatalogIssue>,
) {
    for key in canonical.keys() {
        if catalog.variables_for(key).is_none() {
            issues.push(CatalogIssue {
                locale: catalog.locale,
                kind: CatalogIssueKind::MissingKey {
                    key: key.to_owned(),
                },
            });
        }
    }
}

fn collect_orphan_keys(
    canonical: &CatalogSnapshot,
    catalog: &CatalogSnapshot,
    issues: &mut Vec<CatalogIssue>,
) {
    for key in catalog.keys() {
        if canonical.variables_for(key).is_none() {
            issues.push(CatalogIssue {
                locale: catalog.locale,
                kind: CatalogIssueKind::OrphanKey {
                    key: key.to_owned(),
                },
            });
        }
    }
}

fn collect_variable_mismatches(
    canonical: &CatalogSnapshot,
    catalog: &CatalogSnapshot,
    issues: &mut Vec<CatalogIssue>,
) {
    for key in canonical.keys() {
        let Some(expected) = canonical.variables_for(key) else {
            continue;
        };
        let Some(actual) = catalog.variables_for(key) else {
            continue;
        };
        if expected != actual {
            issues.push(CatalogIssue {
                locale: catalog.locale,
                kind: CatalogIssueKind::VariableMismatch {
                    key: key.to_owned(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                },
            });
        }
    }
}

fn extract_messages(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut messages = BTreeMap::new();
    let mut current_key = None;
    let mut current_body = String::new();

    for line in source.lines() {
        if let Some(key) = message_key(line)
            && let Some(key) = current_key.replace(key)
        {
            messages.insert(key, extract_variables(&current_body));
            current_body.clear();
        }

        if current_key.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if let Some(key) = current_key {
        messages.insert(key, extract_variables(&current_body));
    }

    messages
}

fn message_key(line: &str) -> Option<String> {
    if line.starts_with(char::is_whitespace) || line.trim_start().starts_with('#') {
        return None;
    }

    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    is_message_identifier(key).then(|| key.to_owned())
}

fn is_message_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn extract_variables(source: &str) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    let mut rest = source;

    while let Some(start) = rest.find("{ $") {
        let after_marker = &rest[start + 3..];
        let name: String = after_marker
            .chars()
            .skip_while(|ch| ch.is_whitespace())
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
            .collect();
        if !name.is_empty() {
            variables.insert(name);
        }
        rest = after_marker;
    }

    variables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalogs_are_complete() {
        validate_builtin_catalogs().unwrap().ensure_clean().unwrap();
    }

    #[test]
    fn detects_missing_orphan_and_variable_mismatch() {
        let canonical = CatalogSnapshot::from_source(
            LocaleId::EnUs,
            r#"
hello = Hello, { $name }.
plain = Plain text.
"#,
        )
        .unwrap();
        let translated = CatalogSnapshot::from_source(
            LocaleId::ZhCn,
            r#"
hello = 你好，{ $user }。
extra = Extra.
"#,
        )
        .unwrap();

        let report = validate_catalogs(&[canonical, translated]).unwrap();

        assert!(report.issues.contains(&CatalogIssue {
            locale: LocaleId::ZhCn,
            kind: CatalogIssueKind::MissingKey {
                key: "plain".to_owned()
            }
        }));
        assert!(report.issues.contains(&CatalogIssue {
            locale: LocaleId::ZhCn,
            kind: CatalogIssueKind::OrphanKey {
                key: "extra".to_owned()
            }
        }));
        assert!(report.issues.iter().any(|issue| matches!(
            &issue.kind,
            CatalogIssueKind::VariableMismatch { key, expected, actual }
                if key == "hello"
                    && expected == &BTreeSet::from(["name".to_owned()])
                    && actual == &BTreeSet::from(["user".to_owned()])
        )));
    }
}
