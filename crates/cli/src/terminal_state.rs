use anyhow::{Result, anyhow};
use serde_json::json;

use crate::cli::{StateArgs, StateCommand};
use crate::terminal_output::{
    print_json_or_text, print_submit_report, risk_findings_text, submit_mode_from_flags,
};
use agent_finance_trading::TradingRuntime;

pub(crate) async fn run(args: StateArgs, timeout_seconds: u64) -> Result<()> {
    let runtime = TradingRuntime::new(timeout_seconds);
    match args.command {
        StateCommand::Create(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let change = futures_state_change(&args)?;
            let intent = agent_finance_core::FuturesStateIntent {
                profile: profile.name.clone(),
                provider: profile.provider.provider,
                environment: profile.provider.environment,
                change,
            };
            let risk = agent_finance_core::check_futures_state_intent(&profile, &intent, false);
            let envelope =
                agent_finance_core::create_futures_state_intent(intent, args.ttl_seconds)?;
            let path = runtime.save_intent_with_audit(
                &profile,
                &envelope,
                &risk,
                format!("created futures state intent {}", envelope.id),
            )?;
            print_json_or_text(
                args.json,
                &json!({ "intent": envelope, "risk": risk, "path": path }),
                || {
                    let findings = risk_findings_text(&risk);
                    format!(
                        "created futures state intent {}\nrisk allowed: {}\n{}path: {}",
                        envelope.id,
                        risk.allowed,
                        findings,
                        path.display()
                    )
                },
            )
        }
        StateCommand::Submit(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let mode = submit_mode_from_flags(args.live, false)?;
            let report = runtime
                .submit_futures_state_intent(&profile, &args.intent_id, mode)
                .await?;
            print_submit_report(args.json, &report)
        }
    }
}

fn futures_state_change(
    args: &crate::cli::StateCreateArgs,
) -> Result<agent_finance_core::FuturesStateChange> {
    match args.kind {
        crate::cli::TradingFuturesStateChangeKind::Leverage => {
            reject_present("margin type", args.margin_type.as_ref())?;
            reject_present("position mode", args.position_mode.as_ref())?;
            Ok(agent_finance_core::FuturesStateChange::Leverage {
                symbol: required_symbol(args)?.to_ascii_uppercase(),
                leverage: args
                    .leverage
                    .ok_or_else(|| anyhow!("leverage state change requires --leverage"))?,
            })
        }
        crate::cli::TradingFuturesStateChangeKind::MarginType => {
            reject_present("leverage", args.leverage.as_ref())?;
            reject_present("position mode", args.position_mode.as_ref())?;
            Ok(agent_finance_core::FuturesStateChange::MarginType {
                symbol: required_symbol(args)?.to_ascii_uppercase(),
                margin_type: args
                    .margin_type
                    .ok_or_else(|| anyhow!("margin-type state change requires --margin-type"))?
                    .into(),
            })
        }
        crate::cli::TradingFuturesStateChangeKind::PositionMode => {
            reject_present("symbol", args.symbol.as_ref())?;
            reject_present("leverage", args.leverage.as_ref())?;
            reject_present("margin type", args.margin_type.as_ref())?;
            Ok(agent_finance_core::FuturesStateChange::PositionMode {
                mode: args
                    .position_mode
                    .ok_or_else(|| anyhow!("position-mode state change requires --position-mode"))?
                    .into(),
            })
        }
    }
}

fn required_symbol(args: &crate::cli::StateCreateArgs) -> Result<&str> {
    args.symbol
        .as_deref()
        .ok_or_else(|| anyhow!("this futures state change requires --symbol"))
}

fn reject_present<T>(name: &str, value: Option<&T>) -> Result<()> {
    if value.is_some() {
        return Err(anyhow!("{name} is not valid for this state change kind"));
    }
    Ok(())
}
