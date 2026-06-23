use anyhow::{Result, anyhow};
use chrono::Utc;
use serde_json::json;

use crate::cli::{
    AccountArgs, AccountCommand, AuditArgs, AuditCommand, CapabilitiesArgs, OrderArgs,
    OrderCommand, ProfileArgs, ProfileCommand, RiskArgs, RiskCommand, TransferArgs,
    TransferCommand,
};
use crate::terminal_write::{
    ExpectedIntentKind, WriteMode, binance_client, check_order_with_runtime_limits, load_profile,
    print_json_or_text, print_submit_report, risk_findings_text, save_intent_with_audit,
    submit_intent,
};

pub(crate) fn run_capabilities(args: CapabilitiesArgs) -> Result<()> {
    let report = agent_finance_core::CapabilityReport::new(vec![
        agent_finance_binance::provider_capability(),
    ]);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("command model: {}", report.command_model);
        for provider in report.providers {
            println!("\nprovider: {}", provider.provider);
            for capability in provider.capabilities {
                println!(
                    "- {} [{}] markets={}",
                    capability.name,
                    capability.access,
                    capability.markets.join(",")
                );
                for note in capability.notes {
                    println!("  {note}");
                }
            }
        }
        println!("\nsafety:");
        for item in report.safety_model {
            println!("- {item}");
        }
    }
    Ok(())
}

