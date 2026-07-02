use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use agent_finance_i18n::LocaleId;

const SUPPLEMENTARY_DIRS: &[&str] = &["references", "templates"];

pub fn print_list(locale: LocaleId) -> Result<()> {
    let store = SkillStore::load(locale)?;
    let skills = store.visible_skills();
    let width = skills
        .iter()
        .map(|skill| skill.name.len())
        .max()
        .unwrap_or(0)
        .max(20);

    for skill in skills {
        println!("{:<width$} {}", skill.name, skill.description);
    }

    Ok(())
}

pub fn get(name: &str, full: bool, locale: LocaleId) -> Result<Option<String>> {
    SkillStore::load(locale)?.render(name, full)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillInfo {
    name: String,
    description: String,
    hidden: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillDocument {
    info: SkillInfo,
    content: String,
    supplementary: Vec<SupplementaryFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SupplementaryFile {
    path: String,
    content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Frontmatter {
    name: String,
    description: String,
    hidden: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SkillStore {
    Filesystem(Vec<SkillDocument>),
    Embedded(Vec<SkillDocument>),
}

impl SkillStore {
    fn load(locale: LocaleId) -> Result<Self> {
        if let Some(skill_data_dir) = locate_skill_data_dir()? {
            return load_filesystem_store(&skill_data_dir, locale).map(Self::Filesystem);
        }
        Ok(Self::Embedded(load_embedded_store(locale)?))
    }

    fn visible_skills(&self) -> Vec<&SkillInfo> {
        self.documents()
            .iter()
            .map(|document| &document.info)
            .filter(|skill| !skill.hidden)
            .collect()
    }

    fn render(&self, name: &str, full: bool) -> Result<Option<String>> {
        let Some(document) = self
            .documents()
            .iter()
            .find(|document| document.info.name == name)
        else {
            return Ok(None);
        };

        let mut output = document.content.clone();
        if full {
            append_supplementary(&mut output, &document.supplementary);
        }
        Ok(Some(output))
    }

    fn documents(&self) -> &[SkillDocument] {
        match self {
            Self::Filesystem(documents) | Self::Embedded(documents) => documents,
        }
    }
}

fn locate_skill_data_dir() -> Result<Option<PathBuf>> {
    if let Some(path) = env_dir("AGENT_FINANCE_SKILL_DATA_DIR")? {
        return Ok(Some(path));
    }

    if let Some(root) = env_dir("AGENT_FINANCE_PACKAGE_ROOT")? {
        let skill_data = root.join("skill-data");
        if skill_data.is_dir() {
            return Ok(Some(skill_data));
        }
        return Err(anyhow!(
            "AGENT_FINANCE_PACKAGE_ROOT does not contain skill-data: {}",
            root.display()
        ));
    }

    if let Ok(exe) = env::current_exe() {
        let exe = exe.canonicalize().unwrap_or(exe);
        if let Some(parent) = exe.parent()
            && let Some(found) = find_ancestor_skill_data(parent)
        {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

fn env_dir(name: &str) -> Result<Option<PathBuf>> {
    let Ok(value) = env::var(name) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if path.is_dir() {
        return Ok(Some(path));
    }
    Err(anyhow!(
        "{name} does not point to a directory: {}",
        path.display()
    ))
}

fn find_ancestor_skill_data(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|ancestor| ancestor.join("skill-data"))
        .find(|candidate| candidate.is_dir())
}

fn load_filesystem_store(skill_data_dir: &Path, locale: LocaleId) -> Result<Vec<SkillDocument>> {
    let mut documents = Vec::new();
    let entries = fs::read_dir(skill_data_dir)
        .with_context(|| format!("failed to read {}", skill_data_dir.display()))?;

    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", skill_data_dir.display()))?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }

        let skill_md = localized_skill_file(&dir, locale);
        if !skill_md.is_file() {
            continue;
        }

        let content = fs::read_to_string(&skill_md)
            .with_context(|| format!("failed to read skill {}", skill_md.display()))?;
        let document =
            document_from_content(content, collect_supplementary_files(&dir, locale)?)
                .with_context(|| format!("invalid skill frontmatter in {}", skill_md.display()))?;
        documents.push(document);
    }

    documents.sort_by(|left, right| left.info.name.cmp(&right.info.name));
    Ok(documents)
}

fn load_embedded_store(locale: LocaleId) -> Result<Vec<SkillDocument>> {
    let mut documents = vec![
        embedded_document(
            "core",
            localized_resource(locale, CORE, CORE_ZH_CN, CORE_JA_JP, CORE_KO_KR),
            &[(
                "references/command-map.md",
                localized_resource(
                    locale,
                    CORE_COMMAND_MAP,
                    CORE_COMMAND_MAP_ZH_CN,
                    CORE_COMMAND_MAP_JA_JP,
                    CORE_COMMAND_MAP_KO_KR,
                ),
            )],
        )?,
        embedded_document(
            "crypto",
            localized_resource(locale, CRYPTO, CRYPTO_ZH_CN, CRYPTO_JA_JP, CRYPTO_KO_KR),
            &[],
        )?,
        embedded_document(
            "history-indicators",
            localized_resource(
                locale,
                HISTORY_INDICATORS,
                HISTORY_INDICATORS_ZH_CN,
                HISTORY_INDICATORS_JA_JP,
                HISTORY_INDICATORS_KO_KR,
            ),
            &[],
        )?,
        embedded_document(
            "prediction-markets",
            localized_resource(
                locale,
                PREDICTION_MARKETS,
                PREDICTION_MARKETS_ZH_CN,
                PREDICTION_MARKETS_JA_JP,
                PREDICTION_MARKETS_KO_KR,
            ),
            &[],
        )?,
        embedded_document(
            "price",
            localized_resource(locale, PRICE, PRICE_ZH_CN, PRICE_JA_JP, PRICE_KO_KR),
            &[],
        )?,
        embedded_document(
            "profile",
            localized_resource(locale, PROFILE, PROFILE_ZH_CN, PROFILE_JA_JP, PROFILE_KO_KR),
            &[],
        )?,
        embedded_document(
            "providers",
            localized_resource(
                locale,
                PROVIDERS,
                PROVIDERS_ZH_CN,
                PROVIDERS_JA_JP,
                PROVIDERS_KO_KR,
            ),
            &[],
        )?,
        embedded_document(
            "research-data",
            localized_resource(
                locale,
                RESEARCH_DATA,
                RESEARCH_DATA_ZH_CN,
                RESEARCH_DATA_JA_JP,
                RESEARCH_DATA_KO_KR,
            ),
            &[],
        )?,
        embedded_document(
            "tui",
            localized_resource(locale, TUI, TUI_ZH_CN, TUI_JA_JP, TUI_KO_KR),
            &[],
        )?,
    ];
    documents.sort_by(|left, right| left.info.name.cmp(&right.info.name));
    Ok(documents)
}

fn localized_skill_file(skill_dir: &Path, locale: LocaleId) -> PathBuf {
    if locale == LocaleId::EnUs {
        return skill_dir.join("SKILL.md");
    }
    let localized = skill_dir
        .join("locales")
        .join(locale.as_str())
        .join("SKILL.md");
    if localized.is_file() {
        localized
    } else {
        skill_dir.join("SKILL.md")
    }
}

fn localized_resource(
    locale: LocaleId,
    en_us: &'static str,
    zh_cn: &'static str,
    ja_jp: &'static str,
    ko_kr: &'static str,
) -> &'static str {
    match locale {
        LocaleId::EnUs => en_us,
        LocaleId::ZhCn => zh_cn,
        LocaleId::JaJp => ja_jp,
        LocaleId::KoKr => ko_kr,
    }
}

fn embedded_document(
    _name: &str,
    content: &'static str,
    supplementary: &[(&'static str, &'static str)],
) -> Result<SkillDocument> {
    document_from_content(
        content.to_string(),
        supplementary
            .iter()
            .map(|(path, content)| SupplementaryFile {
                path: (*path).to_string(),
                content: (*content).to_string(),
            })
            .collect(),
    )
}

fn document_from_content(
    content: String,
    supplementary: Vec<SupplementaryFile>,
) -> Result<SkillDocument> {
    let frontmatter = parse_frontmatter(&content)?;
    Ok(SkillDocument {
        info: SkillInfo {
            name: frontmatter.name,
            description: frontmatter.description,
            hidden: frontmatter.hidden,
        },
        content,
        supplementary,
    })
}

fn parse_frontmatter(content: &str) -> Result<Frontmatter> {
    let normalized = content.trim_start().replace("\r\n", "\n");
    let content = normalized.as_str();
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err(anyhow!("missing YAML frontmatter"));
    };
    let Some((frontmatter, _body)) = rest.split_once("\n---") else {
        return Err(anyhow!("unterminated YAML frontmatter"));
    };

    let mut name = None;
    let mut description = None;
    let mut hidden = false;
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("description:") {
            let mut value = value.trim().to_string();
            while index + 1 < lines.len()
                && (lines[index + 1].starts_with("  ") || lines[index + 1].starts_with('\t'))
            {
                index += 1;
                value.push(' ');
                value.push_str(lines[index].trim());
            }
            description = Some(value);
        } else if let Some(value) = line.strip_prefix("hidden:") {
            hidden = matches!(value.trim(), "true" | "yes");
        }
        index += 1;
    }

    let name = name
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing name"))?;
    let description = description
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing description"))?;

    Ok(Frontmatter {
        name,
        description,
        hidden,
    })
}

fn collect_supplementary_files(
    skill_dir: &Path,
    locale: LocaleId,
) -> Result<Vec<SupplementaryFile>> {
    let mut files = Vec::new();

    for subdir_name in SUPPLEMENTARY_DIRS {
        let subdir = skill_dir.join(subdir_name);
        if !subdir.is_dir() {
            continue;
        }

        let mut entries = fs::read_dir(&subdir)
            .with_context(|| format!("failed to read {}", subdir.display()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("failed to read entry in {}", subdir.display()))?;
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let relative = format!(
                "{subdir_name}/{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            let localized = localized_supplementary_file(skill_dir, &relative, locale);
            let content = fs::read_to_string(&localized)
                .with_context(|| format!("failed to read {}", localized.display()))?;
            files.push(SupplementaryFile {
                path: relative,
                content,
            });
        }
    }

    Ok(files)
}

fn localized_supplementary_file(skill_dir: &Path, relative: &str, locale: LocaleId) -> PathBuf {
    if locale == LocaleId::EnUs {
        return skill_dir.join(relative);
    }
    let localized = skill_dir
        .join("locales")
        .join(locale.as_str())
        .join(relative);
    if localized.is_file() {
        localized
    } else {
        skill_dir.join(relative)
    }
}

fn append_supplementary(output: &mut String, supplementary: &[SupplementaryFile]) {
    for file in supplementary {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&format!("\n--- {} ---\n\n", file.path));
        output.push_str(&file.content);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
}

const CORE: &str = include_str!("../../../skill-data/core/SKILL.md");
const CORE_COMMAND_MAP: &str = include_str!("../../../skill-data/core/references/command-map.md");
const CORE_ZH_CN: &str = include_str!("../../../skill-data/core/locales/zh-CN/SKILL.md");
const CORE_JA_JP: &str = include_str!("../../../skill-data/core/locales/ja-JP/SKILL.md");
const CORE_KO_KR: &str = include_str!("../../../skill-data/core/locales/ko-KR/SKILL.md");
const CORE_COMMAND_MAP_ZH_CN: &str =
    include_str!("../../../skill-data/core/locales/zh-CN/references/command-map.md");
const CORE_COMMAND_MAP_JA_JP: &str =
    include_str!("../../../skill-data/core/locales/ja-JP/references/command-map.md");
const CORE_COMMAND_MAP_KO_KR: &str =
    include_str!("../../../skill-data/core/locales/ko-KR/references/command-map.md");
const CRYPTO: &str = include_str!("../../../skill-data/crypto/SKILL.md");
const CRYPTO_ZH_CN: &str = include_str!("../../../skill-data/crypto/locales/zh-CN/SKILL.md");
const CRYPTO_JA_JP: &str = include_str!("../../../skill-data/crypto/locales/ja-JP/SKILL.md");
const CRYPTO_KO_KR: &str = include_str!("../../../skill-data/crypto/locales/ko-KR/SKILL.md");
const HISTORY_INDICATORS: &str = include_str!("../../../skill-data/history-indicators/SKILL.md");
const HISTORY_INDICATORS_ZH_CN: &str =
    include_str!("../../../skill-data/history-indicators/locales/zh-CN/SKILL.md");
const HISTORY_INDICATORS_JA_JP: &str =
    include_str!("../../../skill-data/history-indicators/locales/ja-JP/SKILL.md");
const HISTORY_INDICATORS_KO_KR: &str =
    include_str!("../../../skill-data/history-indicators/locales/ko-KR/SKILL.md");
const PREDICTION_MARKETS: &str = include_str!("../../../skill-data/prediction-markets/SKILL.md");
const PREDICTION_MARKETS_ZH_CN: &str =
    include_str!("../../../skill-data/prediction-markets/locales/zh-CN/SKILL.md");
const PREDICTION_MARKETS_JA_JP: &str =
    include_str!("../../../skill-data/prediction-markets/locales/ja-JP/SKILL.md");
const PREDICTION_MARKETS_KO_KR: &str =
    include_str!("../../../skill-data/prediction-markets/locales/ko-KR/SKILL.md");
const PRICE: &str = include_str!("../../../skill-data/price/SKILL.md");
const PRICE_ZH_CN: &str = include_str!("../../../skill-data/price/locales/zh-CN/SKILL.md");
const PRICE_JA_JP: &str = include_str!("../../../skill-data/price/locales/ja-JP/SKILL.md");
const PRICE_KO_KR: &str = include_str!("../../../skill-data/price/locales/ko-KR/SKILL.md");
const PROFILE: &str = include_str!("../../../skill-data/profile/SKILL.md");
const PROFILE_ZH_CN: &str = include_str!("../../../skill-data/profile/locales/zh-CN/SKILL.md");
const PROFILE_JA_JP: &str = include_str!("../../../skill-data/profile/locales/ja-JP/SKILL.md");
const PROFILE_KO_KR: &str = include_str!("../../../skill-data/profile/locales/ko-KR/SKILL.md");
const PROVIDERS: &str = include_str!("../../../skill-data/providers/SKILL.md");
const PROVIDERS_ZH_CN: &str = include_str!("../../../skill-data/providers/locales/zh-CN/SKILL.md");
const PROVIDERS_JA_JP: &str = include_str!("../../../skill-data/providers/locales/ja-JP/SKILL.md");
const PROVIDERS_KO_KR: &str = include_str!("../../../skill-data/providers/locales/ko-KR/SKILL.md");
const RESEARCH_DATA: &str = include_str!("../../../skill-data/research-data/SKILL.md");
const RESEARCH_DATA_ZH_CN: &str =
    include_str!("../../../skill-data/research-data/locales/zh-CN/SKILL.md");
const RESEARCH_DATA_JA_JP: &str =
    include_str!("../../../skill-data/research-data/locales/ja-JP/SKILL.md");
const RESEARCH_DATA_KO_KR: &str =
    include_str!("../../../skill-data/research-data/locales/ko-KR/SKILL.md");
const TUI: &str = include_str!("../../../skill-data/tui/SKILL.md");
const TUI_ZH_CN: &str = include_str!("../../../skill-data/tui/locales/zh-CN/SKILL.md");
const TUI_JA_JP: &str = include_str!("../../../skill-data/tui/locales/ja-JP/SKILL.md");
const TUI_KO_KR: &str = include_str!("../../../skill-data/tui/locales/ko-KR/SKILL.md");

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_frontmatter_reads_required_fields_and_hidden_flag() {
        let content = "---\nname: agent-finance\ndescription: First line\n  second line\nhidden: true\n---\n\n# Body\n";
        let frontmatter = parse_frontmatter(content).expect("frontmatter");

        assert_eq!(frontmatter.name, "agent-finance");
        assert_eq!(frontmatter.description, "First line second line");
        assert!(frontmatter.hidden);
    }

    #[test]
    fn parse_frontmatter_accepts_windows_line_endings() {
        let content = "---\r\nname: core\r\ndescription: Core guide.\r\n---\r\n\r\n# Body\r\n";
        let frontmatter = parse_frontmatter(content).expect("frontmatter");

        assert_eq!(frontmatter.name, "core");
        assert_eq!(frontmatter.description, "Core guide.");
    }

    #[test]
    fn filesystem_store_reads_skill_data_directories() {
        let root = temp_test_dir("filesystem");
        let skill_data = root.join("skill-data");
        let core = skill_data.join("core");
        let price = skill_data.join("price");
        fs::create_dir_all(&core).expect("core dir");
        fs::create_dir_all(&price).expect("price dir");
        fs::write(
            core.join("SKILL.md"),
            "---\nname: core\ndescription: Core guide.\n---\n\n# Core\n",
        )
        .expect("core skill");
        fs::write(
            price.join("SKILL.md"),
            "---\nname: price\ndescription: Price guide.\n---\n\n# Price\n",
        )
        .expect("price skill");

        let store = SkillStore::Filesystem(
            load_filesystem_store(&skill_data, LocaleId::EnUs).expect("skills"),
        );

        assert_eq!(
            store
                .visible_skills()
                .iter()
                .map(|skill| skill.name.as_str())
                .collect::<Vec<_>>(),
            vec!["core", "price"]
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn embedded_store_keeps_standalone_binary_skills_available() {
        let store =
            SkillStore::Embedded(load_embedded_store(LocaleId::EnUs).expect("embedded store"));

        assert!(
            store
                .visible_skills()
                .iter()
                .any(|skill| skill.name == "core")
        );
        let core = store
            .render("core", true)
            .expect("render")
            .expect("core skill");
        assert!(core.contains("# agent-finance core skill"));
        assert!(core.contains("--- references/command-map.md ---"));
    }

    #[test]
    fn embedded_store_uses_requested_locale_for_skill_documents() {
        let store =
            SkillStore::Embedded(load_embedded_store(LocaleId::ZhCn).expect("embedded store"));

        let skills = store.visible_skills();
        let core = skills
            .iter()
            .find(|skill| skill.name == "core")
            .expect("core skill");
        assert!(
            core.description.contains("入口指南"),
            "localized frontmatter should drive skills list: {core:?}"
        );

        let content = store
            .render("core", true)
            .expect("render")
            .expect("core skill");
        assert!(
            content.contains("本地化说明"),
            "localized body should be returned: {content}"
        );
        assert!(
            content.contains("--- references/command-map.md ---")
                && content.contains("代码块")
                && content.contains("agent-finance market price CRDO"),
            "localized supplementary files should be appended: {content}"
        );
    }

    #[test]
    fn filesystem_store_prefers_locale_document_and_falls_back_to_english() {
        let root = temp_test_dir("localized-filesystem");
        let skill_data = root.join("skill-data");
        let core = skill_data.join("core");
        let price = skill_data.join("price");
        fs::create_dir_all(core.join("locales/zh-CN")).expect("core locale dir");
        fs::create_dir_all(core.join("references")).expect("core references dir");
        fs::create_dir_all(core.join("locales/zh-CN/references"))
            .expect("core locale references dir");
        fs::create_dir_all(&price).expect("price dir");
        fs::write(
            core.join("SKILL.md"),
            "---\nname: core\ndescription: Core guide.\n---\n\n# Core\n",
        )
        .expect("core skill");
        fs::write(
            core.join("locales/zh-CN/SKILL.md"),
            "---\nname: core\ndescription: 核心指南。\n---\n\n# Core\n\n本地化正文\n",
        )
        .expect("localized core skill");
        fs::write(core.join("references/commands.md"), "## Command Map\n").expect("core reference");
        fs::write(
            core.join("locales/zh-CN/references/commands.md"),
            "## 命令地图\n",
        )
        .expect("localized core reference");
        fs::write(
            price.join("SKILL.md"),
            "---\nname: price\ndescription: Price guide.\n---\n\n# Price\n",
        )
        .expect("price skill");

        let store = SkillStore::Filesystem(
            load_filesystem_store(&skill_data, LocaleId::ZhCn).expect("skills"),
        );

        let core = store
            .render("core", false)
            .expect("render")
            .expect("core skill");
        let price = store
            .render("price", false)
            .expect("render")
            .expect("price skill");
        assert!(core.contains("本地化正文"));
        assert!(price.contains("Price guide."));
        let full_core = store
            .render("core", true)
            .expect("render")
            .expect("core skill");
        assert!(full_core.contains("## 命令地图"));
        assert!(!full_core.contains("## Command Map"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn localized_embedded_skills_preserve_structure_and_command_blocks() {
        let en = load_embedded_store(LocaleId::EnUs).expect("en store");
        for locale in [LocaleId::ZhCn, LocaleId::JaJp, LocaleId::KoKr] {
            let localized = load_embedded_store(locale).expect("localized store");
            assert_eq!(localized.len(), en.len());
            for canonical in &en {
                let translated = localized
                    .iter()
                    .find(|document| document.info.name == canonical.info.name)
                    .expect("translated document");
                assert_eq!(translated.info.name, canonical.info.name);
                assert_eq!(
                    heading_levels(&translated.content),
                    heading_levels(&canonical.content),
                    "heading structure drifted for {} {locale}",
                    canonical.info.name,
                );
                assert_eq!(
                    fenced_code_blocks(&translated.content),
                    fenced_code_blocks(&canonical.content),
                    "command/code blocks drifted for {} {locale}",
                    canonical.info.name,
                );
                for canonical_file in &canonical.supplementary {
                    let translated_file = translated
                        .supplementary
                        .iter()
                        .find(|file| file.path == canonical_file.path)
                        .expect("translated supplementary file");
                    assert_eq!(
                        fenced_code_blocks(&translated_file.content),
                        fenced_code_blocks(&canonical_file.content),
                        "supplementary code blocks drifted for {} {} {locale}",
                        canonical.info.name,
                        canonical_file.path,
                    );
                }
            }
        }
    }

    #[test]
    fn supplementary_files_append_references_before_templates() {
        let root = temp_test_dir("supplementary");
        let skill = root.join("core");
        fs::create_dir_all(skill.join("references")).expect("references dir");
        fs::create_dir_all(skill.join("templates")).expect("templates dir");
        fs::write(skill.join("references/commands.md"), "commands\n").expect("commands");
        fs::write(skill.join("templates/example.sh"), "example\n").expect("template");

        let files = collect_supplementary_files(&skill, LocaleId::EnUs).expect("files");

        assert_eq!(
            files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["references/commands.md", "templates/example.sh"]
        );

        fs::remove_dir_all(root).ok();
    }

    fn heading_levels(content: &str) -> Vec<usize> {
        content
            .lines()
            .filter_map(|line| {
                let hashes = line
                    .chars()
                    .take_while(|character| *character == '#')
                    .count();
                (hashes > 0 && line.chars().nth(hashes) == Some(' ')).then_some(hashes)
            })
            .collect()
    }

    fn fenced_code_blocks(content: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut current = Vec::new();
        let mut in_block = false;
        for line in content.lines() {
            if line.starts_with("```") {
                if in_block {
                    blocks.push(current.join("\n"));
                    current.clear();
                }
                in_block = !in_block;
                continue;
            }
            if in_block {
                current.push(line);
            }
        }
        blocks
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        env::temp_dir().join(format!("agent-finance-skills-{name}-{unique}"))
    }
}
