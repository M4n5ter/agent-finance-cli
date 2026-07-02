---
name: core
description: agent-finance の市場価格、セッション、crypto、履歴、リサーチデータ、provider カバレッジ、予測市場、プロキシ文脈、安全な情報源処理の入口ガイド。コマンド利用前に読む。
---

# agent-finance core skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

最初に読む入口です。必要に応じて narrower runtime skill を読み込み、price、session、history、research、crypto、Polymarket、profile、browser boundary を監査可能な流れにします。

## 開始

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance skills list
agent-finance market providers
agent-finance capabilities
```

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

## タスクルーター

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get research-data
agent-finance skills get crypto
agent-finance skills get prediction-markets
agent-finance skills get providers
agent-finance skills get profile
agent-finance skills get tui
```

## デフォルト証拠フロー

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

```bash
agent-finance market history LITE --interval 1d --range 1mo --adjustment auto --limit 30
agent-finance market history LITE --interval 1m --range 5d --session extended --adjustment raw --no-actions --limit 120
```

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market analysis CRDO
agent-finance market options CRDO
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
```

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance skills get prediction-markets
```

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --provider okx --limit 20
agent-finance market crypto discover --provider coingecko --kind trending
```

```bash
agent-finance skills get profile
agent-finance account permissions --profile default --json
agent-finance account balances --profile default --json
agent-finance account positions --profile default --json
agent-finance risk explain --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID --json
agent-finance order open --profile default --market spot --symbol BTCUSDT --json
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20 --json
agent-finance state create --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state create --profile default --kind position-mode --position-mode hedge
agent-finance state submit INTENT_ID --profile default
agent-finance audit export --json
```

## 判断ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。
