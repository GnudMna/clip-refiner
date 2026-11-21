# アーキテクチャ

リポジトリとクレートの構成、モジュールの責務、スレッドモデルをまとめる。

## プロジェクト構成

```
clip-refiner/
├── Cargo.toml           # workspace ルート (virtual manifest / `workspace.dependencies`)
├── package.json         # UI JavaScript の ESLint / Prettier / Playwright E2E (本番バイナリには同梱しない)
├── eslint.config.js
├── e2e/                 # Playwright + WebView2 CDP による設定 / セレクター E2E
│   ├── fixtures.ts
│   ├── playwright.config.ts
│   └── *.spec.ts
├── crates/
│   ├── clip-refiner/    # デスクトップアプリ (バイナリ + tray)。core API は re-export しない
│   │   ├── src/
│   │   │   ├── bootstrap/   # 常駐 / ワンショット起動分岐 (args / logging / panic_hook / instance / oneshot / portable)
│   │   │   ├── ipc/         # ワンショット ↔ 常駐の名前付きパイプ IPC (`protocol` / `client` / `server`。`ClientError` は test-helpers / debug 向け)
│   │   │   ├── platform/    # デスクトップ常駐向け OS 連携 (通知・自動起動・OCR・WebView2・更新)
│   │   │   │   └── update/  # 確認 (check/) / 埋め込みキー復号 (bridge_key) / digest・署名検証 (signature) / MSI・ポータブル適用
│   │   │   ├── test_helpers/ # 統合テスト用 (`ClipboardHarness` / oneshot IPC / update など)
│   │   │   ├── tray/        # 常駐 UI・ワーカー・ホットキー・設定ウィンドウ
│   │   │   │   ├── features/    # トレイメニュー機能ドメイン (menu + event、`update.rs` 含む)
│   │   │   │   ├── menu/        # TrayMenu 型・全体構築・同期・履歴/連続記録 rebuild
│   │   │   │   ├── event/       # イベントループ制御・ルーティング
│   │   │   │   ├── selector/    # クイック/登録クリップセレクター WebView (retention 含む)
│   │   │   │   ├── settings/    # 設定ウィンドウ (model, ipc/, window/, apply/, snapshot など)
│   │   │   │   ├── worker/      # クリップボードワーカー・監視ループ・OS リスナ
│   │   │   │   └── ...
│   │   │   ├── logger.rs    # 日次ローテーション付きファイルログ
│   │   │   ├── user_error.rs # UpdateError 等から UserNotification を生成
│   │   │   └── ui/          # WebView UI (セレクター・設定ウィンドウ)
│   │   ├── tests/           # 統合テスト (`*_flow.rs`)
│   │   └── build.rs
│   └── clip-refiner-core/   # 加工ロジック・設定型 (GUI 非依存)。ライブラリ利用はこちら
│       ├── README.md    # ライブラリとしての利用
│       └── src/
│           ├── clip_template.rs # 登録クリップのテンプレート展開 (`{date}` など。feature 非依存)
│           ├── config/      # types/hotkeys/ ・ persistence/ (load/save) ・ serialize ・ spec (ドキュメント同期テスト) など
│           ├── native/      # DPAPI (`config`)・クリップボード画像と監視除外フォーマットの判定 (`clipboard`)。変換のみでは未コンパイル
│           ├── refiner/     # 変換モード・パイプライン・`OwnedClipboard`
│           │   └── transform/regex/  # キャッシュ / マッチタイムアウト / 実行パス / プレビュー
│           ├── security/
│           └── test_helpers/ # コア単体テスト用
├── .claude/rules/       # Rust ソースのコーディング規約 (comments.md / imports.md)
├── assets/
├── keys/
│   └── update-signing/  # 更新署名の minisign 公開鍵 (`*.pub`。build.rs が埋め込む)
├── scripts/
│   ├── common/          # 実装ヘルパー (cargo / npm / build-ui.mjs / wix / MSI 検証など)
│   ├── git/hooks/       # Git フック (pre-commit / pre-push)
│   ├── templates/       # THIRD_PARTY_NOTICES のテンプレート
│   └── windows/         # 開発者向けエントリポイント (check.ps1, format.ps1 など)
├── wix/                 # MSI 定義 (main.wxs)
├── about.toml           # cargo-about 設定
├── deny.toml            # cargo-deny 設定
├── CLAUDE.md            # Claude Code 向けの作業指針
├── CHANGELOG.md
├── README.md
└── docs/
    ├── user/            # 利用者向け (使い方・設定リファレンス・変換モード・セキュリティ・トラブルシューティング)
    └── dev/             # 開発者向け (環境構築・テスト・アーキテクチャ・リリース・開発規約)
```

