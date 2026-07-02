use std::ffi::OsString;

use agent_finance_i18n::{LocaleId, LocaleSources, Translator};
use anyhow::Result;
use clap::error::ErrorKind;
use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::cli::Cli;

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

pub fn print_help_if_requested(args: &[OsString]) -> Result<bool> {
    if !help_requested(args) {
        return Ok(false);
    }

    let locale = resolve_locale_from_args(args);
    if locale == LocaleId::EnUs {
        return Ok(false);
    }
    let translator = Translator::new(locale)?;
    let command = Cli::command();
    let global_options = visible_global_options(&command);
    let path = command_path(args, &command);
    if let Some(command) = command_for_path(command, &path) {
        print_command_help(&translator, command, &path, &global_options);
    } else {
        print_command_help(&translator, Cli::command(), &[], &global_options);
    }
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

fn print_command_help(
    translator: &Translator,
    command: Command,
    path: &[String],
    global_options: &[Arg],
) {
    println!("{}", command_about(translator, &command, path));
    println!();
    println!(
        "{} {}",
        translator.text("cli-usage-heading"),
        usage_for(path, &command)
    );

    let subcommands = visible_subcommands(&command);
    if !subcommands.is_empty() {
        println!();
        println!("{}", translator.text("cli-commands-heading"));
        print_command_rows(translator, path, &subcommands);
    }

    let positionals = visible_positionals(&command);
    if !positionals.is_empty() {
        println!();
        println!("{}", translator.text("cli-arguments-heading"));
        print_argument_rows(translator, path, &positionals);
    }

    let options = visible_options_with_globals(&command, global_options);
    if !options.is_empty() {
        println!();
        println!("{}", translator.text("cli-options-heading"));
        print_argument_rows(translator, path, &options);
    }
    println!();
    println!("{}", translator.text("cli-after-help"));
}

fn command_about(translator: &Translator, command: &Command, path: &[String]) -> String {
    localized_key_text(translator, &command_description_key(path))
        .or_else(|| command.get_about().map(ToString::to_string))
        .unwrap_or_else(|| translator.text("cli-about"))
}

fn usage_for(path: &[String], command: &Command) -> String {
    let mut usage = String::from("agent-finance");
    for segment in path {
        usage.push(' ');
        usage.push_str(segment);
    }
    if command.has_subcommands() {
        usage.push_str(" [OPTIONS] <COMMAND>");
    } else {
        usage.push_str(" [OPTIONS]");
        for option in visible_options(command)
            .into_iter()
            .filter(Arg::is_required_set)
        {
            if let Some(label) = argument_label(&option) {
                usage.push(' ');
                usage.push_str(&label);
            }
        }
        for positional in visible_positionals(command) {
            usage.push(' ');
            usage.push_str(&positional_usage_label(&positional));
        }
    }
    usage
}

fn visible_subcommands(command: &Command) -> Vec<Command> {
    command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .cloned()
        .collect()
}

fn visible_positionals(command: &Command) -> Vec<Arg> {
    command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set() && arg.is_positional())
        .cloned()
        .collect()
}

fn visible_options(command: &Command) -> Vec<Arg> {
    command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set() && !arg.is_positional())
        .cloned()
        .collect()
}

fn visible_global_options(command: &Command) -> Vec<Arg> {
    visible_options(command)
        .into_iter()
        .filter(Arg::is_global_set)
        .collect()
}

fn visible_options_with_globals(command: &Command, global_options: &[Arg]) -> Vec<Arg> {
    let mut options = visible_options(command);
    for global in global_options {
        if !options.iter().any(|arg| arg.get_id() == global.get_id()) {
            options.push(global.clone());
        }
    }
    options
}

fn print_command_rows(translator: &Translator, path: &[String], subcommands: &[Command]) {
    let rows = subcommands
        .iter()
        .map(|subcommand| {
            let name = subcommand.get_name().to_string();
            let mut child_path = path.to_vec();
            child_path.push(name.clone());
            let description = localized_key_text(translator, &command_description_key(&child_path))
                .or_else(|| subcommand.get_about().map(ToString::to_string))
                .unwrap_or_default();
            (name, description)
        })
        .collect::<Vec<_>>();
    print_owned_aligned(&rows);
}

fn print_argument_rows(translator: &Translator, path: &[String], args: &[Arg]) {
    let rows = args
        .iter()
        .filter_map(|arg| {
            argument_label(arg).map(|label| (label, argument_help(translator, path, arg)))
        })
        .collect::<Vec<_>>();
    print_owned_aligned(&rows);
}

fn argument_help(translator: &Translator, path: &[String], arg: &Arg) -> String {
    argument_description_keys(path, arg)
        .into_iter()
        .find_map(|key| localized_key_text(translator, &key))
        .or_else(|| arg.get_help().map(ToString::to_string))
        .unwrap_or_default()
}

