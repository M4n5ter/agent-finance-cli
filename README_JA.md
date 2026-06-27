# agent-finance

[English](README.md) | [简体中文](README_ZH.md) | [日本語](README_JA.md) | [한국어](README_KO.md)

AI エージェントが金融市場を調べるための、証拠ベースのコマンドラインツールです。

`agent-finance` は、AI エージェントや自動化ワークフロー向けのターミナルネイティブな金融ツールキットです。株価、取引セッション、ヒストリカルデータ、テクニカル指標、暗号資産の市場構造、予測市場のセンチメント、公開企業データ、URL 本文抽出、データプロバイダーの対応範囲、そして安全ガード付きの署名済み取引フローを、CLI から扱えるようにします。

`agent-finance tui` も含まれており、銘柄、provider health、research context、crypto evidence、prediction-market signals を 1 つの terminal cockpit で確認できます。

インストール後は、エージェント自身が CLI から使い方を読み取れます。

```bash
npm install -g agent-finance-cli
npx skills add https://github.com/M4n5ter/agent-finance
agent-finance skills get core
```

## エージェントにできること

- 「いまいくらで取引されているか」に対して、価格、セッション、通常取引時間の基準値、ローカル時刻と UTC 時刻をまとめて返す。
- 通常取引、プレマーケット、アフターマーケット、オーバーナイトを分けて扱い、異なる市場状態をひとつの価格として誤読しない。
- トレンド判断や注文条件の評価前に、OHLCV とローカル指標を取得する。
- Binance、Coinbase、OKX、CoinGecko を横断して、暗号資産のスポット、swap、futures、板情報、約定、ローソク足、funding、open interest、市場の広がりを確認する。
- Polymarket を、イベント確率や市場センチメントを数値で見るための補助情報として使う。
- Yahoo、SEC EDGAR、Robinhood、CNBC、Stooq、URL 抽出 fallback から、API キーなしで取得できる調査データを集める。
- プロバイダー名から推測せず、CLI で各 provider の実際の対応範囲を確認する。
- 監視、比較、調査の舵取りが目的で、単一の parseable payload を抽出するだけではない場合は、live terminal cockpit を開く。
- Binance のアカウント、注文、振替などの書き込み系操作を、profile、intent、risk check、明示的な live 確認、append-only audit log の後ろに置く。
- インストール済みバージョンに同梱された runtime skills で、エージェントに正しい使い方を教える。

## なぜ必要か

金融調査エージェントは、単一の価格、検索スニペット、もっともらしいデータソース名だけに頼ると簡単に判断を誤ります。必要なのは、新しいデータを繰り返し取得し、その価格がどのセッション由来なのかを把握し、provider の限界を理解し、実口座に触れる場合は監査可能な記録を残すことです。

`agent-finance` は CLI-first です。AI エージェントはシェル操作との相性がよく、CLI なら次の性質を保てます。

- コマンドが安定していてスクリプト化しやすい。
- 構造化データが必要なときは JSON を返せる。
- ターミナル出力は、別のエージェントや自動化レイヤーが確認しやすい。
- provider の capabilities を実行時に確認できる。
- skills がバイナリと一緒に配布されるため、説明と実装がずれにくい。

## インストール

npm から：

```bash
npm install -g agent-finance-cli
```

対応しているエージェント環境には discovery skill も追加できます。

```bash
npx skills add https://github.com/M4n5ter/agent-finance
```

プロジェクト名は `agent-finance` です。npm では `agent-finance` という名前を使えないため、パッケージ名だけ `agent-finance-cli` として公開しています。

npm パッケージは、対応プラットフォームではビルド済みバイナリをインストールします。

- macOS arm64 / x64
- Linux arm64 / x64
- Windows x64

通常の npm インストールでは Rust は不要です。対応するビルド済みパッケージがない場合は、ローカルでソースビルドにフォールバックします。その場合は Rust/Cargo に加えて、`wreq`/BoringSSL に必要な CMake、Clang/Clang++、libclang、binutils が必要です。

GitHub から：

```bash
cargo install --git https://github.com/M4n5ter/agent-finance agent-finance-cli
```

checkout から：

```bash
cargo install --path crates/cli --locked
cargo run --bin agent-finance -- skills get core
```

## エージェント向けの入口

AI エージェントに組み込む場合、最初からフラグを覚えさせる必要はありません。インストール済み CLI に現在の使い方を説明させます。

```bash
agent-finance skills list
agent-finance skills get core --full
```

npm パッケージには標準的な discovery skill も含まれています。

```text
skills/agent-finance/SKILL.md
```

この stub はエージェントを runtime skills に戻すため、コマンドの説明がインストール済みバイナリと一致します。

追加した skill は大まかな入口として使い、具体的なコマンドの使い方はローカルの `agent-finance skills get ...` から取得します。

よく使う runtime skills：

```bash
agent-finance skills get price
agent-finance skills get history-indicators
agent-finance skills get crypto
agent-finance skills get research-data
agent-finance skills get providers
agent-finance skills get prediction-markets
agent-finance skills get profile
```

## クイックツアー

現在価格とセッション：

```bash
agent-finance market price CRDO
agent-finance market price CRDO MRVL --json
agent-finance market sessions CRDO
agent-finance market sessions LITE --proxy-symbol LITEUSDT
```

ヒストリカルデータと指標：

```bash
agent-finance market history CRDO --range 1mo --interval 1d
agent-finance market history CRDO --range 5d --interval 1m --session extended --adjustment raw --no-actions
agent-finance market indicators CRDO MRVL --limit 120
```