第三者クレートと `clip-refiner-core` の版はルート `Cargo.toml` の `[workspace.dependencies]` に置く。メンバ側は `workspace = true` とし、`optional` や追加 `features` だけを各 `Cargo.toml` に書く。

`clip-refiner` の `platform/` はデスクトップ常駐向けの Win32 / WinRT 依存機能を `windows` クレート経由で集約する。`platform/update/` は gh-release-bridge の version API・アセットダウンロード (`CLIP_REFINER_KEY` を ChaCha20-Poly1305 で暗号化して埋め込み、利用時に復号して `Authorization: Bearer`)・`digest` (`sha256:<hex>`) 検証・minisign 署名検証 (`keys/update-signing/*.pub` を埋め込み)・宣言サイズ / 絶対上限でのダウンロード打ち切り・MSI / ポータブル ZIP 適用を担う (利用者向けは [アプリの更新](../user/usage.md#アプリの更新))。`clip-refiner-core` の `native/` は feature `config` の DPAPI と feature `clipboard` のクリップボード画像など、ライブラリ利用時にも必要な最小限のネイティブ OS API のみを置く (`default-features = false` ではコンパイルしない。`platform/` とは名前を分け、GUI 連携との混同を避ける)。`ipc/` は常駐プロセスがサーバー、`--mode` / `--pipeline` 実行側がクライアントとなり、パイプは現在ユーザー専用 DACL と `PIPE_REJECT_REMOTE_CLIENTS` で保護し、名前に `.<ログオンセッション ID>` を付けてセッションごとに分ける (多重起動防止の mutex がセッション単位のため。利用者向けは [ワンショット IPC](../user/security.md#ワンショット-ipc))。

セレクター UI (`src/ui/`) は WebView 上で動作する。責務は次のとおり。

| ファイル | 責務 |
| :------- | :--- |
| `selector-common.js` | エスケープ・検索正規化・ハイライト・選択移動など共有ユーティリティ (DOM 非依存 / ESM) |
| `quick_selector_logic.js` / `clip_selector_logic.js` | 各セレクターの一覧組み立て・フィルタ・行 HTML など (DOM 非依存 / ESM) |
| `quick_selector.js` / `clip_selector.js` | DOM 操作・IPC・キーボードイベント (ESM エントリ) |
| `quick_selector.bundle.js` / `clip_selector.bundle.js` | esbuild 生成 IIFE (編集禁止。`npm run build:ui` で再生成) |
| `selector.css` / `clip_selector.css` | スタイル |

純粋ロジックは Node.js 上の `selector_logic.rs` でも検証する (`import` で読み込む)。Rust 側 (`tray/selector/`) は CSS とバンドル JS をインライン展開して `WebView` に渡す (`embed_quick_selector_assets` / `embed_clip_selector_assets`)。設定 / セレクター `WebView` は埋め込み HTML (`NavigateToString`) のみを表示する。CSP で外部 script / connect / frame を禁止し、ナビゲーションハンドラで `http(s)` / `file` 等のページ遷移・`window.open`・ダウンロードを拒否する。初期読み込みは `about:blank` または埋め込んだ HTML と一致する `data:text/html;base64` のみ許可する (`selector/webview_policy.rs`)。

### `tray/` の責務分担

トレイ常駐 UI は **インフラ層** (`menu/`, `event/`, `worker/` など) と **機能ドメイン層** (`features/`, `settings/apply/patch/`) に分ける。新機能を追加するときは、まず `features/` にメニュー構築とクリック処理をまとめ、設定ウィンドウ連携が必要なら `settings/apply/patch/` にパッチ適用を追加する。

| モジュール (`tray/`) | 責務 |
| :------------------- | :--- |
| `features/` | トレイメニュー項目ごとのドメイン実装。各サブモジュールが `menu.rs` (構築・更新) と `event.rs` (クリック処理) を担う (`regex` 含む) |
| `menu/` | `TrayMenu` 型定義、`build.rs` (全体組み立て)、`sync.rs` (設定同期)、`rebuild.rs` (履歴 / 連続記録メニュー再構築の共通化)、`icon.rs` |
| `event/` | tao イベントループへの委譲 (`loop_event`, `user_event/` ルーター + ドメインハンドラ)、ホットキー・OCR・設定再読み込み、`handle_menu_event` ルーティング。`loop_event` 末尾でセレクター保持同期とアイドル起床 (`ControlFlow::WaitUntil`) |
| `selector/` | クイック / 登録クリップセレクターの WebView 束 (`SelectorUi`)。`retention.rs` が保持方針、`webview_policy.rs` が CSP と遷移許可を担う |
| `settings/` | 設定ウィンドウ (下表参照) |
| `worker/` | クリップボード I/O 集約・監視ループ。`clipboard_listener` (OS 通知)、`clipboard_seq` (シーケンストークン)、`config_watcher` (設定ディレクトリ変更通知)、`monitor_loop` / `handlers` / `handle` |
| `state/` | `AppState` (設定・横断操作) とサブ状態アクセサ (`worker` / `clipboard_session` / `config_sync` / `settings_pending`)、メニュー構築用の `MenuSnapshot` (必要フィールドのみ)、`AppEvent` (ドメイン別サブイベントのハブ) |
| `hotkey/` | グローバルホットキー登録・イベント変換 |
| `clipboard_monitor/` | 監視ループの判断・世代管理 |
| `notify/` | デスクトップ通知本文の組み立て (`NotificationSettings` 連動) |
| `history_store/` | 暗号化クリップボード履歴・連続記録のメモリストア (`EncryptedHistoryStore`) |
| `paste_watcher/` | 連続記録の貼り付け補助: キュー先頭書き込みロジックと `SendInput` による `Ctrl+V`。書き込み自体はワーカーの `PasteContinuousRecordFront` が担当 |
| `ocr/` | 画面 OCR キャプチャ連携 (`OcrJobGate` で同時 1 本・旧結果破棄) |
| `app/` | `App` 構造体と起動時初期化 |
| `runner.rs` | tao イベントループの起動と `App` への委譲 |
| `dispatch.rs` | ワーカー・イベントループへの送信ヘルパー |
| `e2e.rs` | `--e2e-window` ハーネス (Playwright E2E 専用) |

`features/` のドメイン一覧:

| サブモジュール | 対応するトレイメニュー / 操作 |
| :------------- | :---------------------------- |
| `refine` | 変換モード (自動変換の切り替えを含む)・お気に入り (`favorites.rs`) |
| `regex` | 正規表現プリセットのトレイ切替 (変換モード内のサブメニュー) |
| `monitor` | 監視方式の更新 (トレイメニューは持たず、設定ウィンドウから呼ぶ) |
| `clips` | 登録クリップ |
| `history` | 履歴メニュー (有効化・クリア・過去項目の選択) |
| `record` | 連続記録 |
| `update` | 更新確認・適用 (`platform/update/` をバックグラウンド起動。設定 UI から呼ぶ) |
| `app_control` | 終了・一時停止・クイック変換・設定ウィンドウ・監視再開 |

設定ウィンドウ (`src/ui/settings/`) は次のファイルに分割している。

| ファイル | 責務 |
| :------- | :--- |
| `main.js` | バンドルエントリ (初期化・IPC 受信。`ctx.render` / `ctx.refreshActivePanel` を代入) |
| `ctx.js` | 共有ミュータブル状態 (`ctx`)。描画関数の相互参照もここに集約し循環 import を避ける |
| `panels.js` | タブ描画 (DOM) |
| `events.js` | 入力イベント (DOM) |
| `shared.js` | 件数入力・パネルキャッシュ・サーバー主導フィールド同期 (DOM) |
| `html_builders.js` | タブ定義・パネル / カード / モード選択などの HTML 組み立て |
| `ipc_apply.js` | IPC 送信・設定パッチ debounce・適用フィードバック |
| `confirm_dialog.js` | 確認ダイアログ |
| `regex_feedback.js` | 正規表現プレビューの DOM 連携 |
| `regex_ui.js` | 正規表現タブのプリセット / ステップ HTML 組み立てと DOM 読み取り |
| `monitor_ui.js` | 監視バナー・加工パイプライン一覧の同期 |
| `shared_logic.js` | 件数クランプ・パイプライン正規化・通知パッチ組み立て・スナップショット再描画判定 (DOM 非依存 / ESM) |
| `panels_logic.js` | サイドバー (WAI-ARIA `tablist` / 矢印・Home / End) / クリップ行 / パイプライン操作・クリック委譲 (DOM 非依存 / ESM) |
| `settings_patch.js` | パッチ組み立て (DOM 非依存 / ESM) |
| `regex_preview.js` | プレビュー表示ロジック (DOM 非依存 / ESM) |
| `settings.bundle.js` | esbuild 生成 IIFE (編集禁止。`npm run build:ui` で再生成) |
| `css/*.css` | スタイル。`tray/settings/window/assets.rs` が `base` → `layout` → `fields` → `regex` → `controls` → `lists` の順に連結して埋め込む (カスケード順のため並びを変えない) |

`shared_logic.js` は Node.js 上の `settings_shared_logic.rs`、`panels_logic.js` は `settings_panels_logic.rs` でも検証する。Rust 側 (`tray/settings/window/assets.rs`) は `settings.bundle.js` をインライン展開し、シェル DOM ID・全タブ定義・`SettingsPanelsLogic` が含まれることをユニットテストする。読み書き項目は `tray/settings/model.rs` の `SettingsWritable` / `SettingsPatch`、初期表示は `snapshot.rs` の `SettingsSnapshot` が担う。IPC は `tray/settings/ipc/` (メッセージ / キュー / ディスパッチ)、パッチ適用は `tray/settings/apply/patch/` でドメイン別 (監視・履歴・連続記録・正規表現・加工・通知・システム・ホットキー・セレクター WebView) に分割されている。設定変更の反映はパッチ側が `AppConfig` を更新し、実際の `WebView` 破棄同期は `loop_event` の `sync_selector_retention` が担う。

| モジュール (`tray/settings/`) | 責務 |
| :---------------------------- | :--- |
| `model` | `SettingsWritable` / `SettingsPatch` (マクロ定義による読み書き項目の一元化) |
| `snapshot` | `SettingsSnapshot`, `SettingsLimits` (初期表示 JSON と UI 上限) |
| `ipc/` | WebView IPC: `message` (解析) / `queue` (適用待ちマージ) / `dispatch` (イベント送信) |
| `apply/` | パッチ適用のオーケストレーション (`patch/` でドメイン別、`feedback` で成功 / エラー通知。ホットキー補正時は割り当て名付きメッセージ) |
| `regex_preview` | 正規表現プレビュー IPC 応答 (UI から渡したサンプル文字列。未指定時は `REGEX_PREVIEW_SAMPLE`) |
| `geometry` | 設定ウィンドウのサイズ・位置 (`settings-window.json` の読み書き) |
| `window/` | WebView の組み立てと表示制御。`assets` (HTML/JS 埋め込み) / `events` (`SettingsEvent` 処理・更新 UI 通知) |

### アプリ更新 (`platform/update/`)

設定「一般」タブは `tray/features/update.rs` から確認・適用経路を呼ぶ。同時実行は `settings_pending` の更新オペレーション排他で 1 本に制限する。

| モジュール | 責務 |
| :--------- | :--- |
| `check/` | gh-release-bridge version API の取得 (`mod.rs`) と判定 (`evaluate`) / アセット選択 (`asset`) / SemVer (`version`)。アーキテクチャ不一致はエラー |
| `bridge_key` | ビルド時 `CLIP_REFINER_KEY` の ChaCha20-Poly1305 暗号化 blob を XOR 解除して復号する。`bridge_key_obfuscate` の XOR は `build.rs` と共有 |
| `verify` | ダウンロード (`CLIP_REFINER_KEY` があれば Bearer) と `digest` (`sha256:<hex>`) / サイズ検証。アセット名の検証 (`validate_asset_name`。ASCII 英数字と `.` `-` `_` の単一ファイル名に限り、予約デバイス名・200 バイト超を拒否。`check/evaluate` と `apply` の両方で呼ぶ)。宣言サイズ超過・絶対上限 (512 MiB) で途中打ち切り。`401` は認証失敗 |
| `signature` | `<asset>.minisig` の取得と minisign 署名検証。公開鍵は `build.rs` が `keys/update-signing/*.pub` から埋め込む。信頼コメント `clip-refiner-update version={version} file={asset}` が更新先と一致しなければ拒否する (古い版の正規アセットへのすり替え対策) |
| `apply` | 署名 → アセット本体の順に取得し、サイズ・SHA-256・署名を検証してから、MSI ならシステムフォルダ (`GetSystemDirectoryW`) の `msiexec.exe` を絶対パスで、ポータブル ZIP なら展開後に差し替えヘルパーを起動。署名検証に失敗したアセットは削除する |
| `replace` | `--apply-portable-update` ヘルパー (引数検証 → 旧 PID 待機 → 実行フォルダへコピー → 新版起動。`data\` は保持)。差し替え先は既存のポータブルフォルダ (`ClipRefiner.exe` + `portable` マーカー)、展開元と `--cleanup-dir` は `%TEMP%\clip-refiner-update\` 配下。生存中の `--wait-pid` は `ClipRefiner.exe` に限る。`validate` (引数検証)・`files` (探索・コピー)・`process` (PID 待機・削除予約・エラー通知) に分割 |
| `error` | `UpdateError` (`thiserror`)。ユーザー向け文言は `Display` に集約し、Outcome / 通知 / MessageBox へ渡す |
| `types` | `AvailableUpdate` / UI 応答型など |

確認・適用の失敗は `UpdateError` に集約する。トレイ / 設定 UI 側 (`tray/features/update.rs`) は `Display` 文字列を Outcome へ載せ、デスクトップ通知は `UserNotification` (`user_error.rs`) 経由で送る。

統合テストは `update_flow.rs` と `test_helpers/update.rs` でローカル HTTP モック bridge を立て、`ApplyOptions::TEST` により `msiexec` / 差し替えヘルパー / エクスプローラーを起動しない。隠し CLI `--apply-portable-update` (および `--wait-pid` / `--install-dir` / `--cleanup-dir`) は利用者向けではなく、ポータブル差し替えヘルパー専用である (`--e2e-window` と同様に help では非表示)。

### スクリプト構成

`scripts/windows/` は開発者が直接実行するエントリポイント。実処理は `scripts/common/` のヘルパーに集約する (PowerShell はドットソース、Node は `npm run` から呼ぶ)。

| `scripts/common/` | 役割 |
| :---------------- | :--- |
| `assert.ps1` | 外部コマンドの終了コード検証 (`Assert-LastExitCode`) |
| `cargo.ps1` | `cargo fmt` / `clippy` / `test` / `llvm-cov` のラッパー |
| `npm.ps1` | `npm ci` / UI バンドル / バンドルのコミットずれ検出 / ESLint / Prettier のラッパー |
| `build-ui.mjs` | 設定 / セレクター UI の esbuild IIFE バンドル (`npm run build:ui`) |
| `cargo-deny-check.ps1` | `cargo deny check` |
| `wix.ps1` | `cargo-wix` / WiX Toolset v3 の存在確認 |
| `windows-target.ps1` | x64 / ARM64 ターゲット解決、PE Machine / MSI Platform 検証 |
| `verify-msi.ps1` | MSI メタデータ・展開内容の検証ロジック |
| `generate-third-party-notices.ps1` | `THIRD_PARTY_NOTICES` 生成 |
| `release-version.ps1` | リリース時のバージョン更新ヘルパー |
| `update-signing.ps1` | 更新アセットの minisign 署名と埋め込み公開鍵での検証 (`release.ps1` から使用) |
| `cd-project-root.ps1` | リポジトリルートへ移動 |
| `wait-if-double-clicked.ps1` | エクスプローラー起動時の Enter 待ち (`Invoke-ScriptMain`) |

| `scripts/windows/` | 役割 |
| :----------------- | :--- |
| `check.ps1` | 品質チェック一式 (`-Coverage` でカバレッジ付きテストに切替可) |
| `format.ps1` | rustfmt + Prettier 整形 |
| `test.ps1` / `clippy.ps1` / `coverage.ps1` | 個別の Cargo コマンド (カバレッジは `cargo-llvm-cov`) |
| `check-pre-commit.ps1` | pre-commit 用 (`cargo fmt --check`) |
| `build.ps1` / `build-msi.ps1` / `build-portable.ps1` | ビルド・MSI / ポータブル ZIP 作成 |
| `verify-msi.ps1` | MSI 検証の CLI ラッパー |
| `release.ps1` | リリース手順一括実行 (MSI / ポータブル ZIP 含む) |
| `install-hooks.ps1` / `uninstall-hooks.ps1` | Git フック管理 |
| `generate-wix-assets.ps1` | WiX ブランド画像生成 |

## スレッドモデル

監視モード (引数なし起動) では、クリップボード操作を単一スレッドに集約し、UI は `tao` のイベントループ上で動作する。各スレッドの役割と通信経路は次のとおり。

```mermaid
flowchart TB
    subgraph main ["メインスレッド (tao Event Loop)"]
        App["App"]
        Tray["TrayMenu / tray-icon"]
        QS["QuickSelector (WebView)"]
        CS["ClipSelector (WebView)"]
        SW["SettingsWindow (WebView)"]
        Paste["連続記録貼り付け (SendInput Ctrl+V)"]
        App --> Tray
        App --> QS
        App --> CS
        App --> SW
        App --> Paste
    end

    subgraph hotkey_thread ["ホットキーリスナースレッド"]
        GHK["GlobalHotKeyEvent::receiver()"]
    end

    subgraph listener ["クリップボードリスナスレッド"]
        CFL["AddClipboardFormatListener"]
    end

    subgraph config_watch ["設定ディレクトリ監視スレッド"]
        CFW["FindFirstChangeNotification"]
    end

    subgraph ipc_server ["ワンショット IPC サーバスレッド"]
        Pipe["Named Pipe (現在ユーザー DACL)"]
    end

    subgraph worker ["クリップボードワーカースレッド"]
        WL["recv_timeout ループ"]
        Mon["監視ループ (Polling / Event)"]
        Cmd["ClipboardCommand 処理"]
        Owned["OwnedClipboard (本文 I/O)"]
        WL --> Mon
        WL --> Cmd
        Cmd --> Owned
        Mon --> Owned
    end

    CB[("OS クリップボード")]
    State[("AppState (Arc)")]
    CLI["ワンショット CLI (--mode / --pipeline)"]

    GHK -->|"HotkeyEvent::Received"| App
    App -->|"ClipboardCommand (mpsc)"| WL
    CFL -->|"ClipboardChanged (mpsc)"| WL
    CFW -->|"ConfigFileChanged (mpsc)"| WL
    Pipe -->|"ProcessOneshotPipeline"| WL
    CLI -->|"接続"| Pipe
    WL -->|"AppEvent (EventLoopProxy)"| App
    Owned <--> CB
    App <--> State
    WL <--> State
```

### 各コンポーネント

| コンポーネント | スレッド | 責務 |
| :------------- | :------- | :--- |
| **Event Loop** (`tray/runner.rs`) | メイン | ウィンドウイベント、トレイメニュー、WebView IPC (セレクター・設定ウィンドウ)、`AppEvent` の受信と `App::handle_event` への振り分け。連続記録貼り付けでは `RecordEvent::SendPaste` を受けて `SendInput` で `Ctrl+V` を送る |
| **Hotkey Listener** (`tray/hotkey/`) | 専用 | `global-hotkey` のイベントをブロッキング受信し、`EventLoopProxy` 経由で `HotkeyEvent::Received` をメインスレッドへ送る。連続記録貼り付け (`Alt+Shift+X`) もここで受信し、メインスレッドの `HotkeyHandler` が `PasteContinuousRecordFront` をワーカーへ送る |
| **Paste helper** (`tray/paste_watcher/`) | (ヘルパー) | キュー先頭のクリップボード書き込みロジックと `Ctrl+V` の `SendInput`。専用スレッドは持たない |
| **Clipboard Listener** (`tray/worker/clipboard_listener.rs`) | 専用 | メッセージ専用ウィンドウで `AddClipboardFormatListener` を登録し、`WM_CLIPBOARDUPDATE` を `ClipboardCommand::ClipboardChanged` としてワーカーへ送る。連打は未処理フラグで束ねる。初期化失敗時はワーカーがシーケンストークンの短周期 peek にフォールバックする |
| **Config Watcher** (`tray/worker/config_watcher.rs`) | 専用 | 設定ディレクトリを `FindFirstChangeNotification` で監視し、`ConfigFileChanged` でワーカーを起床する。変更の有無は `config_sync` の本文指紋 (BLAKE3) で判定する。初期化失敗時は約 2 秒ごとの指紋確認にフォールバックする |
| **Oneshot IPC Server** (`ipc/`) | 専用 (+ 接続ごとの一時スレッド) | 名前付きパイプでワンショット要求を受け、`ProcessOneshotPipeline` をワーカーへ委譲する。待受スレッドは接続を一時スレッドへ渡し、処理中の接続を保持したまま次インスタンスを作って待受を続ける (同時処理は `MAX_ACTIVE_CONNECTIONS` = 4。上限中は枠が空くまで次の接続を保留)。要求 1 行の読み取りは `REQUEST_READ_TIMEOUT` (5 秒) で打ち切り、`PeekNamedPipe` で到着済みの分だけを読むため、要求を送らない接続が枠を占有し続けない。手持ちインスタンスが無いときだけ `FILE_FLAG_FIRST_PIPE_INSTANCE` で作成し、名前の先取りを避ける。現在ユーザー専用 DACL + リモート拒否。パイプ名は `ONESHOT_IPC_PIPE_NAME` + `.<セッション ID>` で、CLI は `GetNamedPipeServerSessionId` で作成元が同じセッションか確かめる。接続・送受信失敗は `ClientError`。待受スレッド起動失敗は `Result` で返し、常駐は監視を継続したまま通知する。常駐未検出時は CLI 側がローカル実行する |
| **Clipboard Worker** (`tray/worker/`) | 専用 | すべてのクリップボード読み書き (`OwnedClipboard`)、監視ループ、設定ファイルの外部変更検知。`recv_timeout` でコマンド受信と監視チックを交互に処理。Event / 連続記録かつリスナ稼働時は `ClipboardChanged` / `ConfigFileChanged` で起床する。再起動・破棄時は `Shutdown` 後にリスナ・設定監視とワーカーを join してから戻る |
| **AppState** (`tray/state/`) | 共有 (`Arc`) | 設定 (`RwLock`)、監視世代、イベントプロキシ。履歴・Undo・二重加工防止は `clipboard_session()`、ワーカー稼働は `worker()`、設定ディスク同期は `config_sync()`、設定 UI 適用待ちは `settings_pending()` 経由 |

### UI の遅延生成

クイックセレクター・登録クリップセレクター・設定ウィンドウの各 `WebView` と画面 OCR オーバーレイは起動時には生成せず、初回表示時に作成する。起動時は空シェル (`SettingsWindow::new_shell`・`init_quick_selector` など) のみ保持する。セレクターの非表示後の保持は `selector_webview_retention` で選べる (`Keep` 既定で保持、`DisposeOnHide` で非表示時破棄、`DisposeAfterIdle` で N 分アイドル後に破棄)。非表示は `hide_with_retention`、方針変更後の同期と期限到来破棄は毎ループの `sync_selector_retention`、アイドル起床は `apply_idle_control_flow` (`ControlFlow::WaitUntil`) が担う。設定ウィンドウと画面 OCR オーバーレイは非表示時に破棄する (アイドル時の RSS 削減)。遅延生成のため `App::handle_event` と `SettingsWindow::show` は `EventLoopWindowTarget` を受け取り、OCR オーバーレイの破棄は `OcrEvent::DisposeOverlay` を経由する。ライフサイクルは `selector/{quick,clip,retention}.rs`、`settings/window/`、`tray/ocr/capture.rs`、`platform/ocr_overlay/overlay_window.rs` のユニットテストで検証する。

### 通信の要点

- **UI → ワーカー**: `ClipboardCommand` を `mpsc` チャネルで送信 (手動加工、履歴復元、登録クリップコピー、連続記録貼り付けの書き込みなど)
- **リスナ → ワーカー**: `ClipboardCommand::ClipboardChanged` で Event / 連続記録の監視を起床する
- **設定監視 → ワーカー**: `ClipboardCommand::ConfigFileChanged` で外部編集の再読み込み判定を起床する。本文指紋が最終同期と異なるときだけ `ConfigEvent::Reload` を送る
- **ワンショット CLI → 常駐**: 名前付きパイプで加工要求を送り、サーバーが `ProcessOneshotPipeline` をワーカーへ渡す (監視エコー回避)
- **ワーカー → UI**: `EventLoopProxy<AppEvent>` でユーザーイベントを送る (例: 設定再読み込み完了、`RecordEvent::SendPaste`)
- **ホットキー → UI**: 専用スレッドが `HotkeyEvent::Received` を送り、メインスレッドの `HotkeyHandler` が処理
- **クリップボード**: 本文の読み書きはワーカースレッド内の `OwnedClipboard` 1 インスタンスに集約する (常駐なしワンショットのみ短命に別インスタンスを開く)。連続記録貼り付けもワーカーが書き込み、メインスレッドは `SendInput` のみ担当する。変更検知は役割を分ける
  - `clipboard_seq` / `AddClipboardFormatListener`: `OpenClipboard` なし (または本文非読) の変更通知
  - `OwnedClipboard` (`arboard`): テキスト / 通常画像 / HTML 形式 (`CF_HTML`) の `OpenClipboard` I/O。HTML 形式の有無は `IsClipboardFormatAvailable` で開かずに判定する
  - `native/clipboard_image` (`CF_DIB` / `CF_DIBV5`): Excel 向け短命 Open。同一呼び出しで `arboard` と重ねて開かない

加工ロジック自体 (`refiner/`) は基本的にスレッドを持たず、ワーカーまたはワンショット実行 (`bootstrap/`) から同期的に呼ばれる。例外として正規表現マッチは実行時間上限のために一時スレッドを使うが、同時実行は最大 2 本 (`REGEX_MATCH_MAX_IN_FLIGHT`) に制限する。タイムアウト時はキャンセルを要求し、実行側はマッチ境界で中断してスロットを返す。`regex` クレートは単一の `find` 実行中を打ち切れないため、その探索が終わるまでスレッドが残る場合がある。画面 OCR も範囲確定後に一時スレッドでキャプチャ・認識するが、`OcrJobGate` により同時実行は 1 本に限り、新しい要求で旧ジョブの結果適用を破棄する。

### デスクトップ通知

通知の呼び出しは 2 層に分ける。

| 層 | モジュール | 責務 |
| :--- | :--------- | :--- |
| **platform** | `platform/notify.rs` | Windows トーストの初期化と送信 (`init_notifications`, `show_notification`) |
| **platform** | `platform/webview2.rs` | WebView2 Runtime のレジストリ検出、未導入時の起動通知、導入ページオープン |
| **platform** | `platform/update/` | 更新確認・digest / 署名検証・MSI / ポータブル適用。失敗は `UpdateError`。通知文言は呼び出し元が `UserNotification` 経由で送る |
| **tray** | `tray/notify/message.rs` | 加工完了・一時停止など設定連動の本文組み立て (`show_process_notification`, `show_when_enabled` など) |
| **app** | `user_error.rs` | 型付きエラー (`UpdateError` / core エラーなど) から `UserNotification` (タイトル・本文) を生成し、ログとデスクトップ通知を一元化 |
| **features** | `tray/features/update.rs` | 更新確認 / 適用のバックグラウンド起動と UI / `UserNotification` への結果反映 |

- エラー表示・設定読み込み失敗など固定メッセージは、呼び出し元から `platform::show_notification` を直接使うか、`UserNotification` 経由で送る (`NotificationSettings.enabled` を経由しない)
- 監視モードの加工完了は `tray/notify` が `NotificationSettings` を反映して本文を組み立て、最終的に `platform::show_notification` へ渡す
- アプリ更新の失敗・案内は `UpdateError::Display` → `UserNotification` → `platform::show_notification` の経路に揃える
- 起動時は `bootstrap/` で `init_notifications` を呼び、AUMID とアイコンを登録してから各所でトーストを送る

`NotificationSettings` (`clip-refiner-core/src/config/types/notification.rs`) はマスター (`enabled`) と種類別トグル (`notify_process` など) を保持する。旧 `notify_mode` / `notify_actions` は `migrate_legacy_fields` で読み込み時に移行し、保存時は出力しない。利用者向けの説明は [通知](../user/configuration.md#通知-notification_settings) を参照。

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存される。ファイル名の日付はローカルタイムゾーン基準 (`LocalDailyFileAppender`)。保存場所は [ログ](../user/configuration.md#ログ) を参照。

ログレベルは環境変数 `RUST_LOG` で制御できる (例: `RUST_LOG=debug`)。

| ビルド種別 | 出力先 |
| :--------- | :----- |
| デバッグビルド (`cargo build` / `cargo run`) | ログファイル + 標準出力 |
| リリースビルド | ログファイルのみ |

通常のログは非同期キュー経由で書き込む。パニックは `bootstrap/panic_hook.rs` のフックが発生スレッド・メッセージ・位置をログファイルへ**同期で**書き込み、その後に既定フックへ委譲する (GUI サブシステムでは標準エラーの出力先が無く、abort で非同期キューも失われうるため)。`RUST_BACKTRACE=1` を指定するとバックトレースも記録する。
