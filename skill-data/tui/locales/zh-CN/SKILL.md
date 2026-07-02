---
name: tui
description: 使用 agent-finance 终端 cockpit 进行实时 watchlist、OHLCV K 线工作台、chart preset、鼠标/键盘导航、provider 状态、账户 overlay 和 draft-only 交易工作流。
---

# agent-finance TUI skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

TUI 是实时 cockpit 和图形化探索界面，不是机器抽取面。图表只能准备 draft，任何交易写操作仍要经过 stage、review、risk、live confirmation 和 audit。

## 启动

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
agent-finance tui --symbols CRDO,LITE,AAOI --chart-preset auto
agent-finance tui --symbols BTC/USDT,ETH/USDT --chart-preset 1d
agent-finance tui --symbols CRDO --profile default
agent-finance tui --symbols CRDO --no-persist
```

## K 线工作台

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 鼠标

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 键盘

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## 交易边界

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance skills get profile --full
```

## 证据规则

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
