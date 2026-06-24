---
name: prediction-markets
description: Use Polymarket prediction-market data as quantifiable sentiment and event-probability evidence, including market search, market details, liquidity, orderbook, holder previews, and probability history.
---

# agent-finance prediction-markets skill

Use this skill when an AI Agent needs prediction-market sentiment, event probabilities, or "what capital is pricing in" for a public event.

## Commands

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket search "spcex" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

## Search Semantics

- Treat Polymarket search as public relevance search. Do not describe it as guaranteed fuzzy search.
- Use multiple query fallbacks for important topics:
  - `spacex`, `space x`, `starship`, `ipo`
  - `nvidia`, `nvda`, product names, regulatory events
- The CLI locally filters and sorts by active/closed state, volume, liquidity, and market signal strength. Still inspect source URLs and raw JSON for important decisions.

## Interpretation Rules

- Polymarket prices are implied probabilities backed by user capital.
- They are useful as quantifiable sentiment and event-probability evidence.
- They are not facts, confirmed insider information, legal equity prices, broker-fill prices, or official company disclosures.
- For investment research, pair this signal with primary sources: SEC filings, IR pages, company releases, earnings calls, and verifiable news.

## Useful Flags

```bash
agent-finance market polymarket search "spacex ipo" --include-closed --min-volume 1000 --json
agent-finance market polymarket market MARKET_ID_OR_SLUG --limit 20 --refresh
```

- `--include-closed`: include resolved/closed markets for historical expectation checks.
- `--min-volume`: ignore thin markets.
- `--refresh`: bypass local cache.
- `--cache-ttl-seconds`: tune freshness for repeated agent workflows.
- `--json`: preserve full structured payloads for downstream reasoning.

## Boundaries

- This CLI is read-only for Polymarket.
- It does not accept private keys, derive API keys, place orders, cancel orders, or manage Polymarket positions.
- Default transport uses the official SDK. Explicit `--proxy` or `--no-proxy` uses public REST fallback through the CLI HTTP stack so those network controls are honored.
- Holder data is reported as preview rows returned by the API limit, not as a total holder count.
