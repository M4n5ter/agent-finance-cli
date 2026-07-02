---
name: core
description: agent-finance 市场价格、session、crypto、历史、研究数据、provider 覆盖、预测市场、代理上下文和安全来源处理的入口指南。使用命令前先读这个。
---

# agent-finance core skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

先读这个入口，再按任务加载更窄的 runtime skill。它把价格、session、历史、研究、crypto、Polymarket、profile 和浏览器边界串成一条可审计工作流。

## 开始

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance skills list
agent-finance market providers
agent-finance capabilities
```

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

## 任务路由

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get research-data
agent-finance skills get crypto
agent-finance skills get prediction-markets
agent-finance skills get providers
agent-finance skills get profile
agent-finance skills get tui
```

## 默认证据流

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

```bash
agent-finance market history LITE --interval 1d --range 1mo --adjustment auto --limit 30
agent-finance market history LITE --interval 1m --range 5d --session extended --adjustment raw --no-actions --limit 120
```

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

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance skills get prediction-markets
```

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --provider okx --limit 20
agent-finance market crypto discover --provider coingecko --kind trending
```

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

## 决策规则

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
