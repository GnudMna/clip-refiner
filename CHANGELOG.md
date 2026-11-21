# 変更履歴

このファイルは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) の形式に従う。

バージョン番号は [Semantic Versioning](https://semver.org/lang/ja/) に準拠する。各リリースの **設定** 節は `config.toml` の `version` (設定スキーマ) に関する変更を示す。

## [Unreleased]

## [0.14.0] - 2026-07-26

### Added

- アプリ更新: トレイ「更新を確認…」と設定「一般」タブから [gh-release-bridge](https://gh-release-bridge.gnudlemna-4656.workers.dev) 経由で最新リリースを確認し、MSI は `msiexec` 起動、ポータブルは ZIP 展開後に差し替えヘルパー (`--apply-portable-update`) で実行フォルダへ自動適用して再起動する (`data\` は保持。失敗時のみ手動差し替え案内)
- アプリ更新の統合テスト (`update_flow`): ローカル HTTP のモック bridge で version API → ダウンロード → サイズ / SHA-256 検証 → ポータブル ZIP 展開 (および MSI 検証まで) を固定する。digest 欠落・ハッシュ / サイズ不一致・API 404 もカバーする
- ポータブルモード: 実行ファイルと同じフォルダに `portable` マーカーを置くと、設定・ログ・登録クリップを `<exe>\data\` へ保存する。FAT / exFAT など ACL 非対応ボリュームでは DACL 設定失敗を警告のみとし起動・保存を継続する
- ポータブル配布物作成スクリプト `scripts/windows/build-portable.ps1` (`ClipRefiner.exe` + `portable` + ライセンス類を `target/portable/` へ出力)
- WebView2 Runtime 未導入時の案内: 起動時にデスクトップ通知で不足を伝え、設定 / セレクター表示失敗時は導入ページを既定ブラウザで開く (監視・ホットキー・ワンショットは継続利用可)

### Security

- アプリ更新: gh-release-bridge の version API アセットに含まれる `digest` (`sha256:<hex>`) を必須とし、ダウンロード後にサイズと SHA-256 を検証してから MSI / ポータブル ZIP を適用するようにした。digest 欠落や不一致時は適用しない

### Changed

- 肥大モジュールを分割した: `refiner/transform/regex/` (キャッシュ / マッチタイムアウト / 実行パス / プレビュー)、`tray/settings/ipc/` (メッセージ / キュー / ディスパッチ)、`tray/settings/window/` (アセット組み立て / イベント処理)。公開 API と挙動は変更なし
- 設定 UI の JavaScript を ESM (`import`/`export`) 化し、esbuild で `settings.bundle.js` へ IIFE バンドルするようにした。Rust は生成物を埋め込み、DOM 側は ESLint でファイル間参照を解決できる。共有状態は `ctx.js` に集約。`flushPendingApply` の OS 呼び出しも `window.` 付きに揃えた
- トレイメニュー・設定 UI・ドキュメントの用語を「変換モード」に統一した (文書・コメントの旧称「加工モード」。カテゴリ名の「日時変換」等はそのまま)
- 機密マスキングが**表示置換のみ**であること (クリップボード本体・履歴復元・登録クリップコピーは対象外) を設定 UI のヒントと README / CONFIG で明示した
- アプリ更新のアセット選択で、CPU アーキテクチャ不一致時に先頭候補へフォールバックせずエラーにするようにした
- トレイ単体テストの共通初期化を `TrayTestContext` に集約し、メニューイベント経路の重複を削減した
- OCR オーバーレイ / トレイメニュー経路の手動スモークを Rust 自動テストへ移行し、手動確認範囲をトレイ実クリック・ホットキー競合・トースト見た目・`SendInput` 実貼り付け・更新の外部プロセス起動に縮小した
- ドキュメント (README / CONFIG / DEVELOPMENT) をアプリ更新・ポータブル差し替え・モジュール分割に合わせて更新した

## [0.13.1] - 2026-07-18

### Security

- ワンショット IPC の名前付きパイプを現在ユーザー専用 DACL で作成するようにした。既定 ACL (Everyone 読み取り・管理者フルコントロール等) をやめ、他ユーザーからのローカル接続を拒否する (`PIPE_REJECT_REMOTE_CLIENTS` は従来どおり)。脅威モデルは [CONFIG.md](CONFIG.md#ワンショット-ipc) に記載

### Added

- ワンショット IPC の実パイプ往復統合テスト (`oneshot_ipc_flow`)。テスト専用パイプ名で待受し、成功 / 変更なし / 正規表現上書き / 空パイプライン / 不正 JSON / ワーカー停止 / 過大要求破棄 / 連続委譲を固定する

### Changed

- 画面 OCR のデフォルトホットキーを `Alt+Shift+O` から `Alt+Shift+G` (`hotkeys.ocr`) へ変更。既存の `config.toml` に保存済みの値がある場合は維持し、キー欠落時のみ新既定を適用する
- Event 監視を `AddClipboardFormatListener` による OS 通知駆動へ変更。専用リスナスレッドが `WM_CLIPBOARDUPDATE` でワーカーを起床し、アイドル時のシーケンストークン周期 peek をやめた。リスナ初期化失敗時のみ従来の短周期 peek にフォールバックする
- クリップボード本文 I/O を `OwnedClipboard` に集約し、変更検知 (`seq_num` / リスナ) と Excel 向け `CF_DIB` 短命 Open の責務を分離した。画像取得では同一呼び出しで `arboard` と `clipboard-win` を重ねて開かない

## [0.13.0] - 2026-07-13

### Fixed

- トレイメニューイベントをメインループの毎回で全件処理するようにした。設定ウィンドウ表示中など WindowEvent が続く間にメニュー操作が遅延・滞留する問題を解消
- クリップボードワーカー再起動時に固定 `sleep(50ms)` ではなく `JoinHandle` で旧スレッド終了を待つようにした。ワーカーはハンドルへの `Arc` を保持せず、`Drop` / `restart` で確実に join して二重監視とテスト時のヒープ破壊を防ぐ
- 設定パッチのマージ失敗時に既存キューを破棄せず保持するようにした。不正 JSON 投入で先行変更が消える問題を解消
- 設定 UI (JS) のパッチマージで `null` を未指定としてスキップし、Rust 側 (`deep_merge_json_values`) と仕様を揃えた
- トレイ常駐中でも `--mode` / `--pipeline` のワンショットを実行できるようにした。常駐中は名前付きパイプ IPC で委譲し、監視の書き戻しエコー再加工を避ける。常駐が無い場合のみローカルで直接加工する。多重起動防止はトレイ常駐起動のみに適用する

### Changed

- 連続記録の貼り付けを `Ctrl+V` 捕捉から専用ショートカット `Alt+Shift+X` (`hotkeys.continuous_record_paste`) へ変更。通常の `Ctrl+V` は OS / AutoHotkey 等に任せ、キュー先頭の取り出し後に `SendInput` で `Ctrl+V` を送る
- 連続記録貼り付けのクリップボード書き込みをワーカーへ集約。メインスレッドは書き込み完了後の `SendInput` (`Ctrl+V`) のみ担当し、監視ループとの二重接続を解消
- 画面 OCR のキャプチャ・認識を同時 1 本に直列化し、新しい範囲選択で旧ジョブの結果書き込み・通知を破棄するようにした。ホットキー連打時の並列 OCR とクリップボード競合を抑制する
- `config.toml` / 登録クリップの保存を一時ファイル経由の原子的置き換えに変更し、書き込み途中のクラッシュによる破損を防ぐ

### Added

- 連続記録の直近 1 件取り消し: トレイ「連続記録 → 直近の記録を取り消す」および `Alt+Shift+Backspace` (`hotkeys.continuous_record_undo`)。キュー末尾のみ削除し、クリップボードは変更しない
- `config.toml` の認識できないキーを検出して警告する。読み込みは継続し、起動時・再読み込み時にログとデスクトップ通知 (ワンショットは stderr) でキー名を案内する。旧キーや別名は警告しない
- 設定ファイルの保存失敗・設定パッチのキュー投入失敗をデスクトップ通知で案内する

## [0.12.6] - 2026-07-12

### Fixed

- 設定ウィンドウをタイトルバーの閉じるボタン (バツ) から閉じられない不具合を修正。`closeWindow` を `window` へ公開し、OS の閉じる操作から呼び出せるようにした
- 設定 UI「ショートカット」タブの説明文を編集可能である実態に合わせて更新
- ログの日次ローテーションを UTC からローカル日付基準へ変更。長時間常駐時にローカル日付と異なるファイルへ書き続けられる問題を解消
- E2E fixture のホットキー (`continuous_record`) を本番デフォルト (`Alt+Shift+C`) に合わせ、実スキーマに無い `[regex] enabled` を削除

### Added

- 設定 UI の閉じる IPC (`apply`→`close`)・Escape 判定・ショートカット差分更新を `shared_logic.js` へ抽出し Node テストで固定
- 設定 IPC → `config.toml` 往復の統合テスト (`SettingsTestContext::with_persistence`)
- 正規表現プレビューのタイムアウト表示と設定ウィンドウ幾何 JSON の往復テスト
- Playwright + WebView2 CDP による設定 / クイックセレクター / 登録クリップセレクターの E2E (`e2e/`)。`--e2e-window` ハーネスと `check.ps1` 組み込み (`CLIP_REFINER_SKIP_E2E=1` でスキップ可)
- E2E に設定の debounce 保存・外部 `config.toml` 変更バナー、登録クリップの空表示・検索絞り込みを追加
- OCR オーバーレイの生成・表示・非表示ライフサイクルを Rust テストで固定

### Changed

- 設定 UI の肥大化した `shared.js` を責務別に分割 (`html_builders.js` / `ipc_apply.js` / `confirm_dialog.js` / `regex_feedback.js` / `monitor_ui.js`)。挙動は維持し、`assemble_settings_js` の連結順のみ更新
- 正規表現マッチの同時実行を最大 2 本に制限。タイムアウト時はキャンセルを要求し、マッチ境界で中断してスロットを解放する (単一の `find` 実行中はエンジン制約で打ち切れない)
- `AppState` の薄い委譲メソッドを削除し、`worker()` / `clipboard_session()` / `config_sync()` / `settings_pending()` アクセサ経由に呼び出しを移行。横断処理 (`save_config` / `add_to_history` 等) のみ `AppState` に残した
- WebView 回帰検知を 3 層から 4 層 (JS 純粋 / Rust IPC / Playwright E2E / 手動) へ拡張。手動スモークはトレイ・ホットキー・OCR ドラッグ・外部連携に縮小
- ドキュメント (README / CONFIG / DEVELOPMENT) を現状に合わせて更新・整理。連続記録の有効化直後の挙動矛盾を解消し、処理上限・設定ディレクトリ構成・環境変数・E2E 構成を追記

## [0.12.5] - 2026-07-11

### Changed

- 設定ウィンドウの監視周期を 4 択セレクトから 100〜60000 ms の数値入力へ変更。トレイは従来プリセットに加え「カスタム…」から設定ウィンドウを開き、プリセット外の値は `カスタム (N ms)` として表示
- クイックセレクター・登録クリップセレクター・設定ウィンドウの各 `WebView` と画面 OCR オーバーレイを起動時常駐から遅延生成へ変更。初回表示時に生成し非表示時に破棄することで、アイドル時のメモリ使用量 (RSS) を削減
- 未使用の内部 API とテスト専用コードを `cfg(test)` へ限定して整理 (公開 API に影響なし)
- 依存クレートを更新

## [0.12.4] - 2026-07-10

### Added

- 正規表現マッチに実行時間上限 (2 秒) を追加。超過時は加工を中断して元テキストを維持し、設定 UI プレビューではタイムアウトとして表示
- 設定 / セレクター WebView の回帰検知を強化: スナップショット再描画判定の純粋ロジック化、組み立て HTML/JS のシェル・タブ検証、IPC パッチ統合テスト拡充、`DEVELOPMENT.md` に設定・セレクターの手動スモークチェックリスト
- OCR / 画面キャプチャの直接テストを追加: 空入力検証、BGRA↔RGBA 変換、`SoftwareBitmap` 生成、仮想画面小領域キャプチャ、OCR パイプライン完走 (エンジン未導入時は初期化失敗を許容)
- 設定ウィンドウの「ショートカット」タブでホットキーを編集可能に (固定割り当て・お気に入りスロット。保存時に不正・重複を正規化し OS へ再登録)
- セレクター UI の Node.js ロジックテスト `selector_logic` (`selector-common.js` / `quick_selector_logic.js` / `clip_selector_logic.js`)
- 設定 UI の Node.js ロジックテスト `settings_panels_logic` (`panels_logic.js`: サイドバー / クリップ行 / パイプライン操作・クリック委譲)

### Changed

- 連続記録の `Ctrl+V` 検知を低レベルキーボードフック (`WH_KEYBOARD_LL`) から、キューがあるときだけ登録する `RegisterHotKey` + `SendInput` に変更。全キー入力の常時フックを廃止
- WebView2 初期化を再試行し、ユーザーデータロック時はプロセス固有ディレクトリへフォールバック。終了直後の再起動で `E_INVALIDARG` になりにくくした
- セレクター UI の DOM 非依存ロジックを `quick_selector_logic.js` / `clip_selector_logic.js` へ抽出し、`quick_selector.js` / `clip_selector.js` は DOM 操作に限定
- 設定 UI のパネル / イベント純粋ロジックを `panels_logic.js` へ抽出し、`panels.js` / `events.js` は DOM 操作に限定

## [0.12.3] - 2026-07-08

### Fixed

- 連続記録を ON にした直後、既にクリップボードにあるテキストが二重に記録される不具合を修正。ON 以降にコピーした内容のみキューへ積む

## [0.12.2] - 2026-07-08

### Added

- 通知設定を種類別に再構成: マスター ON/OFF (エラーは常に表示)、加工完了・一時停止・履歴復元・OCR・登録クリップ・お気に入り・連続記録の個別トグル、クリップボード内容表示を別枠に分離
- Git フック (`scripts/git/hooks/`) と有効化スクリプト `install-hooks.ps1` / `uninstall-hooks.ps1` (pre-commit: `cargo fmt --check`、pre-push: `check.ps1`)
- リリース手順一括スクリプト `scripts/windows/release.ps1` (バージョン更新・CHANGELOG・品質チェック・Git コミット・MSI 作成、`release/v{version}` ブランチ必須)
- MSI パッケージ自動検証 `verify-msi.ps1` (`build-msi.ps1` 完了後にメタデータ・同梱ファイル・展開内容を検証)
- ESLint / Prettier による UI JavaScript の静的解析とフォーマット (`package.json`, `eslint.config.js`; `check.ps1` / `format.ps1` に組み込み)
- 未登録だった統合テスト 4 件 (`worker_recovery_flow` / `oneshot_flow` / `settings_ui_logic` / `regex_preview_logic`) を `Cargo.toml` に登録
- 通知設定の統合テスト `notification_flow.rs`、OS 連携テスト `platform_flow.rs`、設定 UI 共有ロジックテスト `settings_shared_logic.rs`
- 加工変換のプロパティベーステスト `property_transforms.rs` (`proptest`)
- 設定 UI 純粋ロジック `shared_logic.js` (件数クランプ・パイプライン正規化・通知パッチ組み立て)
- JavaScript ロジックテスト用 `run_js_test_script` ヘルパー (`check.ps1` では `CLIP_REFINER_REQUIRE_NODE=1` で必須化)

### Changed

- 旧設定 `notify_mode` / `notify_actions` は読み込み時に新項目へ自動移行
- PowerShell スクリプトの共通処理を `scripts/common/` ヘルパー (`cargo.ps1`, `npm.ps1`, `wix.ps1`, `assert.ps1`) へ集約
- トレイ実装を `features/` (ドメイン別メニュー・イベント) と `selector/` (クイック/登録クリップセレクタ) へ再構成。暗号化履歴データ層を `history_store` へ改名
- `hotkey` / `dispatch` のユニットテストを本番モジュールから分割 (`too_many_lines` 抑制を除去)
- `cargo-about` 生成時の自クレート警告を `about.toml` の `private.ignore` で解消

### Fixed

- 設定ウィンドウでデスクトップ通知マスター OFF 時に個別通知設定を変更できない問題

## [0.12.1] - 2026-07-07

### Added

- 統合テスト `worker_recovery_flow.rs` (クリップボードワーカーの起動・復旧)、`settings_ui_logic.rs` (設定 UI パッチマージ JS)、`oneshot_flow.rs` (ワンショット相当パイプライン)

### Changed

- 連続記録 ON 時はトレイメニュー・ツールチップ・設定ウィンドウでコピー時の自動加工停止を明示 (変換モードメニューは無効化、監視タブの加工設定は無効表示)
- 設定ウィンドウ: DOM 更新と適用エラー通知を改善
- 設定パッチ適用をドメイン別サブモジュール (`tray/settings/apply/patch/`) へ分割
- 設定ウィンドウの読み書きモデル (`SettingsWritable` / `SettingsPatch`) と正規表現プレビュー用サンプル文字列 (`REGEX_PREVIEW_SAMPLE`) を一元化
- Windows API バインディングを `windows` クレートへ統一
- セレクター HTML のインライン JS を `quick_selector.js` / `clip_selector.js` へ抽出
- WebView UI の JavaScript に JSDoc を追加

### Fixed

- 連続記録 ON かつ一時停止 OFF でも監視経路で自動加工が実行される問題

## [0.12.0] - 2026-07-06

### Added

- WebView ベースの設定ウィンドウ (トレイ「設定を開く」): 一般・お気に入り・正規表現・履歴・連続記録・登録クリップ・監視・通知・ショートカットの 8 タブ。変更は debounce 後に自動保存・即時反映
- 設定ウィンドウの正規表現タブで Rust エンジンによるライブプレビュー
- 統合テスト `settings_flow.rs` と `SettingsTestContext` ヘルパー
- 設定 UI の正規表現プレビュー JS ロジックテスト (`regex_preview_logic.rs` / Node.js)

### Fixed

- パイプライン非空時にデフォルトモード変更が保存されない問題
- 正規表現プレビュー応答と DOM 更新のレース
- 設定ウィンドウ非表示後も適用エラーをデスクトップ通知で表示
- 設定ウィンドウからの `interval_ms` パッチがポーリング切替時に無視される問題
- `interval_ms` パッチ適用時のメモリ上 `normalize` 漏れ

## [0.11.0] - 2026-07-04

### Added

- クリップボード連続記録 (`continuous_record_enabled`、トレイ「連続記録 → 連続記録を有効にする」/`Alt+Shift+C`): 履歴とは独立した FIFO キュー。コピーで元テキストを末尾へ積み、連続記録 ON 時の `Ctrl+V` 貼り付けでキュー先頭をクリップボードへ載せてから通常の貼り付けを行う。トレイメニューから指定項目を取り出すこともできる。キューはメモリ上のみ暗号化保持
- MSI パッケージへの `THIRD_PARTY_NOTICES` 同梱 (OSS 依存の帰属・ライセンス全文)

### Changed

- **対応 OS を Windows のみに統一** (Linux / macOS のビルド・パッケージ・OCR・クリップボード監視を廃止)
- Windows 専用化に伴い、クロスプラットフォーム用の `#[cfg]`・スタブ・`windows` 子モジュール名を整理
- OS 連携を `platform/` 配下へ集約 (`autostart`、通知、OCR、画面キャプチャなど)
- デスクトップ通知を WinRT トースト (`notify.rs`) に統一
- 開発時の依存関係監査に `cargo-deny` を導入 ([DEVELOPMENT.md](DEVELOPMENT.md#サプライチェーン監査-cargo-deny))

### Removed

- **Linux 対応を廃止** (deb パッケージ、X11 クリップボード監視、Tesseract OCR、XDG autostart など)
- **macOS 対応を廃止** (DMG、Apple Vision OCR、LaunchAgent、WebView OCR オーバーレイなど)
- 未使用の `notify-rust` 依存と公開 API `config::home_dir`

## [0.10.0] - 2026-07-02

### Added

- 画面 OCR を macOS (Apple Vision) / Linux (Tesseract) で利用可能に (`Alt+Shift+O`、WebView オーバーレイ。Linux は `xcap` による領域キャプチャ)
- `excel-to-image` 変換モードを macOS / Linux でも利用可能に (Windows 専用の `CF_DIB` フォールバックは Windows のみ)
- ライブラリ API: `config` モジュールの公開、加工パイプライン API (`apply_text_pipeline`, `apply_pipeline_to_text`, `split_pipeline`)、クレートルート re-export、`RefineContext::with_regex`
- `clip-refiner-core` クレートで加工ロジックと設定型を GUI 非依存に分離 (`crates/` 配下)
- feature `app` (デフォルト) でトレイ UI / CLI をオプション化。ライブラリ利用は `default-features = false` または core 直接依存
- [CONFIG.md](CONFIG.md) を追加し、設定リファレンス・変換モード一覧・UI 操作・画面 OCR・履歴セキュリティ・使用例を README から分離
- [CHANGELOG.md](CHANGELOG.md) を追加 (Keep a Changelog 形式)
- [DEVELOPMENT.md](DEVELOPMENT.md) にスレッドモデル (Event loop / クリップボードワーカー / ホットキー) の説明を追記

### Changed

- ワークスペースを `crates/clip-refiner` と `crates/clip-refiner-core` に再構成 (ルートは virtual manifest)
- README を概要・クイックスタート中心に簡素化
- プラットフォーム別の変換モード制限を廃止 (`RefineMode::is_supported_on_current_platform` / `normalize_platform_modes` を除去)
- 設定 `version` がアプリより新しい場合、全項目のデフォルト化ではなく `version` を現行スキーマへクランプして既存値を保持
- クリップボードワーカー初期化失敗時に UI へ通知し、自動復旧を試みるように変更
- OCR 前処理 (小画像の拡大・日本語空白除去) を共通化
- 登録クリップのドキュメント記述を `registered-clips.dat` 暗号化保存 (設定 v2) に合わせて修正
- `tray/hotkey` を解決・登録・イベント処理のサブモジュールへ分割。`HotkeySettings` の UI 向けメソッドを `tray/hotkey/settings.rs` へ移動
- `platform/ocr_overlay/windows` を型・座標変換・描画・ウィンドウプロシージャのサブモジュールへ分割

### Fixed

- クリップボード初期化失敗後に監視だけ停止しトレイ常駐が続く状態を解消

## [0.9.0] - 2026-07-01

### Added

- 登録クリップへの画像登録と、登録クリップセレクタでのサムネイルプレビュー

### Changed

- セレクター UI の共通 JS / CSS を `selector-common.js` へ抽出
- **設定 (v1 → v2)**: `[[clips]]` を `config.toml` から分離し、暗号化ファイル `registered-clips.dat` へ保存。初回起動時に既存 `[[clips]]` を自動移行

### Security

- 登録クリップを ChaCha20-Poly1305 で暗号化してディスクへ保存 (平文の `config.toml` 保存を廃止)

## [0.8.0] - 2026-06-28

### Added

- お気に入り変換モード (最大 20 件、専用ホットキー `Alt+Shift+1`〜`9` / `F1`〜`F11`)
- 加工パイプライン (`pipeline`、最大 10 段のモード連鎖)
- 画面 OCR (Windows のみ、`Alt+Shift+O` で範囲選択)
- ケース変換モード 5 種 (`to-camel-case` など)
- `excel-to-image` 変換モード (Windows のみ)

### Changed

- **設定 (v0 → v1)**: `favorite_modes` フィールドを追加
- 起動失敗時のデスクトップ通知とエラーメッセージを強化

### Fixed

- トレイ操作時のお気に入り連携とサブメニュー選択でデッドロックが起きる問題
- 非対応 OS の UI に Windows 専用変換モードが表示される問題

## [0.7.1] - 2026-06-27

### Added

- 設定ファイルの外部編集を検知してホットキー・監視設定などを再起動なしで再読み込み (トレイの「設定を再読み込み」でも即時反映)

### Changed

- 正規表現パターンのコンパイル結果をキャッシュし、繰り返し加工時のオーバーヘッドを削減

### Fixed

- Windows のデスクトップ通知が ClipRefiner 名義で表示されず、アイコンが未登録になる問題

## [0.7.0] - 2026-06-26

### Added

- 登録クリップ (よく使うテキストの保存・ホットキー / セレクタからコピー)
- 正規表現変換モード 4 種 (`regex-replace` / `regex-extract` / `regex-delete` / `regex-split`)
- `markdown-to-excel` 変換モード
- 設定スキーマのバージョン移行 (`config/migrate.rs`)
- Linux (deb) / macOS (DMG) インストーラー生成スクリプト
- 初回起動時に説明コメント付き `config.toml` を自動生成

### Changed

- **設定**: 保存形式を JSON から TOML へ変更 (`config.toml`)
- 通知にクリップボード内容を含めるかどうかを個別に設定可能に
- アプリ表示名を ClipRefiner に統一

### Security

- クリップボード上限 (2 MiB) とパーサー入力上限の適用
- 通知・履歴メニュー・登録クリッププレビューでの機密情報マスキング
- Unix / Windows で設定ディレクトリ・ログファイルのアクセス制限
- 履歴・取り消し用テキストの平文メモリ保持を削減 (`SecretString` / ゼロ化)

## [0.6.0] - 2026-06-24

### Added

- 加工の取り消し (`Alt+Shift+Z`)
- ログイン時の自動起動 (Windows / macOS / Linux)
- トレイメニュー「設定を開く」
- Linux Wayland でのイベント方式クリップボード監視 (`ext-data-control` / `wlr-data-control`)

### Changed

- Windows MSI を per-user インストールに変更 (管理者権限不要)

## [0.5.0] - 2026-06-18

### Added

- ホットキー・監視周期・通知などを `config` でカスタマイズ可能に

### Changed

- クイックセレクターの UI を改善
- 加工結果通知の表示内容を調整

### Security

- クリップボード履歴をメモリ内で ChaCha20-Poly1305 暗号化 (平文の常駐・ディスク書き込みなし)

## [0.4.1] - 2026-06-16

### Added

- macOS / Linux でイベント方式のクリップボード監視に対応

### Changed

- `AppState` の設定アクセス API を整理

### Fixed

- 一時停止中でも監視スレッドが動作し続ける問題
- 二重加工防止ロジックの副作用で意図しないスキップが起きる問題
- ホットキーでの一時停止が `config` に保存されない問題
- CSV 判定・数値判定・パニックリスクなど複数の安定性問題

## [0.4.0] - 2026-04-12

### Changed

- リリースビルドのログ出力をファイルのみに限定 (標準出力への出力はデバッグビルドのみ)
- ログローテーションのオーバーヘッドを改善
- 監視スレッドの停止・再開の挙動を改善

### Fixed

- UTM 除去で URL フラグメントが欠落する問題
- 複数行トリム時に CRLF が LF に変わる問題
- JSON の `\uXXXX` アンエスケープ未対応
- CSV 変換失敗時にクリップボードが空になる問題
- パス判定の誤検知

## [0.3.2] - 2026-03-21

### Added

- ファイルベースのロギング (日次ローテーション、`logs/` 配下)
- 一時停止状態の `config` への永続化

### Changed

- 設定・ログの保存先取得に `directories` クレートを使用
- イベントループとアプリ機能のモジュール分割

## [0.3.1] - 2026-02-16

### Added

- `path-to-slash` / `path-to-backslash` 変換モード
- 通知の種類 (モード変更 / 結果 / 一時停止) を個別に ON/OFF 可能に

### Changed

- 一時停止時にクリップボード監視スレッドを終了するように変更

## [0.3.0] - 2026-02-08

### Added

- クイックセレクター (`Alt+Shift+S`、コマンドパレット風 UI)
- グローバルホットキー (一時停止・終了・セレクター表示など)
- パス編集系変換モード (`extract-basename` など)
- `sort-lines-desc` 変換モード

### Changed

- 加工ロジックを `Refiner` トレイトへ整理
- アイコン画像を更新

## [0.2.1] - 2026-02-01

### Added

- クリップボード履歴 (トレイメニューから過去の加工結果を呼び出し)
- デスクトップ通知 (加工完了・モード変更など)
- Excel → Markdown 変換
- 行操作 (`remove-empty-lines` / `remove-duplicate-lines`)
- エスケープ / アンエスケープ、日時変換モード

### Changed

- トレイメニューをカテゴリ別サブメニューへ再構成
- 変換モードメニューに現在のモードのチェック表示を追加

## [0.2.0] - 2026-01-29

### Added

- イベント方式のクリップボード監視 (Windows / macOS。ポーリングとの切り替え可能)
- `markdown-to-html` 変換モード

## [0.1.1] - 2026-01-24

### Added

- YAML ↔ JSON 変換 (キー順序保持版を含む)
- `trim-lines` 変換モード
- キー順序保持版 JSON 整形 (`json-format-preserve-order`)
- 設定の自動保存と、読み込み失敗時の通知

## [0.1.0] - 2026-01-22

### Added

- 行ソート (`sort-lines-asc`)
- UTM パラメータ削除 (`remove-utm`)
- 数値のカンマ付与 / 除去
- JSON 整形、トリム、URL エンコード / デコード

## [0.0.3] - 2026-01-21

### Changed

- クリップボード監視ループと変換ロジックのリファクタリング
- エラーハンドリングの強化
- `Clipboard` インスタンスの使い回し

## [0.0.2] - 2026-01-20

### Changed

- トレイメニューで変換モードを切り替えた際、直ちにクリップボードへ変換を適用
- Windows 実行ファイルへバージョン・説明などのプロパティを埋め込み

## [0.0.1] - 2026-01-20

### Added

- システムトレイ常駐とクリップボード監視 (ポーリング)
- 多重起動防止
- エラー時のデスクトップ通知

[Unreleased]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.14.0...develop
[0.14.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.13.1...v0.14.0
[0.13.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.13.0...v0.13.1
[0.13.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.6...v0.13.0
[0.12.6]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.5...v0.12.6
[0.12.5]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.4...v0.12.5
[0.12.4]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.3...v0.12.4
[0.12.3]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.2...v0.12.3
[0.12.2]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.1...v0.12.2
[0.12.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.12.0...v0.12.1
[0.12.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.11.0...v0.12.0
[0.11.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.10.0...v0.11.0
[0.10.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.9.0...v0.10.0
[0.9.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.8.0...v0.9.0
[0.8.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.7.1...v0.8.0
[0.7.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.7.0...v0.7.1
[0.7.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.6.0...v0.7.0
[0.6.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.5.0...v0.6.0
[0.5.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.4.1...v0.5.0
[0.4.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.4.0...v0.4.1
[0.4.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.2...v0.4.0
[0.3.2]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.1...v0.3.2
[0.3.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.3.0...v0.3.1
[0.3.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.2.1...v0.3.0
[0.2.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.2.0...v0.2.1
[0.2.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.1.1...v0.2.0
[0.1.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.1.0...v0.1.1
[0.1.0]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.3...v0.1.0
[0.0.3]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.2...v0.0.3
[0.0.2]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/compare/v0.0.1...v0.0.2
[0.0.1]: https://gitea.b-gnud.duckdns.org/GnudMna/clip-refiner/src/tag/v0.0.1
