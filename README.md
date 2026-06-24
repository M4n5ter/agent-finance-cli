# agent-finance-cli

AI Agent-first CLI for no-key financial market data and research context.

`agent-finance` is designed for human-operated AI agents: the CLI can print its own task-specific skills, then provide current quotes, regular/pre/post/overnight session splits, capability-first crypto market data, OHLCV history, local indicators, prediction-market sentiment, no-key research payloads, URL text extraction, provider capability notes, polling, and WebSocket streams.

If you are an AI Agent, start here:

```bash
agent-finance skills get core
agent-finance skills list
```

## Install

From npm:

```bash
npm install -g agent-finance-cli
```

The npm package installs a prebuilt binary for supported platforms. Rust is not required for the normal npm install path.

If no prebuilt package is available for the current platform, npm falls back to a local source build. That fallback requires Rust/Cargo plus the native toolchain needed by `wreq`/BoringSSL: CMake, Clang/Clang++, libclang, and binutils.

From GitHub:

```bash
cargo install --git https://github.com/M4n5ter/agent-finance-cli
```

From a checkout:

```bash
cargo run --bin agent-finance -- skills get core
cargo run --bin agent-finance -- market price CRDO
```

Future distribution targets include crates.io and Homebrew.

## Common Commands

```bash
# Current observable price + regular-market basis.
agent-finance market price CRDO
agent-finance market price CRDO MRVL --json

# Precise regular/pre/post/overnight/provider split.
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT

# History, indicators, crypto/proxy context, and streams.
agent-finance market history CRDO --range 1mo --interval 1d
agent-finance market history CRDO --range 5d --interval 1m --session extended --adjustment raw --no-actions
agent-finance market indicators CRDO MRVL --limit 120
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4

# Cross-provider crypto market data.
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
agent-finance market crypto funding BTCUSDT --provider okx --limit 8
agent-finance market crypto discover --provider coingecko --kind trending

agent-finance market crypto funding BTCUSDT --instrument swap --provider auto --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap --provider okx
agent-finance market crypto stream BTCUSDT --instrument swap --kind mark-price --messages 1
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --crypto-provider okx --instrument spot --interval 1h --limit 48

# No-key research and discovery.
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market analysis CRDO
agent-finance market options CRDO --provider robinhood --count 80
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
agent-finance market providers

# Prediction-market sentiment and event probabilities.
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

## Agent Skills

The npm package ships a standard discovery skill at `skills/agent-finance/SKILL.md`.
That stub points agents back to the installed CLI so command guidance does not drift.

The CLI discovers runtime skills from its packaged `skill-data/` directory:

```bash
agent-finance skills list
agent-finance skills get core --full
agent-finance skills get price
agent-finance skills get crypto
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get prediction-markets
agent-finance skills get history-indicators
```

Set `AGENT_FINANCE_SKILL_DATA_DIR` to test or override the runtime skill directory.
The npm wrapper sets `AGENT_FINANCE_PACKAGE_ROOT` automatically for prebuilt platform binaries.

## Data Source Rules

- `market price SYMBOL` is the default answer to "what is it trading at now?" It returns the current observable price, session, regular-market basis, and local/UTC timestamps.
- `market sessions SYMBOL` is for explicit regular/pre/post/overnight/provider comparisons.
- `market history` defaults to adjusted prices and includes corporate actions unless disabled.
- `market providers` is the source-of-truth capability matrix. Do not infer coverage from provider names.
- Crypto commands are capability-first: use `market crypto quote/book/trades/candles/funding/open-interest/discover`, then force `--provider binance|coinbase|okx|coingecko` only when cross-checking or auditing.
- Binance, Coinbase, OKX, and CoinGecko are tier-1 crypto no-key providers for different jobs. Binance/OKX are best for exchange and derivatives microstructure; Coinbase is a spot exchange cross-check; CoinGecko is an aggregate breadth/trending/metadata source.
- Binance Spot and crypto spot prices are crypto spot. USD-M futures / TradFi perps are derivatives and proxy instruments, not legal equity, broker fill, or pre-IPO ownership price.
- Binance Spot WebSocket uses the public market-data-only endpoint. USD-M Futures WebSocket streams are routed through Binance's current public/market paths.
- Polymarket is a prediction-market sentiment source. Use it for implied probabilities, orderbook, spread, volume, liquidity, open interest, holder preview rows, and probability history. It is not an equity quote, primary-source fact, or confirmation of insider information.
- `market read-url` is an extraction fallback using direct/Jina/Defuddle readers. It is not a login-capable browser.
- Dynamic, login-gated, screenshot-sensitive, or noisy pages should be verified with a real browser tool. `agent-browser` and `opencli` are examples, not dependencies.

## Network Defaults

`agent-finance` respects explicit and environment proxy configuration:

```bash
agent-finance --proxy socks5h://127.0.0.1:7890 market price CRDO
agent-finance --no-proxy market price CRDO
```

Proxy precedence:

1. `--proxy`
2. `AGENT_FINANCE_PROXY`
3. `ALL_PROXY`
4. `HTTPS_PROXY`
5. `HTTP_PROXY`

No proxy is hardcoded by default.

SEC EDGAR requests use `AGENT_FINANCE_SEC_USER_AGENT` when set, otherwise a project-level user agent.

Disclaimer: this tool is not investment advice; data may be delayed, incomplete, or wrong, and users must verify important facts and follow source terms.
