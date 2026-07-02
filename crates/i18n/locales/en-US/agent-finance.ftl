app-name = agent-finance
locale-name = English
test-greeting = Hello, { $name }.
tui-settings-language = Language

cli-about = Fetch financial market data and research context for humans and AI agents.
cli-usage = Usage: agent-finance [OPTIONS] <COMMAND>
cli-commands-heading = Commands:
cli-options-heading = Options:
cli-after-help = AI agents: start with `agent-finance skills get core`; prefer capability-first commands, then force providers only for cross-checks.
cli-command-market = Fetch read-only market data, research context, prediction signals, and streams.
cli-command-tui = Open the interactive public-market cockpit.
cli-command-capabilities = Print the capability-first terminal surface.
cli-command-profile = Inspect and explain trading profiles.
cli-command-account = Inspect signed account state.
cli-command-order = Create, submit, cancel, and query order intents.
cli-command-transfer = Create and submit internal transfer intents.
cli-command-state = Create and submit USD-M futures state-change intents.
cli-command-risk = Check and explain profile risk policy.
cli-command-audit = Read local append-only trading audit events.
cli-command-skills = Print built-in AI-agent skill documents.
cli-option-locale = Human-output locale: en-US, zh-CN, ja-JP, ko-KR; aliases en/zh/ja/ko.
cli-option-proxy = Explicit HTTP or SOCKS proxy URL.
cli-option-no-proxy = Disable proxy use for this invocation.
cli-option-timezone = Human-output timezone; defaults to the machine local IANA timezone.
cli-option-timeout-seconds = HTTP timeout in seconds.
cli-option-help = Print help.
cli-option-version = Print version.
cli-parse-error-guidance = Could not parse the command. Check the command name, flags, and values; run `agent-finance --help` or `agent-finance skills get core`.

price-summary-title = { $symbol } price summary  fetched={ $fetched }  tz={ $timezone }
price-current = Current: { $currency } { $price }  session={ $session }  source={ $source }  change={ $change }  time={ $time }
price-current-missing = Current: no quote available
price-regular-basis = Regular basis: prev_close={ $prevClose } open={ $open } high={ $high } low={ $low } volume={ $volume }
price-proxy = Proxy: { $currency } { $price } via { $provider } time={ $time } note={ $note }
price-session-split-heading = Session / provider split
price-session-split-note = Note: fetched { $count } session/provider rows; use sessions to inspect the split.
price-errors-heading = Quote errors
price-table-label = label
price-table-price = price
price-table-change = chg%
price-table-session = session
price-table-provider = provider
price-table-time = time
price-table-open = open
price-table-high = high
price-table-low = low
price-table-volume = volume

tui-settings-title = configuration cockpit
tui-settings-clean = clean
tui-settings-workspace = workspace: { $workspace }
tui-settings-language-summary = language: { $language } ({ $locale })
tui-settings-dirty-config = dirty config: { $dirty }
tui-settings-watchlist = watchlist: { $count } symbols  selected={ $selected }
tui-settings-trading-profile = trading profile: { $profile }  live writes={ $liveWrites }
tui-settings-submit-mode = default submit mode: { $default }  effective={ $effective }
tui-settings-provider-preferences = provider preferences: equity={ $equity }  crypto={ $crypto }
tui-settings-theme = theme: accent={ $accent }  selection={ $selectionBackground }/{ $selectionForeground }
tui-settings-provider-capability-profiles = provider capability profiles: { $count }
tui-settings-normal-key-bindings = normal key bindings: { $count }
tui-settings-editor-heading = settings editor
tui-settings-pending = pending: { $change }
tui-setting-language = language
tui-setting-equity-provider = equity provider
tui-setting-crypto-provider = crypto provider
tui-setting-theme-accent = theme accent
tui-setting-selection-background = selection background
tui-setting-selection-foreground = selection foreground
tui-setting-key-command-palette = key command palette
tui-setting-key-symbol-search = key symbol search
tui-setting-key-provider-details = key provider details
tui-setting-key-live-writes = key live writes
tui-setting-key-save-config = key save config
tui-setting-key-undo-config = key undo config
tui-setting-unknown = unknown setting

tui-workspace-market = Market
tui-workspace-trade = Trade
tui-workspace-account = Account
tui-workspace-research = Research
tui-workspace-settings = Settings
tui-pane-status-fresh = fresh
tui-pane-status-loading = loading
tui-pane-status-partial = partial
tui-pane-status-empty = empty
tui-pane-status-error = error
tui-pane-status-stale = stale
tui-panel-watchlist = Watchlist
tui-panel-quote = Quote / Sessions
tui-panel-order-ticket = Order Ticket
tui-panel-open-orders = Open Orders
tui-panel-intent-review = Intent Review
tui-panel-risk-audit = Risk / Audit
tui-panel-account = Account
tui-panel-transfer-ticket = Transfer Ticket
tui-panel-futures-state = Futures State
tui-panel-history = History Chart
tui-panel-evidence = Crypto Evidence
tui-panel-polymarket = Polymarket
tui-panel-research = News / Research
tui-panel-provider-health = Provider Health
tui-panel-task-log = Task Log
tui-panel-settings = Settings
tui-panel-profile-risk = Profile / Risk
