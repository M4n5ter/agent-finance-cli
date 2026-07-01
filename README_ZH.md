# agent-finance

[English](README.md) | [简体中文](README_ZH.md) | [日本語](README_JA.md) | [한국어](README_KO.md)

给 AI Agent 用的金融情报命令行工具：少一点猜测，多一点可审计证据。

`agent-finance` 是面向 AI Agent 和自动化流程的终端工具。它把报价、盘前/盘后/隔夜 session、历史 K 线、指标、加密市场结构、预测市场情绪、上市公司公开数据、URL 正文抽取、provider 能力矩阵，以及带风控的签名交易流程，整理成一组可脚本化的 CLI 命令。

它也提供 `agent-finance tui`，可以在一个终端里同时观察标的、provider health、研究上下文、crypto evidence 和预测市场信号。

| 市场 cockpit | 带风控的交易工作区 |
|---|---|
| ![agent-finance TUI 市场 cockpit 和 OHLCV K 线](assets/tui-market-cockpit.png) | ![agent-finance TUI 带风控的交易工作区](assets/tui-trade-workspace.png) |

安装后，Agent 可以直接从 CLI 读取自己的使用说明。

```bash
npm install -g agent-finance-cli
npx skills add https://github.com/M4n5ter/agent-finance
agent-finance skills get core
```

## 它能让 Agent 做什么

- 回答“现在这只标的是什么价格”，同时给出价格来源、session、常规交易时段基准和本地/UTC 时间。
- 区分常规盘、盘前、盘后、隔夜价格，避免把不同市场状态揉成一个看似确定的报价。
- 在判断趋势、挂单质量或止盈止损之前，拉取 OHLCV 历史数据和本地指标。
- 跨 Binance、Coinbase、OKX、CoinGecko 检查加密货币现货、swap、futures、order book、成交、K 线、funding、open interest 和市场热度。
- 把 Polymarket 作为可量化的情绪和事件概率信号，而不是只看社媒热度。
- 从 Yahoo、SEC EDGAR、Robinhood、CNBC、Stooq 以及 URL 读取 fallback 中获取 no-key 研究数据。
- 通过 CLI 查询每个 provider 真实支持什么能力，而不是凭 provider 名字猜。
- 当任务是监控、比较或推进调查，而不是抽取单个可解析 payload 时，打开实时终端 cockpit。
- 把 Binance 账户、订单、划转等写操作放在 profile、intent、risk check、显式 live 确认和 append-only audit log 后面。
- 通过内置 runtime skills 教 Agent 使用当前版本的命令面。

## 为什么需要它

金融研究 Agent 很容易被单一报价、搜索摘要或“看起来权威”的数据源误导。更可靠的流程应该能持续获取新数据，知道价格来自哪个 session，理解 provider 的覆盖边界，并在触碰真实账户时留下审计记录。

`agent-finance` 选择 CLI-first，是因为 Agent 天然适合操作 shell：

- 命令稳定，可脚本化；
- 需要结构化数据时可以输出 JSON；
- 终端输出适合另一个 Agent 或自动化层检查；
- provider 能力可在运行时查询；
- skills 跟随已安装版本一起发布，避免文档和命令漂移。

## 安装

安装 CLI 和 discovery skill：

```bash
npm install -g agent-finance-cli
npx skills add https://github.com/M4n5ter/agent-finance
```

项目名是 `agent-finance`。npm 包发布为 `agent-finance-cli`，只是因为 `agent-finance` 这个 npm 名称不可用。

npm 包会在支持的平台安装预构建二进制：

- macOS arm64 / x64
- Linux arm64 / x64
- Windows x64

正常 npm 安装路径不需要 Rust。如果当前平台没有预构建包，会回退到本地源码构建。源码构建需要 Rust/Cargo，以及 `wreq`/BoringSSL 所需的本地工具链：CMake、Clang/Clang++、libclang、binutils。

通过 GitHub：

```bash
cargo install --git https://github.com/M4n5ter/agent-finance agent-finance-cli
```

在 checkout 中安装：

```bash
cargo install --path crates/cli --locked
cargo run --bin agent-finance -- skills get core
```

## Agent 的入口

接入 AI Agent 时，不要先背命令参数。让已安装的 CLI 自己说明当前能力：

```bash
agent-finance skills list
agent-finance skills get core --full
```

npm 包也带一个标准 discovery skill：

```text
skills/agent-finance/SKILL.md
```

这个 stub 会把 Agent 引回 runtime skills，确保使用说明和已安装的二进制保持一致。

把安装后的 skill 当作粗粒度入口，再让 `agent-finance skills get ...` 从本地二进制输出更具体的命令说明。

常用 runtime skills：

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get crypto
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get prediction-markets
agent-finance skills get profile
agent-finance skills get tui
```

## 快速预览

当前价格和 session：

```bash
agent-finance market price CRDO
agent-finance market price CRDO MRVL --json
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

历史数据和指标：

```bash
agent-finance market history CRDO --range 1mo --interval 1d
agent-finance market history CRDO --range 5d --interval 1m --session extended --adjustment raw --no-actions
agent-finance market indicators CRDO MRVL --limit 120
```

加密市场结构：

```bash
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
agent-finance market crypto funding BTCUSDT --instrument swap --provider auto --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap --provider okx
agent-finance market crypto discover --provider coingecko --kind trending
```

