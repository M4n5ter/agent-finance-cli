---
name: history-indicators
description: 使用 agent-finance 获取 OHLCV 历史和本地指标，覆盖股票和 crypto 的 interval、session、复权、修复行为和指标解读规则。
---

# agent-finance market history and indicators skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

历史 K 线是交易质量判断的基础。下结论前同时看日线和分钟线，指标只能做摘要，不能替代 bar path。

## 历史数据

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market history LITE --provider auto --interval 1d --range 1mo --limit 30
agent-finance market history LITE --interval 1m --range 5d --session extended --adjustment raw --no-actions --limit 200
agent-finance market history LITE --interval 1d --range 1y --adjustment auto --repair --limit 252
agent-finance market history AAPL --provider robinhood --interval 5m --range 1d --session extended --limit 80
agent-finance market history BTC/USDT --asset crypto --crypto-provider auto --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider coinbase --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider okx --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider coingecko --interval 1d --limit 30
agent-finance market history BTCUSDT --asset crypto --crypto-provider binance --instrument swap --interval 1d --limit 30
```

## Interval

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market history --help
agent-finance market stooq sync --help
```

## 复权与修复

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 指标

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market indicators LITE AAOI --provider auto --limit 120
agent-finance market indicators CRDO MRVL --session extended --interval 1m --range 5d --limit 200
```
