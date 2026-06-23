---
name: profile
description: Configure agent-finance trading profiles, Binance HMAC env references, risk policy, intent-first live writes, audit logs, and safe AI Agent workflows.
---

# agent-finance profile skill

Use this skill before any `account`, `order`, `transfer`, `risk`, or `audit` command.
Also use it before USD-M futures `state` changes.

## Model

- A profile is a TOML file in the user config directory.
- The profile stores environment variable names for Binance HMAC keys, not secrets.
- The default HMAC secret env is `BINANCE_PRIVATE_KEY`; in Binance HMAC mode this is the API Secret string, not an RSA or Ed25519 private key.
- Live writes require all of these: profile `allow_live = true`, the relevant order/transfer/futures-state whitelist, intent id, and `--live`.
- Live market orders are blocked until risk notional can be derived from fresh exchange data instead of user-supplied `valuation_price`.
- USD-M futures leverage, margin type, and Binance futures account position mode changes require explicit `risk.allowed_futures_state_changes` policy and use separate `state` intents.
- Binance position mode changes every symbol; UM/CM share `dualSidePosition`, and Binance rejects the change when either side has open orders or open positions.
- Order, cancel, transfer, and futures state writes are intent-first. Create the intent, inspect it, run `risk check`, then submit.
- Audit logging is append-only JSONL in the user data directory.

## Setup

```bash
agent-finance profile path --profile default
agent-finance profile template --profile default
agent-finance profile doctor --profile default
agent-finance profile explain --profile default
agent-finance risk explain --profile default
```

## Order Flow

```bash
agent-finance order intent BTCUSDT --profile default --market spot --side buy --kind limit --quantity 0.001 --price 50000 --time-in-force gtc
agent-finance order intent BTCUSDT --profile default --market spot --side buy --kind limit-maker --quantity 0.001 --price 50000
agent-finance order intent BTCUSDT --profile default --market spot --side buy --kind market --quantity 0.001 --valuation-price 50000
agent-finance risk check INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default --test
agent-finance order submit INTENT_ID --profile default --live
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID
agent-finance order cancel-intent BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID
```

## Futures State Flow

Add Binance futures account position mode policy manually before using `--kind position-mode`; it is not included in the default profile template:

```toml
[[risk.allowed_futures_state_changes]]
kind = "position-mode"
mode = "hedge"
```

```bash
agent-finance state intent --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state intent --profile default --kind margin-type --symbol BTCUSDT --margin-type isolated
agent-finance state intent --profile default --kind position-mode --position-mode hedge
agent-finance risk check INTENT_ID --profile default --live
agent-finance state submit INTENT_ID --profile default
agent-finance state submit INTENT_ID --profile default --live
```

## Transfer Flow

```bash
agent-finance transfer intent USDT --profile default --direction spot-to-usds-futures --amount 10
agent-finance risk check INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default --live
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20
```

## Audit Flow

```bash
agent-finance audit tail --limit 20
agent-finance audit export --json
```

## Gated Smoke Tests

These checks are for maintainers and AI Agents validating signed Binance workflows.
They require exported `BINANCE_API_KEY` and `BINANCE_PRIVATE_KEY`; the test profiles store only env var names.

Live read-only signed smoke, no orders or transfers:

```bash
AGENT_FINANCE_LIVE_BINANCE_SIGNED=1 cargo test --test binance_live binance_live_signed_read_only_surface_is_usable -- --ignored --exact --nocapture
```

Testnet signed order-test smoke, uses Binance test endpoints and does not place a live order:

```bash
AGENT_FINANCE_TESTNET_BINANCE_SIGNED=1 cargo test --test binance_live binance_testnet_signed_order_test_surface_is_usable -- --ignored --exact --nocapture
```

Live place-and-cancel smoke places a real Binance spot `LIMIT_MAKER` post-only order and then cancels it.
The test also reads the live Binance order book and fails before submit unless buy price is below best bid or sell price is above best ask.
Use only with a deliberately non-marketable tiny order that still satisfies Binance min-notional filters; do not fix min-notional failures by making the price marketable.
Set `AGENT_FINANCE_LIVE_BINANCE_SMOKE_DATA_HOME` to a persistent local directory so audit events and daily live notional limits survive between smoke runs.

