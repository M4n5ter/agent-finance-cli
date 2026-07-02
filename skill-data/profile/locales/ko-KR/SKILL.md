---
name: profile
description: agent-finance trading profile, Binance HMAC env 참조, risk policy, intent-first live writes, audit log, 보호된 signed workflow를 설정한다.
---

# agent-finance profile skill

현지화 안내: 이것은 AI Agent용 runtime instructions입니다. command, flag, provider name, JSON field, schema key, code block은 영어로 고정하고 외부 market evidence, news title, SEC text, provider 원문은 번역하지 않습니다.

Signed workflow는 intent-first입니다. intent를 만들고 risk를 확인하고 profile permissions를 확인한 뒤 submit합니다. secret은 env var reference로만 두고 config, docs, logs, prompt에 쓰지 않습니다.

## 모델

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

## Profile 권한

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```toml
[permissions]
spot_trading = true
usds_futures = true
universal_transfer = false
```

## 설정

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance profile path --profile default
agent-finance profile template --profile default
agent-finance profile doctor --profile default
agent-finance profile explain --profile default
agent-finance risk explain --profile default
agent-finance account permissions --profile default --json
agent-finance account balances --profile default --json
agent-finance account positions --profile default --json
```

아래 표의 field name과 `kind` 값은 machine contract이므로 영어로 유지합니다.

| Command | `kind` | Common payload path |
| --- | --- | --- |
| `account permissions` | `api-permissions` | `payload` |
| `account balances` | `spot-balances` | `payload.balances` |
| `account positions` | `usds-futures-positions` | `payload.assets`, `payload.positions` |
| `order query` | `order-query` | `payload` |
| `order open` | `open-orders` | `payload` |
| `transfer history` | `transfer-history` | `payload.rows` |

## 주문 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind limit --quantity 0.001 --price 50000 --time-in-force gtc
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind limit-maker --quantity 0.001 --price 50000
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind market --quantity 0.001 --valuation-price 50000
agent-finance risk check INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default --test
agent-finance order submit INTENT_ID --profile default --live
agent-finance order query BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID --json
agent-finance order open --profile default --market spot --symbol BTCUSDT --json
```

## 취소 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance order cancel BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID
agent-finance risk check CANCEL_INTENT_ID --profile default
agent-finance order submit CANCEL_INTENT_ID --profile default
agent-finance order submit CANCEL_INTENT_ID --profile default --live
```

## Futures state 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```toml
[[risk.allowed_futures_state_changes]]
kind = "position-mode"
mode = "hedge"
```

```bash
agent-finance state create --profile default --kind leverage --symbol BTCUSDT --leverage 2
agent-finance state create --profile default --kind margin-type --symbol BTCUSDT --margin-type isolated
agent-finance state create --profile default --kind position-mode --position-mode hedge
agent-finance risk check INTENT_ID --profile default --live
agent-finance state submit INTENT_ID --profile default
agent-finance state submit INTENT_ID --profile default --live
```

## Transfer 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance transfer create USDT --profile default --direction spot-to-usds-futures --amount 10
agent-finance risk check INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default --live
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20 --json
```

## Audit 흐름

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.

```bash
agent-finance audit tail --limit 20
agent-finance audit export --json
```

## Guardrails

이 절은 올바른 명령과 증거 경로를 고르는 데 사용합니다. 구조화 데이터는 `--json`을 우선하고, provider 강제는 교차 검증이나 provider 동작 감사 때만 사용합니다.
