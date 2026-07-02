---
name: crypto
description: 使用 capability-first crypto 市场数据，覆盖 Binance、Coinbase、OKX、CoinGecko 的现货、合约、报价、盘口、成交、K 线、资金费率、持仓量和情绪。
---

# agent-finance market crypto skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

Crypto 是 24/7 市场。默认按 capability 选 provider；Binance/OKX 更适合微观结构和衍生品，Coinbase 做现货交叉验证，CoinGecko 做广度、趋势和元数据。

## 开始

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
```

## 跨 provider 证据

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market crypto quote BTC/USDT
agent-finance market crypto quote BTC-USD --provider coinbase
agent-finance market crypto book BTC/USDT --provider okx --limit 20
agent-finance market crypto trades BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --provider coingecko --interval 1d --limit 30
agent-finance market crypto funding BTCUSDT --provider auto --instrument swap --limit 8
agent-finance market crypto open-interest BTCUSDT --provider okx --instrument swap
agent-finance market crypto discover --provider coingecko --kind trending
agent-finance market crypto discover --provider coingecko --kind global
agent-finance market crypto discover --provider okx --kind instruments --instrument swap
agent-finance market crypto discover --provider coinbase --kind volume-summary
```

## 工具类型

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market crypto quote BTC/USDT --instrument spot
agent-finance market crypto book BTC/USDT --instrument spot --limit 20
agent-finance market crypto candles BTC/USDT --instrument spot --interval 1m --limit 60
agent-finance market crypto funding BTCUSDT --instrument swap --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap
agent-finance market crypto stream BTCUSDT --kind trade --messages 1
agent-finance market crypto stream BTCUSDT --instrument swap --kind mark-price --messages 1
```

## 规则

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
