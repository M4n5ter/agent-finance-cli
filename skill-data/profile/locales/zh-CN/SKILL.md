---
name: profile
description: 配置 agent-finance 交易 profile、Binance HMAC 环境变量引用、风险策略、intent-first live write、审计日志和受保护的签名工作流。
---

# agent-finance profile skill

本地化说明：这是给 AI Agent 的运行时指令。命令、flag、provider 名、JSON 字段、schema key 和代码块保持英文稳定；外部市场证据、新闻标题、SEC 文本、provider 原文不要翻译。

签名工作流必须 intent-first：创建 intent，检查 risk，确认 profile 权限，再 submit。密钥只通过环境变量引用，绝不写进配置、文档、日志或 prompt。

## 模型

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

## Profile 权限

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```toml
[permissions]
spot_trading = true
usds_futures = true
universal_transfer = false
```

## 设置

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

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

下表中的字段名和 `kind` 值是机器契约，保持英文。

| Command | `kind` | Common payload path |
| --- | --- | --- |
| `account permissions` | `api-permissions` | `payload` |
| `account balances` | `spot-balances` | `payload.balances` |
| `account positions` | `usds-futures-positions` | `payload.assets`, `payload.positions` |
| `order query` | `order-query` | `payload` |
| `order open` | `open-orders` | `payload` |
| `transfer history` | `transfer-history` | `payload.rows` |

## 订单流程

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

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

## 撤单流程

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance order cancel BTCUSDT --profile default --market spot --client-order-id CLIENT_ORDER_ID
agent-finance risk check CANCEL_INTENT_ID --profile default
agent-finance order submit CANCEL_INTENT_ID --profile default
agent-finance order submit CANCEL_INTENT_ID --profile default --live
```

## Futures 状态流程

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

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

## 划转流程

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance transfer create USDT --profile default --direction spot-to-usds-futures --amount 10
agent-finance risk check INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default
agent-finance transfer submit INTENT_ID --profile default --live
agent-finance transfer history --profile live --direction spot-to-usds-futures --size 20 --json
```

## 审计流程

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。

```bash
agent-finance audit tail --limit 20
agent-finance audit export --json
```

## 护栏

使用本节选择正确命令和证据路径。优先结构化 `--json` 输出；只有交叉验证或审计 provider 行为时才强制指定 provider。