fn argument_label(arg: &Arg) -> Option<String> {
    if arg.is_positional() {
        Some(positional_usage_label(arg))
    } else if let Some(long) = arg.get_long() {
        let mut label = String::new();
        if let Some(short) = arg.get_short() {
            label.push('-');
            label.push(short);
            label.push_str(", ");
        }
        label.push_str("--");
        label.push_str(long);
        if takes_value(arg) {
            label.push_str(" <");
            label.push_str(&value_name(arg));
            label.push('>');
        }
        Some(label)
    } else {
        None
    }
}

fn positional_usage_label(arg: &Arg) -> String {
    let name = value_name(arg);
    let repeated = arg
        .get_num_args()
        .map(|range| range.max_values() > 1)
        .unwrap_or(false);
    let label = if arg.is_required_set() {
        format!("<{name}>")
    } else {
        format!("[<{name}>]")
    };
    if repeated {
        format!("{label}...")
    } else {
        label
    }
}

fn value_name(arg: &Arg) -> String {
    arg.get_value_names()
        .and_then(|names| names.first())
        .map(|name| name.as_str().to_string())
        .unwrap_or_else(|| arg.get_id().as_str().to_ascii_uppercase())
}

fn takes_value(arg: &Arg) -> bool {
    if matches!(
        arg.get_action(),
        ArgAction::SetTrue
            | ArgAction::SetFalse
            | ArgAction::Count
            | ArgAction::Help
            | ArgAction::Version
    ) {
        return false;
    }
    arg.get_value_names().is_some()
        || arg
            .get_num_args()
            .map(|range| range.takes_values())
            .unwrap_or(false)
}

fn print_owned_aligned(rows: &[(String, String)]) {
    let width = rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or_default();
    for (label, description) in rows {
        println!("  {label:<width$}  {description}");
    }
}

fn help_requested(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .any(|arg| matches!(arg, "-h" | "--help"))
}

fn command_path(args: &[OsString], root: &Command) -> Vec<String> {
    let mut index = 1;
    let mut path = Vec::new();
    let mut command = root;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            return path;
        };
        match arg {
            "-h" | "--help" => return path,
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
            value if value.starts_with('-') => return path,
            value => {
                let Some(subcommand) = command
                    .get_subcommands()
                    .find(|subcommand| subcommand.get_name() == value)
                else {
                    index += 1;
                    continue;
                };
                path.push(value.to_string());
                command = subcommand;
                index += 1;
            }
        }
    }
    path
}

fn command_for_path(mut command: Command, path: &[String]) -> Option<Command> {
    for segment in path {
        let next = command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == segment)?;
        command = next.clone();
    }
    Some(command)
}

fn localized_key_text(translator: &Translator, key: &str) -> Option<String> {
    let value = translator.text(key);
    (value != format!("⟦{key}⟧")).then_some(value)
}

fn command_description_key(path: &[String]) -> String {
    match path {
        [] => "cli-about".to_string(),
        [command] => COMMANDS
            .iter()
            .find(|(name, _)| name == command)
            .map(|(_, key)| key.to_string())
            .unwrap_or_else(|| command_key(path)),
        _ => command_key(path),
    }
}

fn command_key(path: &[String]) -> String {
    format!("cli-command-{}", path.join("-"))
}

fn argument_description_keys(path: &[String], arg: &Arg) -> Vec<String> {
    let Some(long) = arg.get_long() else {
        return Vec::new();
    };
    match long {
        "full" if path == ["skills", "get"] => vec!["cli-option-skills-full".to_string()],
        _ => vec![
            format!("{}-option-{long}", command_key(path)),
            format!("cli-option-{long}"),
        ],
    }
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
    fn detects_help_anywhere_and_extracts_command_path() {
        let root = Cli::command();
        assert!(help_requested(&args(&[
            "agent-finance",
            "--locale",
            "ja",
            "--help"
        ])));
        assert!(help_requested(&args(&[
            "agent-finance",
            "market",
            "price",
            "--help"
        ])));
        assert_eq!(
            command_path(
                &args(&[
                    "agent-finance",
                    "--locale",
                    "zh",
                    "market",
                    "price",
                    "--help"
                ]),
                &root
            ),
            vec!["market", "price"]
        );
        assert_eq!(
            command_path(
                &args(&[
                    "agent-finance",
                    "--locale",
                    "zh",
                    "market",
                    "price",
                    "AAPL",
                    "--help"
                ]),
                &root
            ),
            vec!["market", "price"]
        );
        assert_eq!(
            command_path(
                &args(&[
                    "agent-finance",
                    "--locale",
                    "zh",
                    "order",
                    "create",
                    "AAPL",
                    "--help"
                ]),
                &root
            ),
            vec!["order", "create"]
        );
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
