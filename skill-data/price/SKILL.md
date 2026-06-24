---
name: price
description: Fetch current price summaries, regular-market basis, premarket, postmarket, overnight sessions, crypto prices, proxy symbols, streams, and watch output with agent-finance.
---

# agent-finance market price skill

## Default Price

Use `market price` to answer "what is it trading at now?":

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

The default output includes current observable price, session, provider, local timestamp, UTC fields in JSON, change from regular-market previous close, and regular-market open/high/low/volume when available.

## Session Split

Use `market sessions` when the task asks about premarket, postmarket, overnight, BOATS, platform 24h prices, or provider disagreement:

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

## Crypto And Proxy Context

Use the crypto market domain for actual crypto symbols:

```bash
agent-finance market price BTC/USDT --asset crypto
agent-finance market price BTCUSDT --asset crypto --instrument spot
agent-finance market price BTCUSDT --asset crypto --instrument swap
```

If an equity or pre-IPO name has a relevant 24/7 derivative or proxy contract, add it only as side context:

```bash
agent-finance market sessions SPCX --proxy-symbol SPCXUSDT
```

Proxy symbols are price-discovery and sentiment signals. They are not the legal equity, pre-IPO ownership, or broker-fill price.

## Streaming

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
```

Use `watch` when WebSocket streaming is blocked by the local network.
