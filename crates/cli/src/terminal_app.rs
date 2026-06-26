use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use serde_json::json;

use crate::cli::{
    AccountArgs, AccountCommand, AuditArgs, AuditCommand, CapabilitiesArgs, OrderArgs,
    OrderCommand, ProfileArgs, ProfileCommand, RiskArgs, RiskCommand, TransferArgs,
    TransferCommand,
};
use crate::terminal_output::{
    print_json_or_text, print_submit_report, risk_findings_text, submit_mode_from_flags,
};
use agent_finance_trading::TradingRuntime;

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
    let runtime = TradingRuntime::new(timeout_seconds);
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
            let mut checks = vec![agent_finance_core::DiagnosticCheck::optional(
                "profile-parse",
                true,
                "profile TOML parsed successfully",
            )];
            let key_ok = std::env::var(&profile.provider.api_key_env).is_ok();
            let secret_ok = std::env::var(&profile.provider.api_secret_env).is_ok();
            checks.push(agent_finance_core::DiagnosticCheck::required(
                "api-key-env",
                key_ok,
                format!(
                    "{} {}",
                    profile.provider.api_key_env,
                    if key_ok { "is set" } else { "is missing" }
                ),
            ));
            checks.push(agent_finance_core::DiagnosticCheck::required(
                "api-secret-env",
                secret_ok,
                format!(
                    "{} {}",
                    profile.provider.api_secret_env,
                    if secret_ok { "is set" } else { "is missing" }
                ),
            ));
            checks.extend(agent_finance_core::check_profile_permission_policy(
                &profile,
            ));
            if key_ok && secret_ok {
                match runtime.account_permissions(&profile).await {
                    Ok(payload) => {
                        let permission_checks =
                            agent_finance_binance::profile_permission_checks(&profile, &payload);
                        checks.push(
                            agent_finance_core::DiagnosticCheck::required(
                                "binance-permissions",
                                true,
                                "Binance API key permission endpoint succeeded",
                            )
                            .with_payload(payload),
                        );
                        checks.extend(permission_checks);
                    }
                    Err(error) => checks.push(agent_finance_core::DiagnosticCheck::required(
                        "binance-permissions",
                        false,
                        format!("{error:#}"),
                    )),
                }
            }
            let report = ProfileDoctorReport {
                profile: args.profile,
                checks,
            };
            print_json_or_text(args.json, &report, || {
                report
                    .checks
                    .iter()
                    .map(|check| {
                        format!(
                            "{}: {} - {}",
                            if check.ok { "ok" } else { "fail" },
                            check.name,
                            check.message
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
    }
}

#[derive(Debug, Serialize)]
struct ProfileDoctorReport {
    profile: String,
    checks: Vec<agent_finance_core::DiagnosticCheck>,
}

pub(crate) async fn run_account(args: AccountArgs, timeout_seconds: u64) -> Result<()> {
    let runtime = TradingRuntime::new(timeout_seconds);
    match args.command {
        AccountCommand::Permissions(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let request = agent_finance_core::SignedReadRequest::ApiPermissions;
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
        AccountCommand::Balances(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let request = agent_finance_core::SignedReadRequest::SpotBalances;
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
        AccountCommand::Positions(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let request = agent_finance_core::SignedReadRequest::UsdsFuturesPositions;
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
    }
}

fn print_signed_read_snapshot(
    json_output: bool,
    snapshot: &agent_finance_core::SignedReadSnapshot,
) -> Result<()> {
    print_json_or_text(json_output, snapshot, || {
        format!(
            "profile: {}\nprovider: {}\nenvironment: {}\nkind: {}\npayload:\n{}",
            snapshot.profile,
            snapshot.provider,
            snapshot.environment,
            snapshot.kind,
            serde_json::to_string_pretty(&snapshot.payload).unwrap()
        )
    })
}

pub(crate) async fn run_order(args: OrderArgs, timeout_seconds: u64) -> Result<()> {
    let runtime = TradingRuntime::new(timeout_seconds);
    match args.command {
        OrderCommand::Create(args) => {
            let profile = runtime.load_profile(&args.profile)?;
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
            let path = runtime.save_intent_with_audit(
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
        OrderCommand::Cancel(args) => {
            let profile = runtime.load_profile(&args.profile)?;
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
            let path = runtime.save_intent_with_audit(
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
            let profile = runtime.load_profile(&args.profile)?;
            let mode = submit_mode_from_flags(args.live, args.test)?;
            let report = runtime
                .submit_order_intent(&profile, &args.intent_id, mode)
                .await?;
            print_submit_report(args.json, &report)
        }
        OrderCommand::Query(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let target =
                agent_finance_core::OrderIdentifier::new(args.order_id, args.client_order_id)?;
            let request = agent_finance_core::SignedReadRequest::OrderQuery {
                market: args.market.into(),
                symbol: args.symbol.to_ascii_uppercase(),
                target,
            };
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
        OrderCommand::Open(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let request = agent_finance_core::SignedReadRequest::OpenOrders {
                market: args.market.into(),
                symbol: args.symbol.map(|symbol| symbol.to_ascii_uppercase()),
            };
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
    }
}

pub(crate) async fn run_transfer(args: TransferArgs, timeout_seconds: u64) -> Result<()> {
    let runtime = TradingRuntime::new(timeout_seconds);
    match args.command {
        TransferCommand::Create(args) => {
            let profile = runtime.load_profile(&args.profile)?;
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
            let path = runtime.save_intent_with_audit(
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
            let profile = runtime.load_profile(&args.profile)?;
            let mode = submit_mode_from_flags(args.live, false)?;
            let report = runtime
                .submit_transfer_intent(&profile, &args.intent_id, mode)
                .await?;
            print_submit_report(args.json, &report)
        }
        TransferCommand::History(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let request = agent_finance_core::SignedReadRequest::transfer_history(
                args.direction.into(),
                args.current,
                args.size,
            );
            let snapshot = runtime.run_signed_read(&profile, request).await?;
            print_signed_read_snapshot(args.json, &snapshot)
        }
    }
}

pub(crate) fn run_risk(args: RiskArgs) -> Result<()> {
    let runtime = TradingRuntime::new(0);
    match args.command {
        RiskCommand::Check(args) => {
            let profile = runtime.load_profile(&args.profile)?;
            let envelope =
                agent_finance_core::IntentStore::from_default_dir()?.load(&args.intent_id)?;
            let risk = match &envelope.kind {
                agent_finance_core::IntentKind::Order(intent) => {
                    runtime.check_order_with_runtime_limits(&profile, intent, args.live)?
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
            let profile = runtime.load_profile(&args.profile)?;
            let used = agent_finance_core::daily_live_order_notional_used_today(&profile)?;
            let report = json!({
                "profile": profile.name,
                "provider": profile.provider.provider,
                "environment": profile.provider.environment,
                "permissions": profile.permissions,
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
        "profile: {}\nprovider: {}\nenvironment: {:?}\npermissions: spot_trading={}, usds_futures={}, universal_transfer={}\nallow_live: {}\nallowed_symbols: {}\nallowed_transfers: {}\nallowed_futures_state_changes: {}",
        profile.name,
        profile.provider.provider,
        profile.provider.environment,
        profile.permissions.spot_trading,
        profile.permissions.usds_futures,
        profile.permissions.universal_transfer,
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
