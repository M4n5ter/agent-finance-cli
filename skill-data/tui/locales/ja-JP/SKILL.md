---
name: tui
description: agent-finance の端末 cockpit で live watchlist、OHLCV chart workbench、chart presets、mouse/keyboard navigation、provider status、account overlays、draft-only trading workflow を扱う。
---

# agent-finance TUI skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

TUI は live cockpit と visual exploration surface です。machine extraction ではありません。chart は draft を作るだけで、write は stage、review、risk、live confirmation、audit を通します。

## 起動

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
agent-finance tui --symbols CRDO,LITE,AAOI --chart-preset auto
agent-finance tui --symbols BTC/USDT,ETH/USDT --chart-preset 1d
agent-finance tui --symbols CRDO --profile default
agent-finance tui --symbols CRDO --no-persist
```

## Chart workbench

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## Mouse

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## Keyboard

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## Trading boundary

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance skills get profile --full
```

## Evidence ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。
