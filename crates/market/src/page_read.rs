use anyhow::{Context, Result, anyhow};
use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;
use wreq::{
    Client,
    header::{ACCEPT, CONTENT_TYPE},
};

use crate::args::ReadUrlProvider;
use crate::http::utc_now;

#[derive(Debug, Clone, Serialize)]
pub struct PageReadReport {
    pub url: String,
    pub provider: String,
    pub fetched_at_utc: String,
    pub source_url: String,
    pub title: Option<String>,
    pub word_count: usize,
    pub char_count: usize,
    pub truncated: bool,
    pub content: String,
    pub errors: Vec<PageReadError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageReadError {
    pub provider: String,
    pub error: String,
}

pub async fn read_url(
    client: &Client,
    url: &str,
    provider: ReadUrlProvider,
    max_chars: usize,
) -> Result<PageReadReport> {
    let normalized = normalize_url(url)?;
    let providers = providers_for_url(provider, &normalized);
    let mut errors = Vec::new();

    for provider in providers {
        match read_with_provider(client, &normalized, provider, max_chars).await {
            Ok(mut report) => {
                report.errors = errors;
                return Ok(report);
            }
            Err(error) => errors.push(PageReadError {
                provider: provider.label().to_string(),
                error: format!("{error:#}"),
            }),
        }
    }

    Err(anyhow!(
        "no URL reader provider returned usable content for {normalized}: {}",
        errors
            .iter()
            .map(|error| format!("{}={}", error.provider, error.error))
            .collect::<Vec<_>>()
            .join("; ")
    ))
}

async fn read_with_provider(
    client: &Client,
    url: &str,
    provider: ReadUrlProvider,
    max_chars: usize,
) -> Result<PageReadReport> {
    let source_url = provider_url(url, provider)?;
    let response = client
        .get(&source_url)
        .header(ACCEPT, "text/markdown,text/plain,text/html,*/*")
        .send()
        .await
        .with_context(|| format!("{} request failed", provider.label()))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .text()
        .await
        .with_context(|| format!("{} response body read failed", provider.label()))?;
    if !status.is_success() {
        return Err(anyhow!(
            "{} returned HTTP {status}: {}",
            provider.label(),
            body.chars().take(500).collect::<String>()
        ));
    }

    let extracted = match provider {
        ReadUrlProvider::Direct => direct_body_to_content(&body, content_type.as_deref()),
        ReadUrlProvider::Defuddle | ReadUrlProvider::Jina => ExtractedContent {
            title: title_from_content(&body),
            content: body,
        },
        ReadUrlProvider::Auto => unreachable!("auto is expanded before provider fetch"),
    };
    let mut content = extracted.content;
    content = normalize_content(&content);
    ensure_usable_content(provider, &content)?;

    let title = extracted.title.or_else(|| title_from_content(&content));
    let word_count = content.split_whitespace().count();
    let char_count = content.chars().count();
    let (content, truncated) = truncate_chars(&content, max_chars);

    Ok(PageReadReport {
        url: url.to_string(),
        provider: provider.label().to_string(),
        fetched_at_utc: utc_now(),
        source_url,
        title,
        word_count,
        char_count,
        truncated,
        content,
        errors: Vec::new(),
    })
}

fn providers_for_url(provider: ReadUrlProvider, url: &str) -> Vec<ReadUrlProvider> {
    match provider {
        ReadUrlProvider::Auto if is_sec_archive_url(url) => vec![
            ReadUrlProvider::Jina,
            ReadUrlProvider::Defuddle,
            ReadUrlProvider::Direct,
        ],
        ReadUrlProvider::Auto => vec![
            ReadUrlProvider::Direct,
            ReadUrlProvider::Jina,
            ReadUrlProvider::Defuddle,
        ],
        provider => vec![provider],
    }
}

fn is_sec_archive_url(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            let host_matches = url
                .host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("www.sec.gov"));
            host_matches.then(|| url.path().starts_with("/Archives/"))
        })
        .unwrap_or(false)
}

