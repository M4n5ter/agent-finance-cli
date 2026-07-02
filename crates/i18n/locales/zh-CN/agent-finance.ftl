app-name = agent-finance
locale-name = 简体中文
test-greeting = 你好，{ $name }。
tui-settings-language = 语言

cli-about = 获取金融市场数据和研究上下文，服务人类与 AI Agent 的证据驱动研究。
cli-usage = 用法：agent-finance [选项] <命令>
cli-commands-heading = 命令：
cli-options-heading = 选项：
cli-after-help = AI Agent：先运行 `agent-finance skills get core`；优先使用 capability-first 命令，只有交叉验证时才强制指定 provider。
cli-command-market = 获取只读市场数据、研究上下文、预测信号和实时流。
cli-command-tui = 打开交互式公开市场驾驶舱。
cli-command-capabilities = 输出面向能力的终端命令面。
cli-command-profile = 检查并解释交易 profile。
cli-command-account = 查看签名账户状态。
cli-command-order = 创建、提交、取消和查询订单意图。
cli-command-transfer = 创建并提交内部划转意图。
cli-command-state = 创建并提交 USD-M 合约状态变更意图。
cli-command-risk = 检查并解释 profile 风控策略。
cli-command-audit = 读取本地 append-only 交易审计事件。
cli-command-skills = 输出内置 AI Agent skill 文档。
cli-option-locale = 人类可读输出语言：en-US、zh-CN、ja-JP、ko-KR；支持 en/zh/ja/ko 别名。
cli-option-proxy = 显式指定 HTTP 或 SOCKS 代理 URL。
cli-option-no-proxy = 本次调用禁用代理。
cli-option-timezone = 人类可读输出时区；默认使用本机 IANA 时区。
cli-option-timeout-seconds = HTTP 超时时间，单位秒。
cli-option-help = 输出帮助。
cli-option-version = 输出版本。
cli-parse-error-guidance = 无法解析命令。请检查命令名、flag 和取值；可运行 `agent-finance --help` 或 `agent-finance skills get core`。

price-summary-title = { $symbol } 价格摘要  获取时间={ $fetched }  时区={ $timezone }
price-current = 当前：{ $currency } { $price }  交易时段={ $session }  来源={ $source }  涨跌={ $change }  时间={ $time }
price-current-missing = 当前：没有可用报价
price-regular-basis = 常规盘基准：前收={ $prevClose } 开盘={ $open } 最高={ $high } 最低={ $low } 成交量={ $volume }
price-proxy = 代理价：{ $currency } { $price } 来自 { $provider } 时间={ $time } 备注={ $note }
price-session-split-heading = 交易时段 / provider 拆分
price-session-split-note = 提示：已获取 { $count } 条交易时段/provider 数据；使用 sessions 查看拆分详情。
price-errors-heading = 报价错误
price-table-label = 标签
price-table-price = 价格
price-table-change = 涨跌%
price-table-session = 时段
price-table-provider = provider
price-table-time = 时间
price-table-open = 开盘
price-table-high = 最高
price-table-low = 最低
price-table-volume = 成交量

tui-settings-title = 配置驾驶舱
tui-settings-clean = 无修改
tui-settings-workspace = 工作区：{ $workspace }
tui-settings-language-summary = 语言：{ $language }（{ $locale }）
tui-settings-dirty-config = 配置变更：{ $dirty }
tui-settings-watchlist = 观察列表：{ $count } 个标的  当前={ $selected }
tui-settings-trading-profile = 交易 profile：{ $profile }  live writes={ $liveWrites }
tui-settings-submit-mode = 默认提交模式：{ $default }  生效={ $effective }
tui-settings-provider-preferences = provider 偏好：股票={ $equity }  加密={ $crypto }
tui-settings-theme = 主题：强调色={ $accent }  选中={ $selectionBackground }/{ $selectionForeground }
tui-settings-provider-capability-profiles = provider 能力档案：{ $count }
tui-settings-normal-key-bindings = 普通模式快捷键：{ $count }
tui-settings-editor-heading = 设置编辑器
tui-settings-pending = 待保存：{ $change }
tui-setting-language = 语言
tui-setting-equity-provider = 股票 provider
tui-setting-crypto-provider = 加密 provider
tui-setting-theme-accent = 主题强调色
tui-setting-selection-background = 选中背景
tui-setting-selection-foreground = 选中文字
tui-setting-key-command-palette = 快捷键：命令面板
tui-setting-key-symbol-search = 快捷键：标的搜索
tui-setting-key-provider-details = 快捷键：provider 详情
tui-setting-key-live-writes = 快捷键：live writes
tui-setting-key-save-config = 快捷键：保存配置
tui-setting-key-undo-config = 快捷键：撤销配置
tui-setting-unknown = 未知设置

tui-workspace-market = 市场
tui-workspace-trade = 交易
tui-workspace-account = 账户
tui-workspace-research = 研究
tui-workspace-settings = 设置
tui-pane-status-fresh = 最新
tui-pane-status-loading = 加载中
tui-pane-status-partial = 部分
tui-pane-status-empty = 空
tui-pane-status-error = 错误
tui-pane-status-stale = 过期
tui-panel-watchlist = 观察列表
tui-panel-quote = 报价 / 时段
tui-panel-order-ticket = 订单票据
tui-panel-open-orders = 活动订单
tui-panel-intent-review = 意图审查
tui-panel-risk-audit = 风险 / 审计
tui-panel-account = 账户
tui-panel-transfer-ticket = 划转票据
tui-panel-futures-state = 合约状态
tui-panel-history = 历史图表
tui-panel-evidence = 加密证据
tui-panel-polymarket = Polymarket
tui-panel-research = 新闻 / 研究
tui-panel-provider-health = Provider 健康
tui-panel-task-log = 任务日志
tui-panel-settings = 设置
tui-panel-profile-risk = Profile / 风险
