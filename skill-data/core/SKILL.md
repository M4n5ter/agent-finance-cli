---
name: core
description: Entry guide for agent-finance market price, sessions, crypto, history, research data, provider coverage, prediction markets, proxy context, and safe source handling. Read this before using agent-finance commands.
---

# agent-finance core skill

This is the runtime entry guide for using `agent-finance`.

## Start

```bash
agent-finance skills list
agent-finance market providers
agent-finance capabilities
```

For an interactive human-facing cockpit, use:

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

Prefer structured `market ... --json` commands for agent data collection. The TUI is an interactive cockpit with quote, history, crypto evidence, research/Polymarket context, provider health, task log, mouse focus, docked-column drag resize, close/restore panel controls, and an executable command palette; it is not a machine extraction surface.

## Task Router

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get research-data
agent-finance skills get crypto
agent-finance skills get prediction-markets
agent-finance skills get providers
agent-finance skills get profile
```

Load a narrow skill before task-specific commands. Use `skills get core --full` when you need the extended command map.

## Default Evidence Flow

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

4. Research and source context:

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

7. Signed Binance workflows:

```bash
agent-finance skills get profile
agent-finance account permissions --profile default --json
agent-finance account balances --profile default --json
agent-finance account positions --profile default --json
agent-finance risk explain --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID --json
agent-finance order open --profile default --market spot --symbol BTCUSDT --json
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20 --json
agent-finance state create --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state create --profile default --kind position-mode --position-mode hedge
agent-finance state submit INTENT_ID --profile default
agent-finance audit export --json
```

## Decision Rules

- Use `market price` for the default "what is the current price?" answer.
- Use `market sessions` when premarket, postmarket, overnight, BOATS, provider differences, or proxy prices matter.
- Use both daily and minute history before judging fills, limit-order quality, stop placement, or intraday action.
- Use `market providers --json` for a machine-readable capability matrix.
- Use `capabilities --json` for the unified terminal surface, including account/order/transfer/futures-state safety boundaries.
- Treat crypto as 24/7 market data. Use Binance/Coinbase/OKX/CoinGecko through capability-first crypto commands, then force providers only for cross-checking.
- Spot is crypto spot; USD-M futures / TradFi perps are derivatives and proxy instruments.
- Treat Polymarket as quantifiable prediction-market sentiment and event-probability evidence only; it is not an equity quote or primary-source fact.
- `market read-url` is a text extraction fallback, not a real browser. For dynamic, login-gated, screenshot-sensitive, or noisy pages, use an available browser tool such as agent-browser or opencli.
- JSON output preserves structured fields for downstream computation. Human output is for quick inspection.
- Use `skills get profile` before touching signed account, order, transfer, futures state, risk, or audit commands.
