---
name: providers
description: Understand agent-finance provider capabilities across Yahoo, SEC EDGAR, CNBC, Robinhood, Stooq, Binance, Coinbase, OKX, CoinGecko, Polymarket, and fallback URL readers.
---

# agent-finance market providers skill

## Capability Matrix

Always inspect provider coverage instead of guessing from provider names:

```bash
agent-finance market providers
agent-finance market providers --json
```

## Provider Rules

- Quotes: use `market price SYMBOL` first. Only force a provider when cross-checking.
- Session split: use `market sessions SYMBOL`.
- History: use `market history --provider auto|yahoo|stooq|robinhood` for equities; use `market history --asset crypto --crypto-provider auto|binance|coinbase|okx|coingecko` for crypto.
- Research: `market fundamentals/events --provider auto` combines useful no-key sources when available.
- SEC EDGAR is official for filings and XBRL facts, not market quotes, options, analyst estimates, or news aggregation.
- Robinhood and CNBC are partial no-key sources; use them as cross-checks, not replacements for official filings or primary disclosures.
- Stooq live can provide no-key daily/weekly/monthly history; intraday bulk data requires explicit imported ZIP cache.
- Crypto: use capability-first commands such as `market crypto quote/book/trades/candles/funding/open-interest/discover`, then force `--provider binance|coinbase|okx|coingecko` only when cross-checking or auditing.
- Binance Spot and USD-M Futures are tier-1 crypto market-data providers. Use `agent-finance skills get crypto`.
- Binance uses local clients against official public REST/WebSocket paths, not the generated Binance SDK.
- Binance USD-M futures / TradFi perps are derivative instruments and proxy price-discovery sources, not legal equity.
- Coinbase is a spot exchange cross-check for products, tickers, stats, books, trades, candles, and volume summary.
- OKX is a spot/derivatives exchange cross-check for instruments, tickers, books, trades, candles, funding, mark price, and open interest.
- CoinGecko is an aggregate crypto source for simple price, coin metadata, markets, tickers, OHLC, market charts, trending, global, exchanges, and derivatives discovery.
- Polymarket is a prediction-market sentiment source. Use `market polymarket search` and `market polymarket market` for implied probability, orderbook, liquidity, OI, holder preview rows, and probability history; do not use it as an equity quote or primary-source fact.

## Browser Boundary

The CLI uses HTTP requests with browser-like TLS behavior where possible, but it is not a full browser. Dynamic, login-gated, screenshot-sensitive, or noisy pages require a real browser tool. Polymarket uses the official SDK by default; explicit `--proxy` or `--no-proxy` uses public REST fallback through the CLI HTTP stack.