fn provider_url(url: &str, provider: ReadUrlProvider) -> Result<String> {
    match provider {
        ReadUrlProvider::Direct => Ok(url.to_string()),
        ReadUrlProvider::Defuddle => {
            let parsed = Url::parse(url)?;
            let host = parsed
                .host_str()
                .ok_or_else(|| anyhow!("URL has no host: {url}"))?;
            let mut target = host.to_string();
            if let Some(port) = parsed.port() {
                target.push(':');
                target.push_str(&port.to_string());
            }
            target.push_str(parsed.path());
            if let Some(query) = parsed.query() {
                target.push('?');
                target.push_str(query);
            }
            Ok(format!("https://defuddle.md/{target}"))
        }
        ReadUrlProvider::Jina => Ok(format!("https://r.jina.ai/{url}")),
        ReadUrlProvider::Auto => Err(anyhow!("auto does not have a single provider URL")),
    }
}

fn normalize_url(url: &str) -> Result<String> {
    let trimmed = url.trim();
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    Url::parse(&with_scheme).with_context(|| format!("invalid URL: {url}"))?;
    Ok(with_scheme)
}

#[derive(Debug)]
struct ExtractedContent {
    content: String,
    title: Option<String>,
}

fn direct_body_to_content(body: &str, content_type: Option<&str>) -> ExtractedContent {
    if content_type
        .map(|value| value.contains("text/html") || value.contains("application/xhtml"))
        .unwrap_or_else(|| looks_like_html(body))
    {
        html_to_text(body)
    } else {
        ExtractedContent {
            content: body.to_string(),
            title: None,
        }
    }
}

fn html_to_text(body: &str) -> ExtractedContent {
    let body = strip_html_tag_blocks(body, &["script", "style", "noscript"]);
    let document = Html::parse_document(&body);
    let title = title_from_html(&body);
    let body_selector = Selector::parse("body").expect("valid body selector");
    let mut text = String::new();
    if let Some(title) = title.as_deref() {
        text.push_str("# ");
        text.push_str(title);
        text.push_str("\n\n");
    }
    if let Some(body) = document.select(&body_selector).next() {
        push_text_nodes(&mut text, body.text());
    } else {
        push_text_nodes(&mut text, document.root_element().text());
    }
    ExtractedContent {
        content: text,
        title,
    }
}

fn ensure_usable_content(provider: ReadUrlProvider, content: &str) -> Result<()> {
    let words = content.split_whitespace().count();
    if words < 40 {
        return Err(anyhow!(
            "{} returned too little readable content: {words} words",
            provider.label()
        ));
    }
    if contains_blocked_marker(content) {
        return Err(anyhow!(
            "{} returned likely anti-bot or blocked content",
            provider.label()
        ));
    }
    Ok(())
}

fn push_text_nodes<'a>(output: &mut String, nodes: impl Iterator<Item = &'a str>) {
    for text in nodes {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        if !output.ends_with([' ', '\n']) {
            output.push(' ');
        }
        output.push_str(text);
    }
}

fn contains_blocked_marker(content: &str) -> bool {
    [
        "access denied",
        "captcha",
        "cloudflare",
        "checking your browser",
        "please enable javascript",
    ]
    .iter()
    .any(|needle| contains_ascii_case_insensitive(content, needle))
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn title_from_html(body: &str) -> Option<String> {
    let document = Html::parse_document(body);
    let selector = Selector::parse("title").ok()?;
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<Vec<_>>()
        .join(" ");
    let title = normalize_inline_text(&title);
    (!title.is_empty()).then_some(title)
}

fn title_from_content(content: &str) -> Option<String> {
    for line in content.lines().take(8) {
        let line = line.trim();
        if let Some(title) = line.strip_prefix("title:") {
            return Some(title.trim().trim_matches('"').to_string());
        }
        if let Some(title) = line.strip_prefix("Title:") {
            return Some(title.trim().to_string());
        }
        if let Some(title) = line.strip_prefix("# ") {
            return Some(title.trim().to_string());
        }
    }
    None
}

fn truncate_chars(content: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (content.to_string(), false);
    }
    let mut end = 0;
    for (count, (index, character)) in content.char_indices().enumerate() {
        if count >= max_chars {
            return (content[..end].to_string(), true);
        }
        end = index + character.len_utf8();
    }
    (content.to_string(), false)
}

