use std::path::Path;
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
        help.contains("用法： agent-finance [OPTIONS] <COMMAND>"),
        "top-level help should be localized: {help}"
    );
    assert!(
        help.contains("输出内置 AI Agent skill 文档"),
        "command descriptions should be localized: {help}"
    );
}

#[test]
fn subcommand_help_uses_requested_locale() {
    let help = command_text(&["--locale", "zh", "market", "--help"]);
    assert!(
        help.contains("输出一个或多个标的的当前可观察价格摘要"),
        "market help should localize subcommand descriptions: {help}"
    );
    assert!(
        help.contains("用法： agent-finance market [OPTIONS] <COMMAND>"),
        "market help should use localized chrome: {help}"
    );

    let price_help = command_text(&["--locale", "zh", "market", "price", "--help"]);
    assert!(
        price_help.contains("输出一个或多个标的的当前可观察价格摘要"),
        "market price help should localize command about text: {price_help}"
    );
    assert!(
        price_help.contains("用法： agent-finance market price [OPTIONS] <SYMBOLS>..."),
        "market price help should preserve required positional usage: {price_help}"
    );
    assert!(
        price_help.contains("参数：") && price_help.contains("<SYMBOLS>..."),
        "market price help should render positionals under Arguments: {price_help}"
    );
    assert!(
        price_help.contains("选项：") && price_help.contains("--session <SESSION>"),
        "market price help should keep value-taking flags under Options: {price_help}"
    );
    assert!(
        price_help.contains("--locale <LOCALE>")
            && price_help.contains("--proxy <PROXY>")
            && price_help.contains("--timeout-seconds <TIMEOUT_SECONDS>"),
        "market price help should include inherited global options: {price_help}"
    );
    assert!(
        price_help.contains("--json                               输出 JSON。"),
        "bool flags should not render a fake value name: {price_help}"
    );

    let price_help_after_symbol =
        command_text(&["--locale", "zh", "market", "price", "AAPL", "--help"]);
    assert!(
        price_help_after_symbol
            .contains("用法： agent-finance market price [OPTIONS] <SYMBOLS>..."),
        "localized help after a positional should stay on the leaf command: {price_help_after_symbol}"
    );
}

#[test]
fn localized_write_command_help_preserves_required_options() {
    let order_help = command_text(&["--locale", "zh", "order", "--help"]);
    assert!(
        order_help.contains("创建并持久化订单意图"),
        "order help should localize write command descriptions: {order_help}"
    );

    let create_help = command_text(&["--locale", "zh", "order", "create", "--help"]);
    assert!(
        create_help.contains(
            "用法： agent-finance order create [OPTIONS] --market <MARKET> --side <SIDE> --kind <KIND> --quantity <QUANTITY> <SYMBOL>"
        ),
        "order create help should preserve required options in usage: {create_help}"
    );
    assert!(
        create_help.contains("--quantity <QUANTITY>") && create_help.contains("订单数量。"),
        "order create option help should use localized argument descriptions: {create_help}"
    );

    let create_help_after_symbol =
        command_text(&["--locale", "zh", "order", "create", "AAPL", "--help"]);
    assert!(
        create_help_after_symbol.contains(
            "用法： agent-finance order create [OPTIONS] --market <MARKET> --side <SIDE> --kind <KIND> --quantity <QUANTITY> <SYMBOL>"
        ),
        "localized help after a positional should preserve the order create leaf command: {create_help_after_symbol}"
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
fn market_providers_human_output_uses_requested_locale() {
    let output = command_text(&["--locale", "zh", "market", "providers"]);
    assert!(
        output.contains("官方性") && output.contains("能力"),
        "provider human output should localize table chrome: {output}"
    );
    assert!(
        output.contains("yahoo") || output.contains("auto"),
        "provider identifiers should remain stable source values: {output}"
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

    let mut package_root_command = command(&["--locale", "zh", "skills", "get", "core", "--full"]);
    package_root_command.env("AGENT_FINANCE_SKILL_DATA_DIR", workspace_skill_data_dir());
    let package_root_core = output_text(package_root_command);
    assert!(
        package_root_core.contains("## 命令地图"),
        "filesystem skill-data should localize supplementary files: {package_root_core}"
    );
    assert!(
        !package_root_core.contains("## Command Map"),
        "filesystem skill-data should not fall back to English supplementary files when locale resources exist: {package_root_core}"
    );
}

fn command_text(args: &[&str]) -> String {
    output_text(command(args))
}

fn output_text(mut command: Command) -> String {
    let output = command
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

fn workspace_skill_data_dir() -> &'static Path {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("skill-data"))
        .expect("workspace root");
    Box::leak(path.into_boxed_path())
}
