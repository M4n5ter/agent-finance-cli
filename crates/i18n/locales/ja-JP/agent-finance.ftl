app-name = agent-finance
locale-name = 日本語
test-greeting = こんにちは、{ $name }。
tui-settings-language = 言語

cli-about = 金融市場データとリサーチ文脈を取得し、人と AI Agent の根拠ある調査を支援します。
cli-usage = 使い方: agent-finance [OPTIONS] <COMMAND>
cli-commands-heading = コマンド:
cli-options-heading = オプション:
cli-after-help = AI Agent はまず `agent-finance skills get core` を実行してください。通常は capability-first コマンドを使い、照合が必要な場合だけ provider を明示します。
cli-command-market = 読み取り専用の市場データ、リサーチ文脈、予測シグナル、ストリームを取得します。
cli-command-tui = インタラクティブな公開市場コックピットを開きます。
cli-command-capabilities = capability-first の端末インターフェースを表示します。
cli-command-profile = 取引 profile を確認して説明します。
cli-command-account = 署名付きアカウント状態を確認します。
cli-command-order = 注文 intent の作成、送信、取消、照会を行います。
cli-command-transfer = 内部振替 intent を作成して送信します。
cli-command-state = USD-M 先物の状態変更 intent を作成して送信します。
cli-command-risk = profile のリスクポリシーを確認して説明します。
cli-command-audit = ローカルの append-only 取引監査イベントを読み取ります。
cli-command-skills = 内蔵 AI Agent skill ドキュメントを表示します。
cli-option-locale = 人が読む出力の言語: en-US、zh-CN、ja-JP、ko-KR。en/zh/ja/ko も使えます。
cli-option-proxy = HTTP または SOCKS プロキシ URL を明示します。
cli-option-no-proxy = この実行ではプロキシを使いません。
cli-option-timezone = 人が読む出力のタイムゾーン。既定はこのマシンの IANA タイムゾーンです。
cli-option-timeout-seconds = HTTP タイムアウト秒数。
cli-option-help = ヘルプを表示します。
cli-option-version = バージョンを表示します。
cli-parse-error-guidance = コマンドを解析できませんでした。コマンド名、flag、値を確認してください。`agent-finance --help` または `agent-finance skills get core` を実行できます。

price-summary-title = { $symbol } 価格サマリー  取得={ $fetched }  タイムゾーン={ $timezone }
price-current = 現在値: { $currency } { $price }  セッション={ $session }  ソース={ $source }  変化={ $change }  時刻={ $time }
price-current-missing = 現在値: 利用できる気配値がありません
price-regular-basis = 通常取引基準: 前日終値={ $prevClose } 始値={ $open } 高値={ $high } 安値={ $low } 出来高={ $volume }
price-proxy = プロキシ価格: { $currency } { $price } via { $provider } 時刻={ $time } メモ={ $note }
price-session-split-heading = セッション / provider 別内訳
price-session-split-note = 注記: { $count } 件のセッション/provider 行を取得しました。内訳は sessions で確認できます。
price-errors-heading = 気配値エラー
price-table-label = ラベル
price-table-price = 価格
price-table-change = 騰落%
price-table-session = セッション
price-table-provider = provider
price-table-time = 時刻
price-table-open = 始値
price-table-high = 高値
price-table-low = 安値
price-table-volume = 出来高

tui-settings-title = 設定コックピット
tui-settings-clean = 変更なし
tui-settings-workspace = ワークスペース: { $workspace }
tui-settings-language-summary = 言語: { $language } ({ $locale })
tui-settings-dirty-config = 設定変更: { $dirty }
tui-settings-watchlist = ウォッチリスト: { $count } 銘柄  選択={ $selected }
tui-settings-trading-profile = 取引 profile: { $profile }  live writes={ $liveWrites }
tui-settings-submit-mode = 既定の送信モード: { $default }  有効={ $effective }
tui-settings-provider-preferences = provider 設定: 株式={ $equity }  暗号資産={ $crypto }
tui-settings-theme = テーマ: アクセント={ $accent }  選択={ $selectionBackground }/{ $selectionForeground }
tui-settings-provider-capability-profiles = provider 能力プロファイル: { $count }
tui-settings-normal-key-bindings = 通常モードのキー割り当て: { $count }
tui-settings-editor-heading = 設定エディタ
tui-settings-pending = 未保存: { $change }
tui-setting-language = 言語
tui-setting-equity-provider = 株式 provider
tui-setting-crypto-provider = 暗号資産 provider
tui-setting-theme-accent = テーマのアクセント
tui-setting-selection-background = 選択背景
tui-setting-selection-foreground = 選択文字
tui-setting-key-command-palette = キー: コマンドパレット
tui-setting-key-symbol-search = キー: 銘柄検索
tui-setting-key-provider-details = キー: provider 詳細
tui-setting-key-live-writes = キー: live writes
tui-setting-key-save-config = キー: 設定保存
tui-setting-key-undo-config = キー: 設定取り消し
tui-setting-unknown = 不明な設定

tui-workspace-market = 市場
tui-workspace-trade = 取引
tui-workspace-account = 口座
tui-workspace-research = リサーチ
tui-workspace-settings = 設定
tui-pane-status-fresh = 最新
tui-pane-status-loading = 読込中
tui-pane-status-partial = 部分
tui-pane-status-empty = 空
tui-pane-status-error = エラー
tui-pane-status-stale = 古い
tui-panel-watchlist = ウォッチリスト
tui-panel-quote = 気配値 / セッション
tui-panel-order-ticket = 注文チケット
tui-panel-open-orders = 未約定注文
tui-panel-intent-review = intent レビュー
tui-panel-risk-audit = リスク / 監査
tui-panel-account = 口座
tui-panel-transfer-ticket = 振替チケット
tui-panel-futures-state = 先物状態
tui-panel-history = 履歴チャート
tui-panel-evidence = 暗号資産エビデンス
tui-panel-polymarket = Polymarket
tui-panel-research = ニュース / リサーチ
tui-panel-provider-health = Provider 状態
tui-panel-task-log = タスクログ
tui-panel-settings = 設定
tui-panel-profile-risk = Profile / リスク
