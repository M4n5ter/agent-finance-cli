---
name: history-indicators
description: agent-finance로 OHLCV 히스토리와 로컬 지표를 가져온다. 주식/crypto interval, session, adjustment, repair, 지표 해석 규칙을 포함한다.
---

# agent-finance market history and indicators skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

Historical bars는 order-quality 판단의 기반입니다. 결론 전 daily와 minute bars를 함께 확인하고, indicator는 summary로만 사용합니다.

## 히스토리 데이터

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

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

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market history --help
agent-finance market stooq sync --help
```

## Adjustment와 repair

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

## 지표

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market indicators LITE AAOI --provider auto --limit 120
agent-finance market indicators CRDO MRVL --session extended --interval 1m --range 5d --limit 200
```
