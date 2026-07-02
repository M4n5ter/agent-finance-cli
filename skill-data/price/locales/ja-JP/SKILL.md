---
name: price
description: 現在価格、regular-market basis、premarket、postmarket、overnight、crypto 価格、proxy symbol、stream、watch 出力を取得する。
---

# agent-finance market price skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

`market price` は「今いくらか」への標準回答です。pre/post/overnight、provider disagreement、proxy price が重要な場合は `market sessions` を使います。

## デフォルト価格

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

## Session 分解

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

## Crypto と proxy context

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market price BTC/USDT --asset crypto
agent-finance market price BTCUSDT --asset crypto --instrument spot
agent-finance market price BTCUSDT --asset crypto --instrument swap
```

```bash
agent-finance market sessions SPCX --proxy-symbol SPCXUSDT
```

## Stream と watch

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
```
