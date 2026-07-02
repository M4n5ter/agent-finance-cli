---
name: price
description: 用 agent-finance 获取当前价格摘要、regular-market basis、premarket、postmarket、overnight、crypto 价格、代理 symbol、stream 和 watch 输出。
---

# agent-finance market price skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

默认用 `market price` 回答“现在多少钱”；需要 pre/post/overnight、provider 分歧或代理价时再用 `market sessions`。

## 默认价格

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

## Session 拆分

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

## Crypto 与代理上下文

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market price BTC/USDT --asset crypto
agent-finance market price BTCUSDT --asset crypto --instrument spot
agent-finance market price BTCUSDT --asset crypto --instrument swap
```

```bash
agent-finance market sessions SPCX --proxy-symbol SPCXUSDT
```

## 流式与 watch

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
```
