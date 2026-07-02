---
name: crypto
description: Binance, Coinbase, OKX, CoinGecko 전반에서 spot, derivatives, quote, order book, trades, candles, funding, open interest, sentiment를 capability-first로 사용한다.
---

# agent-finance market crypto skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

Crypto는 24/7 market입니다. 기본은 capability로 provider를 고르며, Binance/OKX는 microstructure와 derivatives, Coinbase는 spot cross-check, CoinGecko는 breadth, trending, metadata에 강합니다.

## 시작

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market crypto snapshot BTC/USDT
agent-finance market crypto sentiment BTCUSDT
agent-finance market price BTC/USDT --asset crypto
agent-finance market history BTC/USDT --asset crypto --interval 1h --limit 48
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
```

## Provider 교차 증거

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

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

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market crypto quote BTC/USDT --instrument spot
agent-finance market crypto book BTC/USDT --instrument spot --limit 20
agent-finance market crypto candles BTC/USDT --instrument spot --interval 1m --limit 60
agent-finance market crypto funding BTCUSDT --instrument swap --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap
agent-finance market crypto stream BTCUSDT --kind trade --messages 1
agent-finance market crypto stream BTCUSDT --instrument swap --kind mark-price --messages 1
```

## 규칙

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.
