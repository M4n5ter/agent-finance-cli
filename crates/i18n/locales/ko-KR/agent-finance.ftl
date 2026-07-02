app-name = agent-finance
locale-name = 한국어
test-greeting = 안녕하세요, { $name }.
tui-settings-language = 언어

cli-about = 금융 시장 데이터와 리서치 맥락을 가져와 사람과 AI Agent의 근거 기반 조사를 돕습니다.
cli-usage = 사용법: agent-finance [OPTIONS] <COMMAND>
cli-commands-heading = 명령:
cli-options-heading = 옵션:
cli-after-help = AI Agent는 먼저 `agent-finance skills get core`를 실행하세요. 기본은 capability-first 명령을 쓰고, 교차 검증이 필요할 때만 provider를 강제하세요.
cli-command-market = 읽기 전용 시장 데이터, 리서치 맥락, 예측 신호, 스트림을 가져옵니다.
cli-command-tui = 대화형 공개 시장 콕핏을 엽니다.
cli-command-capabilities = capability-first 터미널 표면을 출력합니다.
cli-command-profile = 거래 profile을 점검하고 설명합니다.
cli-command-account = 서명된 계정 상태를 확인합니다.
cli-command-order = 주문 intent를 만들고 제출, 취소, 조회합니다.
cli-command-transfer = 내부 이체 intent를 만들고 제출합니다.
cli-command-state = USD-M 선물 상태 변경 intent를 만들고 제출합니다.
cli-command-risk = profile 리스크 정책을 점검하고 설명합니다.
cli-command-audit = 로컬 append-only 거래 감사 이벤트를 읽습니다.
cli-command-skills = 내장 AI Agent skill 문서를 출력합니다.
cli-option-locale = 사람이 읽는 출력 언어: en-US, zh-CN, ja-JP, ko-KR. en/zh/ja/ko 별칭도 지원합니다.
cli-option-proxy = HTTP 또는 SOCKS 프록시 URL을 명시합니다.
cli-option-no-proxy = 이번 실행에서는 프록시를 사용하지 않습니다.
cli-option-timezone = 사람이 읽는 출력 시간대. 기본값은 이 머신의 IANA 시간대입니다.
cli-option-timeout-seconds = HTTP 타임아웃 초.
cli-option-help = 도움말을 출력합니다.
cli-option-version = 버전을 출력합니다.
cli-parse-error-guidance = 명령을 해석할 수 없습니다. 명령 이름, flag, 값을 확인하세요. `agent-finance --help` 또는 `agent-finance skills get core`를 실행할 수 있습니다.

price-summary-title = { $symbol } 가격 요약  조회={ $fetched }  시간대={ $timezone }
price-current = 현재가: { $currency } { $price }  세션={ $session }  출처={ $source }  변동={ $change }  시간={ $time }
price-current-missing = 현재가: 사용 가능한 호가가 없습니다
price-regular-basis = 정규장 기준: 전일종가={ $prevClose } 시가={ $open } 고가={ $high } 저가={ $low } 거래량={ $volume }
price-proxy = 프록시 가격: { $currency } { $price } via { $provider } 시간={ $time } 메모={ $note }
price-session-split-heading = 세션 / provider 구분
price-session-split-note = 참고: { $count }개의 세션/provider 행을 가져왔습니다. 자세한 구분은 sessions로 확인하세요.
price-errors-heading = 호가 오류
price-table-label = 라벨
price-table-price = 가격
price-table-change = 변동%
price-table-session = 세션
price-table-provider = provider
price-table-time = 시간
price-table-open = 시가
price-table-high = 고가
price-table-low = 저가
price-table-volume = 거래량

tui-settings-title = 설정 콕핏
tui-settings-clean = 변경 없음
tui-settings-workspace = 워크스페이스: { $workspace }
tui-settings-language-summary = 언어: { $language } ({ $locale })
tui-settings-dirty-config = 설정 변경: { $dirty }
tui-settings-watchlist = 관심 목록: { $count }개 종목  선택={ $selected }
tui-settings-trading-profile = 거래 profile: { $profile }  live writes={ $liveWrites }
tui-settings-submit-mode = 기본 제출 모드: { $default }  적용={ $effective }
tui-settings-provider-preferences = provider 설정: 주식={ $equity }  crypto={ $crypto }
tui-settings-theme = 테마: 강조색={ $accent }  선택={ $selectionBackground }/{ $selectionForeground }
tui-settings-provider-capability-profiles = provider capability profile: { $count }
tui-settings-normal-key-bindings = 일반 모드 키 바인딩: { $count }
tui-settings-editor-heading = 설정 편집기
tui-settings-pending = 저장 대기: { $change }
tui-setting-language = 언어
tui-setting-equity-provider = 주식 provider
tui-setting-crypto-provider = crypto provider
tui-setting-theme-accent = 테마 강조색
tui-setting-selection-background = 선택 배경
tui-setting-selection-foreground = 선택 글자색
tui-setting-key-command-palette = 키: 명령 팔레트
tui-setting-key-symbol-search = 키: 종목 검색
tui-setting-key-provider-details = 키: provider 상세
tui-setting-key-live-writes = 키: live writes
tui-setting-key-save-config = 키: 설정 저장
tui-setting-key-undo-config = 키: 설정 되돌리기
tui-setting-unknown = 알 수 없는 설정

tui-workspace-market = 시장
tui-workspace-trade = 거래
tui-workspace-account = 계정
tui-workspace-research = 리서치
tui-workspace-settings = 설정
tui-pane-status-fresh = 최신
tui-pane-status-loading = 로딩
tui-pane-status-partial = 부분
tui-pane-status-empty = 비어 있음
tui-pane-status-error = 오류
tui-pane-status-stale = 오래됨
tui-panel-watchlist = 관심 목록
tui-panel-quote = 호가 / 세션
tui-panel-order-ticket = 주문 티켓
tui-panel-open-orders = 열린 주문
tui-panel-intent-review = intent 검토
tui-panel-risk-audit = 리스크 / 감사
tui-panel-account = 계정
tui-panel-transfer-ticket = 이체 티켓
tui-panel-futures-state = 선물 상태
tui-panel-history = 히스토리 차트
tui-panel-evidence = crypto 근거
tui-panel-polymarket = Polymarket
tui-panel-research = 뉴스 / 리서치
tui-panel-provider-health = Provider 상태
tui-panel-task-log = 작업 로그
tui-panel-settings = 설정
tui-panel-profile-risk = Profile / 리스크
