use std::ffi::OsString;

use agent_finance_i18n::{LocaleId, LocaleSources, Translator};
use anyhow::Result;
use clap::error::ErrorKind;

const COMMANDS: &[(&str, &str)] = &[
    ("market", "cli-command-market"),
    ("tui", "cli-command-tui"),
    ("capabilities", "cli-command-capabilities"),
    ("profile", "cli-command-profile"),
    ("account", "cli-command-account"),
    ("order", "cli-command-order"),
    ("transfer", "cli-command-transfer"),
    ("state", "cli-command-state"),
    ("risk", "cli-command-risk"),
    ("audit", "cli-command-audit"),
    ("skills", "cli-command-skills"),
];

const GLOBAL_OPTIONS: &[(&str, &str)] = &[
    ("--locale <locale>", "cli-option-locale"),
    ("--proxy <url>", "cli-option-proxy"),
    ("--no-proxy", "cli-option-no-proxy"),
    ("--timezone <timezone>", "cli-option-timezone"),
    ("--timeout-seconds <seconds>", "cli-option-timeout-seconds"),
    ("-h, --help", "cli-option-help"),
    ("-V, --version", "cli-option-version"),
];

pub fn print_top_level_help_if_requested(args: &[OsString]) -> Result<bool> {
    if !top_level_help_requested(args) {
        return Ok(false);
    }

    let locale = resolve_locale_from_args(args);
    let translator = Translator::new(locale)?;
    print_top_level_help(&translator);
    Ok(true)
}

pub fn resolve_locale_from_args(args: &[OsString]) -> LocaleId {
    let locale_arg = locale_arg(args);
    LocaleSources::from_environment(locale_arg.as_deref(), None).locale
}

pub fn print_parse_error_guidance(args: &[OsString], kind: ErrorKind) -> Result<()> {
    if matches!(kind, ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        return Ok(());
    }

    let translator = Translator::new(resolve_locale_from_args(args))?;
    eprintln!("{}", translator.text("cli-parse-error-guidance"));
    Ok(())
}

fn print_top_level_help(translator: &Translator) {
    println!("{}", translator.text("cli-about"));
    println!();
    println!("{}", translator.text("cli-usage"));
    println!();
    println!("{}", translator.text("cli-commands-heading"));
    print_aligned(COMMANDS, translator);
    println!();
    println!("{}", translator.text("cli-options-heading"));
    print_aligned(GLOBAL_OPTIONS, translator);
    println!();
    println!("{}", translator.text("cli-after-help"));
}

fn print_aligned(rows: &[(&str, &str)], translator: &Translator) {
    let width = rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or_default();
    for (label, key) in rows {
        println!("  {label:<width$}  {}", translator.text(key));
    }
}

fn top_level_help_requested(args: &[OsString]) -> bool {
    let mut index = 1;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            return false;
        };
        match arg {
            "-h" | "--help" => return true,
            "--locale" | "--proxy" | "--timezone" | "--timeout-seconds" => index += 2,
            "--no-proxy" => index += 1,
            value
                if value.starts_with("--locale=")
                    || value.starts_with("--proxy=")
                    || value.starts_with("--timezone=")
                    || value.starts_with("--timeout-seconds=") =>
            {
                index += 1;
            }
            value if value.starts_with('-') => index += 1,
            _ => return false,
        }
    }
    false
}

fn locale_arg(args: &[OsString]) -> Option<String> {
    let mut index = 1;
    while index < args.len() {
        let arg = args[index].to_str()?;
        if let Some(value) = arg.strip_prefix("--locale=") {
            return Some(value.to_owned());
        }
        if arg == "--locale" {
            return args.get(index + 1)?.to_str().map(str::to_owned);
        }
        index += 1;
    }
    None
}

#[allow(dead_code)]
fn supported_locale_labels() -> String {
    LocaleId::ALL
        .into_iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn detects_only_top_level_help() {
        assert!(top_level_help_requested(&args(&[
            "agent-finance",
            "--locale",
            "ja",
            "--help"
        ])));
        assert!(!top_level_help_requested(&args(&[
            "agent-finance",
            "market",
            "--help"
        ])));
    }

    #[test]
    fn extracts_locale_flag_forms() {
        assert_eq!(
            locale_arg(&args(&["agent-finance", "--locale", "ko", "--help"])).as_deref(),
            Some("ko")
        );
        assert_eq!(
            locale_arg(&args(&["agent-finance", "--locale=zh-CN", "--help"])).as_deref(),
            Some("zh-CN")
        );
    }
}
