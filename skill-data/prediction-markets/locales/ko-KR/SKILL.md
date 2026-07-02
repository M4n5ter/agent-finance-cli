---
name: prediction-markets
description: Polymarket 예측시장 데이터를 정량적 심리와 이벤트 확률 증거로 사용하는 가이드.
---

# agent-finance prediction-markets skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

Polymarket price는 자금이 반영된 implied probability입니다. sentiment와 event probability 증거이지 fact, insider information, equity quote가 아닙니다.

## 명령

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket search "spcex" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

## 검색 의미

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

## 해석 규칙

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

## 유용한 flag

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance market polymarket search "spacex ipo" --include-closed --min-volume 1000 --json
agent-finance market polymarket market MARKET_ID_OR_SLUG --limit 20 --refresh
```

## 경계

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.
