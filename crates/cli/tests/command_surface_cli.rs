use std::process::Command;

#[test]
fn market_providers_is_the_read_only_capability_entrypoint() {
    let output = command(&["market", "providers", "--json"])
        .output()
        .expect("agent-finance command should start");
    assert!(
        output.status.success(),
        "market providers should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let profiles: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("providers output should be JSON");
    assert!(
        profiles
            .as_array()
            .expect("provider profiles should be an array")
            .iter()
            .any(|profile| profile["provider"] == "auto"),
        "provider matrix should include auto routing profile: {profiles}"
    );
}

#[test]
fn read_only_commands_are_not_exposed_at_the_root() {
    let output = command(&["providers", "--json"])
        .output()
        .expect("agent-finance command should start");
    assert!(
        !output.status.success(),
        "root providers should be rejected after read-only commands moved under market"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized subcommand 'providers'"),
        "root providers should fail at the clap command boundary: stderr={stderr}"
    );
}

#[test]
fn write_commands_use_action_names_not_internal_intent_names() {
    let help = command_text(&["order", "--help"]);
    assert!(
        help.contains("create") && help.contains("cancel"),
        "order help should expose user action names: {help}"
    );
    assert!(
        !help.contains("cancel-intent"),
        "order help should not expose the old cancel-intent command: {help}"
    );

    assert_unknown_subcommand(&["order", "intent", "--help"], "intent");
    assert_unknown_subcommand(&["order", "cancel-intent", "--help"], "cancel-intent");
    assert_unknown_subcommand(&["transfer", "intent", "--help"], "intent");
    assert_unknown_subcommand(&["state", "intent", "--help"], "intent");
}

#[test]
fn top_level_help_uses_requested_locale() {
    let help = command_text(&["--locale", "zh", "--help"]);

    assert!(
        help.contains("用法：agent-finance [选项] <命令>"),
        "top-level help should be localized: {help}"
    );
    assert!(
        help.contains("输出内置 AI Agent skill 文档"),
        "command descriptions should be localized: {help}"
    );
}

#[test]
fn locale_does_not_localize_json_contracts() {
    let output = command(&["--locale", "zh", "market", "providers", "--json"])
        .output()
        .expect("agent-finance command should start");
    assert!(
        output.status.success(),
        "localized JSON command should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let profiles: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("providers output should stay JSON");
    assert!(
        profiles
            .as_array()
            .expect("provider profiles should be an array")
            .iter()
            .any(|profile| profile["provider"] == "auto"),
        "provider enum values should remain stable English: {profiles}"
    );
}

#[test]
fn parse_errors_include_localized_guidance() {
    let output = command(&["--locale", "zh", "unknown-command"])
        .output()
        .expect("agent-finance command should start");

    assert!(
        !output.status.success(),
        "unknown command should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("无法解析命令"),
        "stderr should include localized guidance: {stderr}"
    );
    assert!(
        stderr.contains("unrecognized subcommand 'unknown-command'"),
        "stderr should preserve clap's detailed parser error: {stderr}"
    );
}

#[test]
fn skills_commands_use_requested_locale_without_localizing_command_blocks() {
    let list = command_text(&["--locale", "zh", "skills", "list"]);
    assert!(
        list.contains("入口指南"),
        "skills list should use localized frontmatter: {list}"
    );

    let core = command_text(&["--locale", "ko", "skills", "get", "core", "--full"]);
    assert!(
        core.contains("현지화 안내"),
        "skills get should return localized body: {core}"
    );
    assert!(
        core.contains("agent-finance market providers"),
        "command blocks should remain stable English commands: {core}"
    );
}

fn command_text(args: &[&str]) -> String {
    let output = command(args)
        .output()
        .expect("agent-finance command should start");
    assert!(
        output.status.success(),
        "command should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf8")
}

fn assert_unknown_subcommand(args: &[&str], name: &str) {
    let output = command(args)
        .output()
        .expect("agent-finance command should start");
    assert!(
        !output.status.success(),
        "{args:?} should be rejected after public write commands moved to action names"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("unrecognized subcommand '{name}'")),
        "expected clap to reject {name:?} at command boundary: stderr={stderr}"
    );
}

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-finance"));
    command.args(args);
    command
}
