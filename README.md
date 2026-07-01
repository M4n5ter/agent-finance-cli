# agent-finance

[English](README.md) | [简体中文](README_ZH.md) | [日本語](README_JA.md) | [한국어](README_KO.md)

Market intelligence for AI agents that need evidence, not guesswork.

`agent-finance` is a terminal-native finance toolkit built for AI agents and automation workflows. It gives an agent a compact command surface for prices, sessions, history, indicators, crypto market structure, prediction-market sentiment, public-company research data, URL extraction, provider capability discovery, and guarded signed trading workflows.

It also includes `agent-finance tui`, a live market cockpit for watching symbols, provider health, research context, crypto evidence, and prediction-market signals in one terminal.

![agent-finance TUI market cockpit](assets/tui-market-cockpit.png)

Install once, then let agents discover the command surface from the CLI itself.

```bash
npm install -g agent-finance-cli
npx skills add https://github.com/M4n5ter/agent-finance
agent-finance skills get core
```

## What It Helps Agents Do

- Answer "what is it trading at now?" with current observable price, session, regular-market basis, and local/UTC timestamps.
- Separate regular, premarket, postmarket, and overnight signals instead of mixing them into one stale quote.
- Pull OHLCV history and local indicators before making order-quality or trend claims.
- Cross-check crypto spot, swaps, futures, order books, trades, candles, funding, open interest, and market breadth across Binance, Coinbase, OKX, and CoinGecko.
- Use Polymarket markets as quantifiable sentiment and event-probability evidence.
- Fetch no-key research payloads from Yahoo, SEC EDGAR, Robinhood, CNBC, Stooq, and fallback URL readers.
- Ask the CLI what each provider can actually do instead of guessing from provider names.
- Open a live terminal cockpit when the work is monitoring, comparing, or steering an investigation rather than extracting one parseable payload.
- Keep signed Binance account/order/transfer workflows behind profiles, intents, risk checks, explicit live confirmation, and append-only audit logs.
- Teach agents how to use the tool through built-in runtime skills that ship with the binary.

## Why This Exists

Finance research agents fail when they rely on a single quote, a search snippet, or a provider name that sounds authoritative. They need a repeatable way to collect fresh data, know which session a price came from, understand provider limits, and leave an audit trail when touching live accounts.

`agent-finance` is intentionally CLI-first because AI agents are already good at shells:

- commands are stable and scriptable;
- JSON output is available where agents need structure;
- terminal output stays readable when another agent or automation layer inspects a run;
- provider capabilities are discoverable at runtime;
- skills are embedded, so agent guidance can evolve with the installed version.

## Install

Install the CLI and the discovery skill:

```bash
npm install -g agent-finance-cli
npx skills add https://github.com/M4n5ter/agent-finance
```

The project is `agent-finance`. The npm package is published as `agent-finance-cli` because `agent-finance` is not available on npm.

The npm package installs a prebuilt binary on supported platforms:

- macOS arm64 / x64
- Linux arm64 / x64
- Windows x64

Rust is not required for the normal npm install path. If no prebuilt package is available, npm falls back to a local source build. That fallback requires Rust/Cargo plus the native toolchain needed by `wreq`/BoringSSL: CMake, Clang/Clang++, libclang, and binutils.

From GitHub:

```bash
cargo install --git https://github.com/M4n5ter/agent-finance agent-finance-cli
```

From a checkout:

```bash
cargo install --path crates/cli --locked
cargo run --bin agent-finance -- skills get core
```

## The Agent Entry Point

If you are wiring this into an AI agent, do not start by memorizing flags. Let the installed CLI describe itself:

```bash
agent-finance skills list
agent-finance skills get core --full
```

The npm package also ships a standard discovery skill at:

```text
skills/agent-finance/SKILL.md
```

That stub points agents back to the runtime skills, so command guidance stays aligned with the installed binary.

Use the installed skill as the coarse entry point, then let `agent-finance skills get ...` provide command-specific guidance from the local binary.

Useful runtime skills:

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get crypto
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get prediction-markets
agent-finance skills get profile
agent-finance skills get tui
```

## Quick Tour

Current price and session context:

```bash
agent-finance market price CRDO
agent-finance market price CRDO MRVL --json
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

History and indicators:

```bash
agent-finance market history CRDO --range 1mo --interval 1d
agent-finance market history CRDO --range 5d --interval 1m --session extended --adjustment raw --no-actions
agent-finance market indicators CRDO MRVL --limit 120
```

Crypto market structure:

```bash
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
agent-finance market crypto funding BTCUSDT --instrument swap --provider auto --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap --provider okx
agent-finance market crypto discover --provider coingecko --kind trending
```

Public-company research and discovery:

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market analysis CRDO
agent-finance market options CRDO --provider robinhood --count 80
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
```

Prediction-market sentiment:

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

Streams, polling, and URL extraction:

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
```

Provider capability discovery:

```bash
agent-finance market providers
agent-finance capabilities
```

Interactive cockpit:

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
agent-finance tui --symbols CRDO,LITE,AAOI --chart-preset auto
```

The TUI is an interactive cockpit with watchlist, quote/sessions, history, crypto evidence, research, Polymarket, provider health, task log, mouse focus, docked-column drag resize, floating-corner resize, close/restore panel controls, executable command palette, and a native OHLCV candlestick workbench. In the History panel, press `z` for the full chart, hover for O/H/L/C/V, wheel or drag to zoom, click a chart price to fill the order ticket draft, or use `j`/`k` plus `Enter` to copy a visible reference line such as current price, previous close, open, high, low, open order, or position entry. The command palette can also prepare stop-loss or take-profit ticket drafts from the selected reference line. Chart-guided trading never submits directly; it still goes through stage, review, risk, live confirmation, and audit. Use `agent-finance skills get tui` for the cockpit workflow, and keep using `market ... --json` commands when you need structured data.
It persists the watchlist, docked panel set, focused panel, column layout, floating panes, refresh cadence, and provider preferences to TOML unless `--no-persist` is used.

## Signed Trading Workflows

`agent-finance` includes guarded Binance Spot and USD-M workflows for account reads, order intents, internal transfer intents, futures state changes, risk checks, and audit logs.

The important design choice: live writes are not exposed as casual one-liners. They go through profiles, risk policy, whitelists, intent files, explicit `--live` confirmation, provider permission checks, and append-only audit events.

Start here:

```bash
agent-finance skills get profile --full
agent-finance profile template --profile default
agent-finance profile doctor --profile default
```

Command families:

```bash
agent-finance account balances --profile default
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind limit --quantity 0.001 --price 50000
agent-finance risk check INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance audit tail --limit 20
```

## Provider Notes

- `market price SYMBOL` is the default answer to "what is it trading at now?"
- `market sessions SYMBOL` is for explicit regular/pre/post/overnight/provider comparisons.
- `market history` defaults to adjusted prices and includes corporate actions unless disabled.
- `market providers` is the source-of-truth capability matrix. Do not infer coverage from provider names.
- Crypto commands are capability-first. Force `--provider binance|coinbase|okx|coingecko` only when cross-checking or auditing.
- Binance and OKX are best for exchange and derivatives microstructure. Coinbase is a spot exchange cross-check. CoinGecko is aggregate breadth, trending, and metadata.
- Binance USD-M futures and TradFi proxy symbols are derivatives/proxies, not legal equity, broker fills, or pre-IPO ownership prices.
- Polymarket is useful for implied probabilities, spread, volume, liquidity, open interest, holder preview rows, and probability history. It is not a primary-source fact feed.
- `market read-url` is an extraction fallback using direct/Jina/Defuddle readers. It is not a login-capable browser.
- Dynamic, login-gated, screenshot-sensitive, or noisy pages should still be verified with a real browser tool such as `agent-browser` or OpenCLI.

## Network And Local State

Proxy precedence:

1. `--proxy`
2. `AGENT_FINANCE_PROXY`
3. `ALL_PROXY`
4. `HTTPS_PROXY`
5. `HTTP_PROXY`

Examples:

```bash
agent-finance --proxy socks5h://127.0.0.1:7890 market price CRDO
agent-finance --no-proxy market price CRDO
```

Local profile/data roots can be overridden for tests, sandboxes, and agent workspaces:

```bash
AGENT_FINANCE_CONFIG_HOME=/tmp/agent-finance/config \
AGENT_FINANCE_DATA_HOME=/tmp/agent-finance/data \
agent-finance profile template --profile default
```

SEC EDGAR requests use `AGENT_FINANCE_SEC_USER_AGENT` when set, otherwise a project-level user agent.

Set `AGENT_FINANCE_SKILL_DATA_DIR` to test or override runtime skill documents. The npm wrapper sets `AGENT_FINANCE_PACKAGE_ROOT` automatically for prebuilt platform binaries.

## Safety

This tool is not investment advice. Market data can be delayed, incomplete, or wrong. Provider payloads can change. Social and prediction-market signals are evidence, not truth. Verify important facts against primary sources and follow source terms.
