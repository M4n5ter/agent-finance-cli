---
name: prediction-markets
description: 把 Polymarket 预测市场数据作为可量化舆情和事件概率证据，覆盖搜索、详情、流动性、orderbook、holder preview 和概率历史。
---

# agent-finance prediction-markets skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

Polymarket 价格是资金押注出来的隐含概率，只能作为舆情和事件概率证据，不能当成事实、内幕或股票报价。

## 命令

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket search "spcex" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

## 搜索语义

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 解读规则

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 常用 flag

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market polymarket search "spacex ipo" --include-closed --min-volume 1000 --json
agent-finance market polymarket market MARKET_ID_OR_SLUG --limit 20 --refresh
```

## 边界

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