pub(crate) async fn run_profile(args: ProfileArgs, timeout_seconds: u64) -> Result<()> {
    let store = agent_finance_core::ProfileStore::from_default_dir()?;
    match args.command {
        ProfileCommand::Path(args) => {
            let path = store.path(&args.profile);
            print_json_or_text(
                args.json,
                &json!({ "profile": args.profile, "path": path }),
                || path.display().to_string(),
            )
        }
        ProfileCommand::Template(args) => {
            let content = agent_finance_binance::profile_template(&args.profile);
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "profile": args.profile,
                        "content": content,
                    }))?
                );
            } else {
                print!("{content}");
            }
            Ok(())
        }
        ProfileCommand::Explain(args) => {
            let profile = store.load(&args.profile)?;
            print_json_or_text(args.json, &profile, || explain_profile(&profile))
        }
        ProfileCommand::Doctor(args) => {
            let profile = store.load(&args.profile)?;
            let mut checks = vec![json!({
                "name": "profile-parse",
                "ok": true,
                "message": "profile TOML parsed successfully",
            })];
            let key_ok = std::env::var(&profile.provider.api_key_env).is_ok();
            let secret_ok = std::env::var(&profile.provider.api_secret_env).is_ok();
            checks.push(json!({
                "name": "api-key-env",
                "ok": key_ok,
                "message": format!("{} {}", profile.provider.api_key_env, if key_ok { "is set" } else { "is missing" }),
            }));
            checks.push(json!({
                "name": "api-secret-env",
                "ok": secret_ok,
                "message": format!("{} {}", profile.provider.api_secret_env, if secret_ok { "is set" } else { "is missing" }),
            }));
            if key_ok && secret_ok {
                match binance_client(&profile, timeout_seconds)?
                    .account_permissions()
                    .await
                {
                    Ok(payload) => checks.push(json!({
                        "name": "binance-permissions",
                        "ok": true,
                        "message": "Binance API key permission endpoint succeeded",
                        "payload": payload,
                    })),
                    Err(error) => checks.push(json!({
                        "name": "binance-permissions",
                        "ok": false,
                        "message": format!("{error:#}"),
                    })),
                }
            }
            let report = json!({
                "profile": args.profile,
                "checks": checks,
            });
            print_json_or_text(args.json, &report, || {
                report["checks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|check| {
                        format!(
                            "{}: {} - {}",
                            if check["ok"].as_bool().unwrap_or(false) {
                                "ok"
                            } else {
                                "fail"
                            },
                            check["name"].as_str().unwrap_or("unknown"),
                            check["message"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
    }
}

pub(crate) async fn run_account(args: AccountArgs, timeout_seconds: u64) -> Result<()> {
    match args.command {
        AccountCommand::Permissions(args) => {
            let profile = load_profile(&args.profile)?;
            let payload = binance_client(&profile, timeout_seconds)?
                .account_permissions()
                .await?;
            print_json_or_text(args.json, &payload, || {
                serde_json::to_string_pretty(&payload).unwrap()
            })
        }
        AccountCommand::Balances(args) => {
            let profile = load_profile(&args.profile)?;
            let payload = binance_client(&profile, timeout_seconds)?
                .spot_account()
                .await?;
            print_json_or_text(args.json, &payload, || {
                serde_json::to_string_pretty(&payload).unwrap()
            })
        }
        AccountCommand::Positions(args) => {
            let profile = load_profile(&args.profile)?;
            let payload = binance_client(&profile, timeout_seconds)?
                .futures_account()
                .await?;
            print_json_or_text(args.json, &payload, || {
                serde_json::to_string_pretty(&payload).unwrap()
            })
        }
    }
}

pub(crate) async fn run_order(args: OrderArgs, timeout_seconds: u64) -> Result<()> {
    match args.command {
        OrderCommand::Intent(args) => {
            let profile = load_profile(&args.profile)?;
            let market = args.market.into();
            let intent = agent_finance_core::OrderIntent {
                profile: profile.name.clone(),
                provider: profile.provider.provider,
                environment: profile.provider.environment,
                market,
                symbol: args.symbol.to_ascii_uppercase(),
                side: args.side.into(),
                quantity: args.quantity.parse()?,
                spec: agent_finance_core::OrderSpec::new(
                    market,
                    args.kind.into(),
                    parse_optional_decimal(args.price)?,
                    parse_optional_decimal(args.valuation_price)?,
                    parse_optional_decimal(args.stop_price)?,
                    args.time_in_force.map(Into::into),
                )?,
                reduce_only: args.reduce_only,
                position_side: args.position_side.map(Into::into),
                client_order_id: format!("af-{}", Utc::now().timestamp_millis()),
            };
            let risk = agent_finance_core::check_order_intent(&profile, &intent, false);
            let envelope = agent_finance_core::create_order_intent(intent, args.ttl_seconds)?;
            let path = save_intent_with_audit(
                &profile,
                &envelope,
                &risk,
                format!("created order intent {}", envelope.id),
            )?;
            print_json_or_text(
                args.json,
                &json!({ "intent": envelope, "risk": risk, "path": path }),
                || {
                    format!(
                        "created order intent {}\nrisk allowed: {}\npath: {}",
                        envelope.id,
                        risk.allowed,
                        path.display()
                    )
                },
            )
        }
        OrderCommand::CancelIntent(args) => {
            let profile = load_profile(&args.profile)?;
            let intent = agent_finance_core::CancelIntent {
                profile: profile.name.clone(),
                provider: profile.provider.provider,
                environment: profile.provider.environment,
                market: args.market.into(),
                symbol: args.symbol.to_ascii_uppercase(),
                target: agent_finance_core::OrderIdentifier::new(
                    args.order_id,
                    args.client_order_id,
                )?,
            };
            let risk = agent_finance_core::check_cancel_intent(&profile, &intent, false);
            let envelope = agent_finance_core::create_cancel_intent(intent, args.ttl_seconds)?;
            let path = save_intent_with_audit(
                &profile,
                &envelope,
                &risk,
                format!("created cancel intent {}", envelope.id),
            )?;
            print_json_or_text(
                args.json,
                &json!({ "intent": envelope, "risk": risk, "path": path }),
                || {
                    format!(
                        "created cancel intent {}\nrisk allowed: {}\npath: {}",
                        envelope.id,
                        risk.allowed,
                        path.display()
                    )
                },
            )
        }
        OrderCommand::Submit(args) => {
            let profile = load_profile(&args.profile)?;
            let mode = WriteMode::from_flags(args.live, args.test)?;
            let report = submit_intent(
                &profile,
                &args.intent_id,
                ExpectedIntentKind::Order,
                mode,
                timeout_seconds,
            )
            .await?;
            print_submit_report(args.json, &report)
        }
        OrderCommand::Query(args) => {
            let profile = load_profile(&args.profile)?;
            let target =
                agent_finance_core::OrderIdentifier::new(args.order_id, args.client_order_id)?;
            let response = binance_client(&profile, timeout_seconds)?
                .query_order(args.market.into(), &args.symbol, &target)
                .await?;
            print_json_or_text(args.json, &response, || {
                serde_json::to_string_pretty(&response).unwrap()
            })
        }
        OrderCommand::Open(args) => {
            let profile = load_profile(&args.profile)?;
            let response = binance_client(&profile, timeout_seconds)?
                .open_orders(args.market.into(), args.symbol.as_deref())
                .await?;
            print_json_or_text(args.json, &response, || {
                serde_json::to_string_pretty(&response).unwrap()
            })
        }
    }
}

pub(crate) async fn run_transfer(args: TransferArgs, timeout_seconds: u64) -> Result<()> {
    match args.command {
        TransferCommand::Intent(args) => {
            let profile = load_profile(&args.profile)?;
            let intent = agent_finance_core::TransferIntent {
                profile: profile.name.clone(),
                provider: profile.provider.provider,
                environment: profile.provider.environment,
                direction: args.direction.into(),
                asset: args.asset.to_ascii_uppercase(),
                amount: args.amount.parse()?,
                client_transfer_id: format!("af-{}", Utc::now().timestamp_millis()),
            };
            let risk = agent_finance_core::check_transfer_intent(&profile, &intent, false);
            let envelope = agent_finance_core::create_transfer_intent(intent, args.ttl_seconds)?;
            let path = save_intent_with_audit(
                &profile,
                &envelope,
                &risk,
                format!("created transfer intent {}", envelope.id),
            )?;
            print_json_or_text(
                args.json,
                &json!({ "intent": envelope, "risk": risk, "path": path }),
                || {
                    format!(
                        "created transfer intent {}\nrisk allowed: {}\npath: {}",
                        envelope.id,
                        risk.allowed,
                        path.display()
                    )
                },
            )
        }
        TransferCommand::Submit(args) => {
            let profile = load_profile(&args.profile)?;
            let mode = WriteMode::from_flags(args.live, false)?;
            let report = submit_intent(
                &profile,
                &args.intent_id,
                ExpectedIntentKind::Transfer,
                mode,
                timeout_seconds,
            )
            .await?;
            print_submit_report(args.json, &report)
        }
        TransferCommand::History(args) => {
            let profile = load_profile(&args.profile)?;
            ensure_live_sapi_profile(&profile, "transfer history")?;
            let response = binance_client(&profile, timeout_seconds)?
                .transfer_history(args.direction.into(), args.current, args.size)
                .await?;
            print_json_or_text(args.json, &response, || {
                serde_json::to_string_pretty(&response).unwrap()
            })
        }
    }
}

pub(crate) fn run_risk(args: RiskArgs) -> Result<()> {
    match args.command {
        RiskCommand::Check(args) => {
            let profile = load_profile(&args.profile)?;
            let envelope =
                agent_finance_core::IntentStore::from_default_dir()?.load(&args.intent_id)?;
            let risk = match &envelope.kind {
                agent_finance_core::IntentKind::Order(intent) => {
                    check_order_with_runtime_limits(&profile, intent, args.live)?
                }
                agent_finance_core::IntentKind::Cancel(intent) => {
                    agent_finance_core::check_cancel_intent(&profile, intent, args.live)
                }
                agent_finance_core::IntentKind::Transfer(intent) => {
                    agent_finance_core::check_transfer_intent(&profile, intent, args.live)
                }
                agent_finance_core::IntentKind::FuturesState(intent) => {
                    agent_finance_core::check_futures_state_intent(&profile, intent, args.live)
                }
            };
            print_json_or_text(args.json, &risk, || {
                let findings = risk_findings_text(&risk);
                if findings.is_empty() {
                    format!("allowed: {}", risk.allowed)
                } else {
                    format!("allowed: {}\n{findings}", risk.allowed)
                }
            })
        }
        RiskCommand::Explain(args) => {
            let profile = load_profile(&args.profile)?;
            let used = agent_finance_core::daily_live_order_notional_used_today(&profile)?;
            let report = json!({
                "profile": profile.name,
                "provider": profile.provider.provider,
                "environment": profile.provider.environment,
                "allow_live": profile.risk.allow_live,
                "max_daily_order_notional_usdt": profile.risk.max_daily_order_notional_usdt,
                "daily_order_notional_used_utc": used.to_string(),
                "allowed_symbols": profile.risk.allowed_symbols,
                "allowed_transfers": profile.risk.allowed_transfers,
                "allowed_futures_state_changes": profile.risk.allowed_futures_state_changes,
            });
            print_json_or_text(args.json, &report, || {
                serde_json::to_string_pretty(&report).unwrap()
            })
        }
    }
}

pub(crate) fn run_audit(args: AuditArgs) -> Result<()> {
    match args.command {
        AuditCommand::Tail(args) => {
            let events = agent_finance_core::read_audit_events(args.limit)?;
            print_json_or_text(args.json, &events, || {
                events
                    .iter()
                    .map(|event| {
                        format!(
                            "{} {:?} {}",
                            event.timestamp_utc.to_rfc3339(),
                            event.kind,
                            event.summary
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        AuditCommand::Export(args) => {
            let events = agent_finance_core::read_all_audit_events()?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                for event in events {
                    println!("{}", serde_json::to_string(&event)?);
                }
            }
            Ok(())
        }
    }
}

fn ensure_live_sapi_profile(profile: &agent_finance_core::Profile, operation: &str) -> Result<()> {
    if profile.provider.environment == agent_finance_core::Environment::Live {
        return Ok(());
    }
    Err(anyhow!(
        "{operation} uses Binance SAPI live account data; use a live profile after reviewing the request"
    ))
}

fn parse_optional_decimal(
    value: Option<String>,
) -> Result<Option<agent_finance_core::DecimalValue>> {
    value.map(|value| value.parse()).transpose()
}

fn explain_profile(profile: &agent_finance_core::Profile) -> String {
    let symbols = profile
        .risk
        .allowed_symbols
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "profile: {}\nprovider: {}\nenvironment: {:?}\nallow_live: {}\nallowed_symbols: {}\nallowed_transfers: {}\nallowed_futures_state_changes: {}",
        profile.name,
        profile.provider.provider,
        profile.provider.environment,
        profile.risk.allow_live,
        symbols,
        profile
            .risk
            .allowed_transfers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        profile
            .risk
            .allowed_futures_state_changes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}
