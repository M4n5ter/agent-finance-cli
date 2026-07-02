---
name: price
description: 현재 가격 요약, regular-market basis, premarket, postmarket, overnight, crypto 가격, proxy symbol, stream, watch 출력을 가져온다.
---

# agent-finance market price skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

`market price`는 “지금 얼마인가”의 기본 답입니다. pre/post/overnight, provider disagreement, proxy price가 중요하면 `market sessions`를 사용합니다.

## 기본 가격

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market price CRDO
agent-finance market price CRDO --json
```

## Session 분해

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

## Crypto와 proxy context

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market price BTC/USDT --asset crypto
agent-finance market price BTCUSDT --asset crypto --instrument spot
agent-finance market price BTCUSDT --asset crypto --instrument swap
```

```bash
agent-finance market sessions SPCX --proxy-symbol SPCXUSDT
```

## Stream과 watch

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
```
