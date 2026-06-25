# agent-finance full core skill

Read this when you need the full command map for `agent-finance`.

## Command Map

```bash
agent-finance skills list
agent-finance skills get core
agent-finance skills get price
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get crypto
agent-finance skills get prediction-markets
agent-finance skills get history-indicators
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

Use the TUI as an interactive cockpit for quote, history, crypto evidence, research/Polymarket context, provider health, task log, mouse focus, docked-column drag resize, close/restore panel controls, and an executable command palette. Use structured `market ... --json` commands when an agent needs parseable data.

## Price and Sessions

```bash
agent-finance market price CRDO
agent-finance market price CRDO MRVL --json
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

`market price` answers the default current-price question. `market sessions` compares regular/pre/post/overnight/provider/proxy sources.

## History and Indicators

```bash
agent-finance market history CRDO --range 1mo --interval 1d
agent-finance market history CRDO --range 5d --interval 1m --session extended --adjustment raw --no-actions
agent-finance market history CRDO --range 1y --interval 1d --adjustment auto --repair
agent-finance market indicators CRDO MRVL --limit 120
```

Use history before making order, fill, stop-loss, take-profit, or intraday trend judgments. Indicators are summaries; they do not replace the bar path.

## Research Data

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market fundamentals CRDO --provider robinhood
agent-finance market fundamentals CRDO --provider cnbc
agent-finance market analysis CRDO
agent-finance market options CRDO
agent-finance market options CRDO --provider robinhood --count 80
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
```

Research reports include sources, modules, coverage gaps, highlights, and raw payloads in JSON mode.

## Providers and Proxy Data

```bash
agent-finance market providers
agent-finance market providers --json
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market crypto quote BTC/USDT
agent-finance market crypto candles BTC/USDT --provider coingecko --interval 1d --limit 30
agent-finance market crypto discover --provider okx --kind instruments --instrument swap
```

Use `market providers` as the source-of-truth coverage matrix. Crypto commands are capability-first across Binance/Coinbase/OKX/CoinGecko; USD-M futures / TradFi perps are derivative/proxy prices, not legal equity or broker-fill prices.

## Prediction Markets

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket search "spcex" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
agent-finance skills get prediction-markets
```

Use Polymarket for quantifiable sentiment and event-probability signals. It does not replace SEC/IR/company releases, verified news, or equity quotes.

## Signed Profile, Risk, and Audit

```bash
agent-finance skills get profile
agent-finance profile doctor --profile default
agent-finance account permissions --profile default --json
agent-finance account balances --profile default --json
agent-finance account positions --profile default --json
agent-finance risk explain --profile default
agent-finance risk check INTENT_ID --profile default --live
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID --json
agent-finance order open --profile default --market spot --symbol BTCUSDT --json
agent-finance state create --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state create --profile default --kind margin-type --symbol BTCUSDT --margin-type isolated
agent-finance state create --profile default --kind position-mode --position-mode hedge
agent-finance state submit INTENT_ID --profile default
agent-finance audit tail --limit 20
agent-finance audit export --json
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20 --json
```

Use `profile doctor` to inspect profile `[permissions]`, risk-policy consistency, Binance API key restrictions, and provider permissions before live writes. Use `risk explain` to inspect profile limits and the local audit-backed daily order notional counter before live writes.
Signed read JSON commands return a typed `SignedReadSnapshot` envelope. The typed request scope is under `request`; provider-native data is under `payload`.

| Command | `kind` | Common payload path |
| --- | --- | --- |
| `account permissions` | `api-permissions` | `payload` |
| `account balances` | `spot-balances` | `payload.balances` |
| `account positions` | `usds-futures-positions` | `payload.assets`, `payload.positions` |
| `order query` | `order-query` | `payload` |
| `order open` | `open-orders` | `payload` |
| `transfer history` | `transfer-history` | `payload.rows` |

Signed submit JSON commands return a typed `SubmitSnapshot` envelope. Branch on `intent_kind`, `mode`, and `execution.kind`; read dry-run plans or exchange-native execution details from `execution.payload`.

Order test/live submit checks locally checkable Binance exchangeInfo filters before sending the order; market-order notional is reported as not locally checked because the exchange execution price is unknown before submit. Live market orders are blocked until risk notional can be derived from fresh exchange data instead of user-supplied `valuation_price`. Dry-run remains offline and prints the exchangeInfo request for later verification.
USD-M futures leverage, margin type, and Binance futures account position mode changes use separate `state` intents and require explicit `risk.allowed_futures_state_changes` policy before live submit. Position mode policy is not in the default profile template; add an explicit `kind = "position-mode"` entry with the intended `mode`. Position mode changes every symbol; Binance UM/CM share `dualSidePosition`, and the exchange rejects the change when either side has open orders or open positions.
Transfer history reads Binance SAPI live account data and requires a reviewed live profile.

## Network and Browser Boundaries

The CLI respects `--proxy`, `AGENT_FINANCE_PROXY`, and standard proxy environment variables. It does not hardcode a local proxy.

`market read-url` is a text extraction fallback. For dynamic, login-gated, screenshot-sensitive, or noisy pages, open the original page with an available real browser tool such as agent-browser or opencli.
