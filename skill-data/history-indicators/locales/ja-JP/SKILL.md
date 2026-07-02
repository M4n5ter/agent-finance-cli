---
name: history-indicators
description: agent-finance で OHLCV 履歴とローカル指標を取得する。株式/crypto interval、session、adjustment、repair、指標解釈を含む。
---

# agent-finance market history and indicators skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

Historical bars are the basis for order-quality decisions. 結論前に daily と minute bars を確認し、indicator は summary として扱います。

## 履歴データ

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market history LITE --provider auto --interval 1d --range 1mo --limit 30
agent-finance market history LITE --interval 1m --range 5d --session extended --adjustment raw --no-actions --limit 200
agent-finance market history LITE --interval 1d --range 1y --adjustment auto --repair --limit 252
agent-finance market history AAPL --provider robinhood --interval 5m --range 1d --session extended --limit 80
agent-finance market history BTC/USDT --asset crypto --crypto-provider auto --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider coinbase --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider okx --interval 1h --limit 48
agent-finance market history BTC/USDT --asset crypto --crypto-provider coingecko --interval 1d --limit 30
agent-finance market history BTCUSDT --asset crypto --crypto-provider binance --instrument swap --interval 1d --limit 30
```

## Interval

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market history --help
agent-finance market stooq sync --help
```

## Adjustment と repair

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

## 指標

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market indicators LITE AAOI --provider auto --limit 120
agent-finance market indicators CRDO MRVL --session extended --interval 1m --range 5d --limit 200
```
