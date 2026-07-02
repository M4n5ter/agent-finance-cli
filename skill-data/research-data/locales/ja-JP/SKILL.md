---
name: research-data
description: no-key の Yahoo、SEC EDGAR、Robinhood、CNBC リサーチデータを取得する。fundamentals、analyst、options、ownership、events、news、search、screeners、URL 抽出を含む。
---

# agent-finance research data skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

No-key research sources は leads と cross-check 用です。official facts は SEC、IR、company releases、earnings calls を優先し、抽出が怪しいページは real browser で検証します。

## コマンド

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market fundamentals CRDO --provider robinhood
agent-finance market fundamentals CRDO --provider cnbc
agent-finance market analysis CRDO
agent-finance market options CRDO
agent-finance market options CRDO --provider robinhood --count 80
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
agent-finance market search "optical interconnect"
agent-finance market screen most_actives
```

## 出力ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## Research ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。
