---
name: crypto
description: Binance、Coinbase、OKX、CoinGecko を横断し、spot、derivatives、quote、order book、trades、candles、funding、open interest、sentiment を capability-first で扱う。
---

# agent-finance market crypto skill

ローカライズ注記: これは AI Agent 向けの runtime instructions です。command、flag、provider 名、JSON field、schema key、code block は英語のまま固定し、外部の market evidence、news title、SEC text、provider 原文は翻訳しません。

Crypto は 24/7 market です。通常は capability で provider を選び、Binance/OKX は microstructure と derivatives、Coinbase は spot cross-check、CoinGecko は breadth、trending、metadata に使います。

## 開始

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
```

## Provider 横断証拠

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market crypto quote BTC/USDT
agent-finance market crypto quote BTC-USD --provider coinbase
agent-finance market crypto book BTC/USDT --provider okx --limit 20
agent-finance market crypto trades BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --provider coingecko --interval 1d --limit 30
agent-finance market crypto funding BTCUSDT --provider auto --instrument swap --limit 8
agent-finance market crypto open-interest BTCUSDT --provider okx --instrument swap
agent-finance market crypto discover --provider coingecko --kind trending
agent-finance market crypto discover --provider coingecko --kind global
agent-finance market crypto discover --provider okx --kind instruments --instrument swap
agent-finance market crypto discover --provider coinbase --kind volume-summary
```

## Instrument

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。

```bash
agent-finance market crypto quote BTC/USDT --instrument spot
agent-finance market crypto book BTC/USDT --instrument spot --limit 20
agent-finance market crypto candles BTC/USDT --instrument spot --interval 1m --limit 60
agent-finance market crypto funding BTCUSDT --instrument swap --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap
agent-finance market crypto stream BTCUSDT --kind trade --messages 1
agent-finance market crypto stream BTCUSDT --instrument swap --kind mark-price --messages 1
```

## ルール

この節は正しいコマンドと証拠経路を選ぶために使います。構造化データには `--json` を優先し、provider を固定するのは照合または provider 挙動の監査時だけにします。
