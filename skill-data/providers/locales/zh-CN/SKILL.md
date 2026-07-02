---
name: providers
description: 理解 agent-finance provider 能力，覆盖 Yahoo、SEC EDGAR、CNBC、Robinhood、Stooq、Binance、Coinbase、OKX、CoinGecko、Polymarket 和 URL fallback reader。
---

# agent-finance market providers skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

不要从 provider 名字猜能力；先查 capability matrix，再选择自动路由或强制 provider。CLI 不是浏览器，动态页面要交给真实浏览器工具。

## 能力矩阵

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market providers
agent-finance market providers --json
```

## Provider 规则

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 浏览器边界

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
