---
name: core
description: Entry guide for agent-finance market price, sessions, crypto, history, research data, provider coverage, prediction markets, proxy context, and safe source handling. Read this before using agent-finance commands.
---

# agent-finance core skill

This skill is printed by the `agent-finance` CLI. It is the first thing an AI Agent should read before using the tool.

## Start Here

```bash
agent-finance skills list
agent-finance skills get core --full
agent-finance market providers
agent-finance capabilities
agent-finance skills get crypto
agent-finance skills get profile
```

## Default Workflow

1. Current observable price:

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

2. Precise session/provider split:

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

3. History before a trading or order-quality conclusion:

```bash
agent-finance market history LITE --interval 1d --range 1mo --adjustment auto --limit 30
agent-finance market history LITE --interval 1m --range 5d --session extended --adjustment raw --no-actions --limit 120
```

4. Research data:

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market analysis CRDO
agent-finance market options CRDO
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
```

5. Prediction-market sentiment:

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance skills get prediction-markets
```

6. Crypto market data:

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --provider okx --limit 20
agent-finance market crypto discover --provider coingecko --kind trending
```

7. Signed trading profile and audit workflows:

```bash
agent-finance skills get profile
agent-finance account permissions --profile default --json
agent-finance account balances --profile default --json
agent-finance account positions --profile default --json
agent-finance risk explain --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID
agent-finance state create --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state create --profile default --kind position-mode --position-mode hedge
agent-finance state submit INTENT_ID --profile default
agent-finance audit export --json
```

## Rules

- Use `market price` for the default "what is the current price?" answer.
- Use `market sessions` when premarket, postmarket, overnight, BOATS, provider differences, or proxy prices matter.
- Use both daily and minute history before judging fills, limit-order quality, stop placement, or intraday action.
- Use `market providers --json` when an Agent needs a machine-readable capability matrix.
- Use `capabilities --json` for the unified terminal surface, including account/order/transfer/futures-state safety boundaries.
- Use `skills get profile` before touching signed account, order, transfer, futures state, risk, or audit commands.
- Signed `account` commands return a typed snapshot envelope with `profile`, `provider`, `environment`, `kind`, and raw provider data under `payload`.
- Account snapshot kinds are command discriminators: `account permissions` -> `api-permissions` with data under `payload`; `account balances` -> `spot-balances` with balances under `payload.balances`; `account positions` -> `usds-futures-positions` with futures account data under `payload.assets` and `payload.positions`.
- Run `profile doctor` before live writes; it checks `[permissions]` against the risk policy, reports Binance API permission checks when HMAC env vars are set, and live submit rechecks exchange permissions before claiming the intent.
- Signed order test/live submit checks locally checkable Binance exchangeInfo filters before sending an order; dry-run remains offline.
- Live market orders are blocked until risk notional can be derived from fresh exchange data instead of user-supplied `valuation_price`.
- USD-M futures leverage, margin type, and Binance futures account position mode changes use separate `state` intents; order submit never changes account state implicitly.
- Position mode changes every symbol; Binance UM/CM share `dualSidePosition`, and the exchange rejects the change when either side has open orders or open positions.
- Position mode policy is not in the default profile template; add an explicit `[[risk.allowed_futures_state_changes]]` entry with `kind = "position-mode"` and the intended `mode` before creating that intent.
- Treat crypto as 24/7 market data. Use Binance/Coinbase/OKX/CoinGecko through capability-first crypto commands, then force providers only for cross-checking.
- Spot is crypto spot; USD-M futures / TradFi perps are derivatives and proxy instruments.
- Treat Polymarket as quantifiable prediction-market sentiment and event-probability evidence only; it is not an equity quote or primary-source fact.
- `market read-url` is a text extraction fallback, not a real browser. For dynamic, login-gated, screenshot-sensitive, or noisy pages, use an available browser tool such as agent-browser or opencli.
- JSON output preserves structured fields for downstream computation. Human output is for quick inspection.