fn normalize_content(value: &str) -> String {
    let mut output = Vec::new();
    let mut previous_blank = false;
    for line in value.lines() {
        let line = normalize_inline_text(line);
        let blank = line.is_empty();
        if blank {
            if !previous_blank {
                output.push(String::new());
            }
        } else {
            output.push(line);
        }
        previous_blank = blank;
    }
    output.join("\n").trim().to_string()
}

fn normalize_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn looks_like_html(body: &str) -> bool {
    let lower = body
        .chars()
        .take(500)
        .collect::<String>()
        .to_ascii_lowercase();
    lower.contains("<html") || lower.contains("<body") || lower.contains("<!doctype")
}

fn strip_html_tag_blocks(input: &str, tags: &[&str]) -> String {
    let mut output = input.to_string();
    for tag in tags {
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(start) = lower.find(&format!("<{tag}")) else {
                break;
            };
            let Some(relative_end) = lower[start..].find(&format!("</{tag}>")) else {
                break;
            };
            let end = start + relative_end + tag.len() + 3;
            output.replace_range(start..end, " ");
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defuddle_url_keeps_sec_archive_path_without_double_scheme() {
        let url = "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo.htm";
        let provider_url = provider_url(url, ReadUrlProvider::Defuddle).expect("provider URL");
        assert_eq!(
            provider_url,
            "https://defuddle.md/www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo.htm"
        );
    }

    #[test]
    fn direct_html_extracts_title_and_visible_text() {
        let html = r#"
            <html>
              <head><title>Credo 10-Q</title></head>
              <body><script>ignored()</script><h1>FORM 10-Q</h1><p>Revenue increased with hyperscale data center customers.</p></body>
            </html>
        "#;
        let extracted = html_to_text(html);
        assert_eq!(extracted.title.as_deref(), Some("Credo 10-Q"));
        assert!(extracted.content.contains("# Credo 10-Q"));
        assert!(extracted.content.contains("FORM 10-Q"));
        assert!(
            extracted
                .content
                .contains("Revenue increased with hyperscale data center customers.")
        );
        assert!(!extracted.content.contains("ignored()"));
    }

    #[test]
    fn unusable_block_pages_are_rejected() {
        let content = "
            This page contains enough words to avoid the short-content guard and verify the blocked
            marker path directly. The response keeps repeating filler words for a normal-looking
            paragraph, but it still says Access Denied and asks for a Cloudflare captcha challenge
            before any useful filing or article text becomes available to the reader.
        ";
        let error = ensure_usable_content(ReadUrlProvider::Direct, content).expect_err("blocked");
        assert!(error.to_string().contains("anti-bot"));
    }

    #[test]
    fn short_content_is_rejected_before_anti_bot_markers() {
        let error = ensure_usable_content(ReadUrlProvider::Direct, "short readable page")
            .expect_err("short");
        assert!(error.to_string().contains("too little"));
    }

    #[test]
    fn sec_archive_auto_prefers_reader_fallbacks_before_direct() {
        let providers = providers_for_url(
            ReadUrlProvider::Auto,
            "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo.htm",
        );
        assert_eq!(
            providers,
            vec![
                ReadUrlProvider::Jina,
                ReadUrlProvider::Defuddle,
                ReadUrlProvider::Direct
            ]
        );
    }
}
