---
name: tui
description: Use the agent-finance terminal cockpit for live watchlists, OHLCV candlestick workbench, chart presets, mouse/keyboard navigation, provider status, account overlays, and draft-only trading workflows.
---

# agent-finance TUI skill

Use the TUI when the task is live monitoring, visual comparison, chart-guided investigation, or cautious trade preparation. Use structured `market ... --json` commands when another agent needs parseable data.

## Launch

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
agent-finance tui --symbols CRDO,LITE,AAOI --chart-preset auto
agent-finance tui --symbols BTC/USDT,ETH/USDT --chart-preset 1d
agent-finance tui --symbols CRDO --profile default
agent-finance tui --symbols CRDO --no-persist
```

The TUI persists the watchlist, focused panel, docked panel set, column layout, floating panes, refresh cadence, chart preset, and provider preferences to TOML unless `--no-persist` is used.

## Chart Workbench

- Focus the History panel and press `z` to enter or leave the full chart workbench.
- The chart uses OHLCV candles when the provider returns open/high/low/close data.
- If only close prices are available, treat the chart as degraded and verify the provider/session before drawing trading conclusions.
- The workbench shows volume, MA20, MA50, VWAP, current price, previous close, day open, day high, day low, open-order lines, and position entry lines when the local snapshots contain them.
- The title and warnings are part of the evidence: read provider, session, interval, range, fetched time, and fallback notes before acting.

## Mouse

- Hover a candle column for crosshair and O/H/L/C/V tooltip.
- Wheel inside the History chart to zoom the time window.
- Drag across the chart to zoom into the selected window.
- Click a chart price to fill the order ticket reference price. This only edits a draft; it must still pass stage, review, risk, and live confirmation before any exchange write.
- Use mouse focus, panel close/restore controls, docked-column drag resize, and floating-corner resize for layout work.

## Keyboard

- `h` / `l` or left / right moves the chart cursor.
- `[` / `]` zooms the chart window.
- Number preset actions switch chart ranges.
- `r` refreshes history for the selected symbol.
- Open the command palette to search chart actions such as preset changes, chart refresh, reset zoom, toggle overlays, and copy price to ticket.

## Trading Boundary

The chart can prepare an order ticket but must not submit anything by itself. For signed workflows, load:

```bash
agent-finance skills get profile --full
```

Then use the profile, intent, risk, confirmation, and audit path. Live writes are gated separately from chart exploration.

## Evidence Rules

- For trading, stop-loss, take-profit, or limit-order decisions, inspect both daily and minute history.
- Do not mix chart providers silently. If a provider falls back or maps a preset to another interval/range, record the actual provider and request shape.
- Treat account/order/position overlays as local snapshot evidence. If the account snapshot is stale, refresh it before relying on the overlay.
- Use `market price`, `market sessions`, and `market history --json` to capture structured evidence after visual TUI exploration.
