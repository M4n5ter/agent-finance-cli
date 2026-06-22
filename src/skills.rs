use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

const SUPPLEMENTARY_DIRS: &[&str] = &["references", "templates"];

pub fn print_list() -> Result<()> {
    let store = SkillStore::load()?;
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

pub fn get(name: &str, full: bool) -> Result<Option<String>> {
    SkillStore::load()?.render(name, full)
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
    fn load() -> Result<Self> {
        if let Some(skill_data_dir) = locate_skill_data_dir()? {
            return load_filesystem_store(&skill_data_dir).map(Self::Filesystem);
        }
        Ok(Self::Embedded(load_embedded_store()?))
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

fn load_filesystem_store(skill_data_dir: &Path) -> Result<Vec<SkillDocument>> {
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

        let skill_md = dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let content = fs::read_to_string(&skill_md)
            .with_context(|| format!("failed to read skill {}", skill_md.display()))?;
        let document = document_from_content(content, collect_supplementary_files(&dir)?)
            .with_context(|| format!("invalid skill frontmatter in {}", skill_md.display()))?;
        documents.push(document);
    }

    documents.sort_by(|left, right| left.info.name.cmp(&right.info.name));
    Ok(documents)
}

fn load_embedded_store() -> Result<Vec<SkillDocument>> {
    let mut documents = vec![
        embedded_document(
            "core",
            CORE,
            &[("references/command-map.md", CORE_COMMAND_MAP)],
        )?,
        embedded_document("crypto", CRYPTO, &[])?,
        embedded_document("history-indicators", HISTORY_INDICATORS, &[])?,
        embedded_document("prediction-markets", PREDICTION_MARKETS, &[])?,
        embedded_document("price", PRICE, &[])?,
        embedded_document("providers", PROVIDERS, &[])?,
        embedded_document("research-data", RESEARCH_DATA, &[])?,
    ];
    documents.sort_by(|left, right| left.info.name.cmp(&right.info.name));
    Ok(documents)
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

fn collect_supplementary_files(skill_dir: &Path) -> Result<Vec<SupplementaryFile>> {
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
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let relative = format!(
                "{subdir_name}/{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            files.push(SupplementaryFile {
                path: relative,
                content,
            });
        }
    }

    Ok(files)
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

const CORE: &str = include_str!("../skill-data/core/SKILL.md");
const CORE_COMMAND_MAP: &str = include_str!("../skill-data/core/references/command-map.md");
const CRYPTO: &str = include_str!("../skill-data/crypto/SKILL.md");
const HISTORY_INDICATORS: &str = include_str!("../skill-data/history-indicators/SKILL.md");
const PREDICTION_MARKETS: &str = include_str!("../skill-data/prediction-markets/SKILL.md");
const PRICE: &str = include_str!("../skill-data/price/SKILL.md");
const PROVIDERS: &str = include_str!("../skill-data/providers/SKILL.md");
const RESEARCH_DATA: &str = include_str!("../skill-data/research-data/SKILL.md");

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

        let store = SkillStore::Filesystem(load_filesystem_store(&skill_data).expect("skills"));

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
        let store = SkillStore::Embedded(load_embedded_store().expect("embedded store"));

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
    fn supplementary_files_append_references_before_templates() {
        let root = temp_test_dir("supplementary");
        let skill = root.join("core");
        fs::create_dir_all(skill.join("references")).expect("references dir");
        fs::create_dir_all(skill.join("templates")).expect("templates dir");
        fs::write(skill.join("references/commands.md"), "commands\n").expect("commands");
        fs::write(skill.join("templates/example.sh"), "example\n").expect("template");

        let files = collect_supplementary_files(&skill).expect("files");

        assert_eq!(
            files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["references/commands.md", "templates/example.sh"]
        );

        fs::remove_dir_all(root).ok();
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        env::temp_dir().join(format!("agent-finance-skills-{name}-{unique}"))
    }
}
