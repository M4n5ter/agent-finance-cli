---
name: core
description: agent-finance의 시장 가격, 세션, crypto, 히스토리, 리서치 데이터, provider 범위, 예측시장, 프록시 맥락, 안전한 소스 처리를 위한 시작 가이드. 명령 사용 전에 읽는다.
---

# agent-finance core skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

먼저 읽는 진입점입니다. 필요할 때 더 좁은 runtime skill을 불러와 price, session, history, research, crypto, Polymarket, profile, browser boundary를 감사 가능한 흐름으로 연결합니다.

## 시작

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance skills list
agent-finance market providers
agent-finance capabilities
```

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

## 작업 라우터

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

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

## 기본 증거 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

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

## 판단 규칙

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.
