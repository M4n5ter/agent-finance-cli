---
name: agent-finance
description: AI Agent-first CLI for no-key financial market data and research context. Use when Codex or another AI agent needs current quotes, regular/pre/post/overnight session splits, crypto market data, OHLCV history, indicators, prediction-market sentiment, provider capability discovery, no-key research payloads, URL text extraction, polling, or WebSocket streams.
hidden: true
---

# agent-finance

`agent-finance` is a finance and market-data CLI built for human-operated AI agents.

This file is a discovery stub, not the full usage guide. Before running data-collection commands, load the installed version's runtime skill content:

```bash
agent-finance skills get core
agent-finance skills get core --full
agent-finance skills list
```

The CLI serves skill content from its packaged `skill-data` directory, so the instructions match the installed binary.

## Specialized Skills

Load a narrower skill when the task is specific:

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get crypto
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get prediction-markets
agent-finance skills get profile
```

## Boundaries

- Use `market price` for the default current observable price.
- Use `market sessions` when regular, premarket, postmarket, overnight, provider differences, or proxy prices matter.
- Inspect daily and minute history before trading, order-quality, stop-loss, or take-profit conclusions.
- Treat crypto and prediction-market data as market evidence, not primary company facts.
- Load `skills get profile` before signed account, order, transfer, futures state, risk, or audit workflows.
- Use a real browser tool for login-gated, dynamic, screenshot-sensitive, X/Reddit, brokerage, or extraction-suspicious pages.
