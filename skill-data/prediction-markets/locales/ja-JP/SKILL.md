---
name: prediction-markets
description: Polymarket の予測市場データを、定量化できるセンチメントとイベント確率の証拠として使うためのガイド。
---

# agent-finance prediction-markets skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

Polymarket price は資金で裏付けられた implied probability です。sentiment と event probability の証拠であり、fact、insider information、equity quote ではありません。

## コマンド

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket search "spcex" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

## 検索セマンティクス

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## 解釈ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## 便利な flag

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market polymarket search "spacex ipo" --include-closed --min-volume 1000 --json
agent-finance market polymarket market MARKET_ID_OR_SLUG --limit 20 --refresh
```

## 境界

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。
