use anyhow::Result;
use serde::Serialize;

pub(crate) fn print_submit_report(
    json_output: bool,
    report: &agent_finance_core::SubmitSnapshot,
) -> Result<()> {
    print_json_or_text(json_output, report, || {
        let findings = risk_findings_text(&report.risk);
        format!(
            "submitted intent {}\nmode: {}\nexecution: {}\nrisk allowed: {}\n{}{}",
            report.intent_id,
            report.mode,
            report.execution.kind,
            report.risk.allowed,
            findings,
            serde_json::to_string_pretty(&report.execution.payload).unwrap()
        )
    })
}

pub(crate) fn risk_findings_text(risk: &agent_finance_core::RiskDecision) -> String {
    if risk.findings.is_empty() {
        return String::new();
    }
    let mut text = String::from("risk findings:");
    for finding in &risk.findings {
        text.push_str(&format!(
            "\n- {} {}: {}",
            finding.severity, finding.code, finding.message
        ));
    }
    text.push('\n');
    text
}

pub(crate) fn submit_mode_from_flags(
    live: bool,
    test: bool,
) -> Result<agent_finance_core::SubmitMode> {
    match (live, test) {
        (true, true) => anyhow::bail!("--live and --test are mutually exclusive"),
        (true, false) => Ok(agent_finance_core::SubmitMode::Live),
        (false, true) => Ok(agent_finance_core::SubmitMode::Test),
        (false, false) => Ok(agent_finance_core::SubmitMode::DryRun),
    }
}

pub(crate) fn print_json_or_text<T, F>(json_output: bool, value: &T, text: F) -> Result<()>
where
    T: Serialize,
    F: FnOnce() -> String,
{
    if json_output {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", text());
    }
    Ok(())
}
