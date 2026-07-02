---
name: research-data
description: no-key Yahoo, SEC EDGAR, Robinhood, CNBC 리서치 데이터를 가져온다. fundamentals, analyst data, options, ownership, events, news, search, screeners, URL 추출을 포함한다.
---

# agent-finance research data skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

No-key research source는 leads와 cross-check용입니다. official facts는 SEC, IR, company releases, earnings calls를 우선하고, 추출이 의심스러운 page는 real browser로 검증합니다.

## 명령

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

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

## 출력 규칙

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

## Research 규칙

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.