暗号資産の市場構造：

```bash
agent-finance market crypto quote BTC/USDT
agent-finance market crypto book BTC/USDT --limit 20
agent-finance market crypto candles BTC/USDT --interval 1h --limit 48
agent-finance market crypto funding BTCUSDT --instrument swap --provider auto --limit 8
agent-finance market crypto open-interest BTCUSDT --instrument swap --provider okx
agent-finance market crypto discover --provider coingecko --kind trending
```

公開企業データと銘柄探索：

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

予測市場センチメント：

```bash
agent-finance market polymarket search "spacex ipo" --limit 5
agent-finance market polymarket market MARKET_ID_OR_SLUG --json
```

ストリーム、ポーリング、URL 抽出：

```bash
agent-finance market stream CRDO --messages 5
agent-finance market watch CRDO --interval-seconds 15 --iterations 4
agent-finance market read-url "https://www.sec.gov/Archives/edgar/data/0001807794/000162828026014017/crdo-20260131.htm"
```

provider capabilities の確認：

```bash
agent-finance market providers
agent-finance capabilities
```

インタラクティブ cockpit：

```bash
agent-finance tui --symbols AAPL,CRDO,BTCUSDT
```

TUI は、watchlist、quote/sessions、history、crypto evidence、research、Polymarket、provider health、task log、マウスフォーカス、docked column のドラッグリサイズ、floating corner resize、panel の close/restore、実行可能な command palette を備えたインタラクティブ cockpit です。ライブ監視や探索の進行管理に使い、構造化データが必要な場合は引き続き `market ... --json` を使ってください。
`--no-persist` を指定しない限り、TUI は watchlist、docked panel の構成、現在 focused な panel、列レイアウト、floating panes、更新間隔、provider 設定を TOML に保存します。

## 署名付き取引フロー

`agent-finance` には、Binance Spot と USD-M 向けの保護されたフローがあります。アカウント読み取り、注文 intent、内部振替 intent、futures state change、risk check、audit log を扱えます。

重要なのは、live write を気軽なワンライナーにしないことです。profile、risk policy、allowlist、intent file、明示的な `--live`、provider permission check、append-only audit event を通してから実行します。

まずはここから：

```bash
agent-finance skills get profile --full
agent-finance profile template --profile default
agent-finance profile doctor --profile default
```

コマンド例：

```bash
agent-finance account balances --profile default
agent-finance order create BTCUSDT --profile default --market spot --side buy --kind limit --quantity 0.001 --price 50000
agent-finance risk check INTENT_ID --profile default
agent-finance order submit INTENT_ID --profile default
agent-finance audit tail --limit 20
```

## Provider について

- `market price SYMBOL` は「いまの価格」を聞くときの基本入口です。
- `market sessions SYMBOL` は regular/pre/post/overnight/provider を明示的に比較するときに使います。
- `market history` はデフォルトで adjusted price を使い、明示的に無効化しない限り corporate actions を含めます。
- `market providers` が capability matrix の正です。provider 名だけで対応範囲を推測しないでください。
- crypto コマンドは capability-first です。`--provider binance|coinbase|okx|coingecko` を固定するのは、クロスチェックや監査目的のときに限るのが基本です。
- Binance と OKX は取引所データやデリバティブの市場構造に強く、Coinbase はスポット取引所のクロスチェック、CoinGecko は集計、trending、metadata に向いています。
- Binance USD-M futures や TradFi proxy symbols はデリバティブまたは proxy です。法的な株式、証券会社の約定価格、pre-IPO 持分価格ではありません。
- Polymarket は implied probability、spread、volume、liquidity、open interest、holder preview、probability history を見るための補助材料です。一次情報ではありません。
- `market read-url` は direct/Jina/Defuddle による本文抽出 fallback であり、ログイン可能なブラウザではありません。
- 動的ページ、ログインが必要なページ、スクリーンショットでの確認が必要なページ、ノイズが多いページは、`agent-browser` や OpenCLI のような実ブラウザツールで確認してください。

## ネットワークとローカル状態

プロキシの優先順位：

1. `--proxy`
2. `AGENT_FINANCE_PROXY`
3. `ALL_PROXY`
4. `HTTPS_PROXY`
5. `HTTP_PROXY`

例：

```bash
agent-finance --proxy socks5h://127.0.0.1:7890 market price CRDO
agent-finance --no-proxy market price CRDO
```

テスト、サンドボックス、エージェント用ワークスペースでは、profile/data のルートを上書きできます。

```bash
AGENT_FINANCE_CONFIG_HOME=/tmp/agent-finance/config \
AGENT_FINANCE_DATA_HOME=/tmp/agent-finance/data \
agent-finance profile template --profile default
```

SEC EDGAR へのリクエストでは、`AGENT_FINANCE_SEC_USER_AGENT` が設定されていればそれを使い、なければプロジェクト既定の user agent を使います。

`AGENT_FINANCE_SKILL_DATA_DIR` を設定すると runtime skill documents をテストまたは差し替えできます。npm wrapper は、ビルド済み platform binary のために `AGENT_FINANCE_PACKAGE_ROOT` を自動設定します。

## 安全性

このツールは投資助言ではありません。市場データは遅延、不完全、または誤っている可能性があります。provider payload は変更される可能性があります。ソーシャルシグナルや予測市場のシグナルは証拠の一部であって、真実そのものではありません。重要な事実は一次情報で確認し、各データソースの利用条件に従ってください。
