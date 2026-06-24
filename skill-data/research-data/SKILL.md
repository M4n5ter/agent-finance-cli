---
name: research-data
description: Fetch no-key Yahoo, SEC EDGAR, Robinhood, and CNBC research data, including fundamentals, analyst data, options, ownership, events, news, search, screeners, and URL text extraction.
---

# agent-finance research data skill

## Commands

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
agent-finance market screen most_actives
```

## Output Rules

- Human mode prints a compact table.
- `--json` preserves sources, modules, coverage gaps, highlights, and raw payloads.
- `--raw` prints raw payloads in human mode.
- `--refresh` skips cache.
- `--cache-ttl-seconds <N>` changes non-price cache TTL.
- `market read-url --provider auto` tries direct/Jina/Defuddle readers and reports fallback errors.

## Research Rules

- Treat social media, search snippets, and extracted web text as leads until confirmed by primary sources.
- Use SEC/company filings for official facts when available.
- Use a real browser for dynamic, login-gated, table-layout-sensitive, or extraction-suspicious pages.