```bash
AGENT_FINANCE_LIVE_BINANCE_PLACE_AND_CANCEL_ORDER=1 \
AGENT_FINANCE_LIVE_BINANCE_WRITE_ACK=I_UNDERSTAND_THIS_PLACES_A_LIVE_ORDER \
AGENT_FINANCE_LIVE_BINANCE_SMOKE_DATA_HOME=$HOME/.local/state/agent-finance-live-smoke \
AGENT_FINANCE_LIVE_BINANCE_ORDER_SYMBOL=BTCUSDT \
AGENT_FINANCE_LIVE_BINANCE_ORDER_MARKET=spot \
AGENT_FINANCE_LIVE_BINANCE_ORDER_SIDE=buy \
AGENT_FINANCE_LIVE_BINANCE_ORDER_QUANTITY=0.0004 \
AGENT_FINANCE_LIVE_BINANCE_ORDER_PRICE=30000 \
AGENT_FINANCE_LIVE_BINANCE_ORDER_MAX_NOTIONAL_USDT=15 \
cargo test --test binance_live_write binance_live_order_cancel_smoke_is_usable -- --ignored --exact --nocapture
```

Live transfer smoke moves real funds between Spot and USD-M. Run it only with a tiny amount and an explicit direction:

```bash
AGENT_FINANCE_LIVE_BINANCE_TRANSFERS=1 \
AGENT_FINANCE_LIVE_BINANCE_TRANSFER_ACK=I_UNDERSTAND_THIS_MOVES_FUNDS \
AGENT_FINANCE_LIVE_BINANCE_SMOKE_DATA_HOME=$HOME/.local/state/agent-finance-live-smoke \
AGENT_FINANCE_LIVE_BINANCE_TRANSFER_ASSET=USDT \
AGENT_FINANCE_LIVE_BINANCE_TRANSFER_DIRECTION=spot-to-usds-futures \
AGENT_FINANCE_LIVE_BINANCE_TRANSFER_AMOUNT=0.1 \
AGENT_FINANCE_LIVE_BINANCE_TRANSFER_MAX_AMOUNT=0.1 \
cargo test --test binance_live_write binance_live_transfer_smoke_is_usable -- --ignored --exact --nocapture
```

## Guardrails

- Never put API secrets in TOML, Markdown, command history, audit logs, or prompts.
- Use Binance testnet profiles first.
- For live profiles, keep whitelist and notional limits small.
- `max_daily_order_notional_usdt` is enforced from the local append-only audit log for `risk check --live` and live order submit. Matching live-submit events with missing notional data fail closed.
- `order submit` without flags is an offline dry-run; `--test` calls an exchange test endpoint where available but does not consume the intent; only `--live` consumes the intent.
- `order submit --test` and `order submit --live` fetch Binance `exchangeInfo` and block orders that violate locally checkable symbol status, price tick, lot size, or notional filters. Dry-run is offline and prints the `exchangeInfo` request that will be checked later.
- Limit orders use `--price` as the exchange price. Spot `limit-maker` orders map to Binance `LIMIT_MAKER`, do not accept `--time-in-force`, and rely on the exchange to reject orders that would immediately take liquidity. Market orders use `--valuation-price` for risk notional checks and never send an exchange `price` parameter; exchange notional for market orders is reported as not locally checked because it depends on execution price.
- Live universal transfers require explicit `[[risk.allowed_transfers]]` entries with direction, asset, and max amount.
- Live futures state changes require explicit `[[risk.allowed_futures_state_changes]]` entries. Order submit does not change leverage, margin type, or position mode implicitly.
- Review `risk check` findings before live position-mode submit; the CLI warns that Binance applies it account-wide across every symbol and that UM/CM share the setting.
- Transfer history reads Binance SAPI live account data and requires a reviewed live profile.
- Do not use this CLI for withdrawals, margin, COIN-M, options, earn, or external transfers.