上市公司研究和发现：

```bash
agent-finance market fundamentals CRDO
agent-finance market fundamentals CRDO --provider sec-edgar
agent-finance market analysis CRDO
agent-finance market options CRDO --provider robinhood --count 80
agent-finance market ownership CRDO
agent-finance market events CRDO --provider sec-edgar
agent-finance market news CRDO
agent-finance market search "optical interconnect"
agent-finance market screen day_gainers
```

预测市场情绪：

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

流式、轮询和 URL 抽取：

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
```

provider 能力发现：

```bash
agent-finance market providers
agent-finance capabilities
```

交互式 cockpit：

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
agent-finance tui --symbols CRDO,LITE,AAOI --chart-preset auto
```

TUI 是交互式 cockpit，包含 watchlist、quote/sessions、history、crypto evidence、research、Polymarket、provider health、task log、鼠标聚焦、docked column 拖拽调整、floating 右下角调整、关闭/恢复 panel 控制、可执行 command palette，以及原生 OHLCV K 线 workbench。在 History panel 中按 `z` 进入完整图表，hover 查看 O/H/L/C/V，滚轮或拖拽缩放，点击图上价格可填入订单 ticket 草稿，也可以用 `j`/`k` 选择 current price、previous close、open、high、low、活动订单或持仓成本等参考线，再按 `Enter` 复制到 ticket。command palette 还可以把选中的参考线准备成单腿 stop-loss / take-profit ticket 草稿，或收集到 draft-only protective OCO plan 中。图表只辅助准备交易，不会直接提交；仍然必须经过 stage、review、risk、live confirmation 和 audit。使用 `agent-finance skills get tui` 查看 cockpit 工作流；需要结构化数据时，应继续使用 `market ... --json`。

图表 preset 会按 provider 能力调整。source 行显示 provider fallback 后的 provider、session、range、provider-reported interval 和 bar 数量。
除非使用 `--no-persist`，TUI 会把 watchlist、docked panel 集合、当前 focused panel、列布局、floating panes、刷新频率和 provider 偏好持久化到 TOML。

## 签名交易流程

`agent-finance` 包含受保护的 Binance Spot 和 USD-M 流程，可用于账户读取、订单 intent、内部划转 intent、futures 状态变更、风控检查和审计日志。

关键设计是：真实写操作不会变成随手可执行的一行命令。它必须经过 profile、risk policy、白名单、intent 文件、显式 `--live` 确认、provider 权限检查和 append-only audit event。

从这里开始：

```bash
agent-finance skills get profile --full
agent-finance profile template --profile default
agent-finance profile doctor --profile default
```

命令族示例：

```bash
agent-finance account balances --profile default
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind limit --quantity 0.001 --price 50000
agent-finance risk check INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance audit tail --limit 20
```

## Provider 说明

- `market price SYMBOL` 是回答“现在交易价格是多少”的默认入口。
- `market sessions SYMBOL` 用于明确比较 regular/pre/post/overnight/provider。
- `market history` 默认使用 adjusted price，并包含 corporate actions，除非显式关闭。
- `market providers` 是 provider 能力矩阵的准确信息来源。不要从 provider 名字推断覆盖范围。
- crypto 命令按 capability 优先设计。只有在交叉验证或审计时才强制指定 `--provider binance|coinbase|okx|coingecko`。
- Binance 和 OKX 更适合交易所及衍生品微观结构；Coinbase 适合现货交易所交叉验证；CoinGecko 适合聚合广度、trending 和 metadata。
- Binance USD-M futures 和 TradFi proxy symbols 是衍生品/代理信号，不是法定股票、券商成交价或 pre-IPO 持仓价格。
- Polymarket 适合观察 implied probability、spread、volume、liquidity、open interest、holder preview 和 probability history。它不是 primary-source fact feed。
- `market read-url` 是 direct/Jina/Defuddle 的正文抽取 fallback，不是可登录浏览器。
- 动态页面、登录态页面、需要截图判断的页面，或噪声很大的页面，仍应使用真实浏览器工具验证，例如 `agent-browser` 或 OpenCLI。

## 网络和本地状态

代理优先级：

1. `--proxy`
2. `AGENT_FINANCE_PROXY`
3. `ALL_PROXY`
4. `HTTPS_PROXY`
5. `HTTP_PROXY`

示例：

```bash
agent-finance --proxy socks5h://127.0.0.1:7890 market price CRDO
agent-finance --no-proxy market price CRDO
```

测试、沙盒和 Agent 工作区可以覆盖本地 profile/data 根目录：

```bash
AGENT_FINANCE_CONFIG_HOME=/tmp/agent-finance/config \
AGENT_FINANCE_DATA_HOME=/tmp/agent-finance/data \
agent-finance profile template --profile default
```

SEC EDGAR 请求会优先使用 `AGENT_FINANCE_SEC_USER_AGENT`，否则使用项目级 user agent。

设置 `AGENT_FINANCE_SKILL_DATA_DIR` 可以测试或替换 runtime skill 文档。npm wrapper 会为预构建平台二进制自动设置 `AGENT_FINANCE_PACKAGE_ROOT`。

## 安全

本工具不构成投资建议。市场数据可能延迟、不完整或错误；provider payload 可能变化；社交信号和预测市场信号只是证据，不是真相。重要事实应回到 primary sources 验证，并遵守数据源条款。
