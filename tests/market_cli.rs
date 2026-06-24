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

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent-finance"));
    command.args(args);
    command
}
