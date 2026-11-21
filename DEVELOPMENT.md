# ClipRefiner 開発者向けドキュメント

ユーザ向けの機能説明・操作方法は [README.md](README.md) を参照してください。設定の詳細は [CONFIG.md](CONFIG.md)、バージョンごとの変更内容は [CHANGELOG.md](CHANGELOG.md) を参照してください。

## 目次

- [前提条件](#前提条件)
- [ソースからのビルド](#ソースからのビルド)
- [Windows パッケージ作成](#windows-パッケージ作成)
- [開発・テスト](#開発テスト)
- [品質チェック](#品質チェック)
- [ライブラリとしての利用](#ライブラリとしての利用)
- [プロジェクト構成](#プロジェクト構成)
- [スレッドモデル](#スレッドモデル)
- [ログ](#ログ)
- [リリースと変更履歴](#リリースと変更履歴)
- [コーディング規約](#コーディング規約)

---

## 前提条件

- [Rust / Cargo](https://www.rust-lang.org/tools/install) (edition 2024、Rust 1.96 以上。`rust-toolchain.toml` でピン留め)
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) (サプライチェーン監査。`cargo install cargo-deny --locked`)
- [Node.js](https://nodejs.org/) (設定 UI の JavaScript ロジックテスト・ESLint / Prettier。`check.ps1` 実行時は必須。通常の `cargo test` では未インストール時に JS テストをスキップする)

対応 OS は **Windows** のみ。

---

## ソースからのビルド

```bash
git clone <repository_url>
cd clip-refiner
cargo build --release
```

品質チェックをコミット・プッシュ時に自動実行する場合は [Git フック](#git-フック) を有効化する:

```powershell
./scripts/windows/install-hooks.ps1
```

バイナリは `target/release/ClipRefiner.exe` に生成される。ビルドスクリプト `scripts/windows/build.ps1` も利用できる。

---

## Windows パッケージ作成

### Windows MSI

`cargo-wix` と WiX Toolset v3 を使って MSI を作成できる。

```powershell
# 前提: cargo install cargo-wix --locked
#       winget install WiXToolset.WiXToolset
./scripts/windows/build-msi.ps1
```

出力先: `target/wix/clip-refiner-{version}-{arch}.msi` (per-user インストール、日本語 UI)

`build-msi.ps1` は MSI 作成後に内容検証を自動実行する。既存 MSI のみ検証する場合:

```powershell
./scripts/windows/verify-msi.ps1
./scripts/windows/verify-msi.ps1 -MsiPath target\wix\clip-refiner-0.12.1-x86_64-pc-windows-msvc.msi
```

**検証内容:**

| 区分 | 項目 |
| :--- | :--- |
| メタデータ | `Cargo.toml` の `upgrade-guid` と `wix/main.wxs` の `UpgradeCode` の一致 |
| MSI Property | `ProductName`, `ProductVersion`, `UpgradeCode`, `Manufacturer`, per-user (`ALLUSERS`) |
| MSI テーブル | 必須ファイル (`ClipRefiner.exe`, `LICENSE`, `THIRD_PARTY_NOTICES`)、Feature (`Binaries`, `Environment`, `Autostart`) |
| 展開内容 | 管理インストール (`msiexec /a`) で展開し、実行ファイルサイズ・`THIRD_PARTY_NOTICES` ヘッダー・生成物サイズ一致を確認 |

`-SkipExtract` でデータベース検証のみに省略できる。`build-msi.ps1 -SkipVerify` でビルド後検証をスキップできる。

---

## 開発・テスト

```bash
# 単体テスト・統合テストを実行 (デフォルト feature `app` で統合テストも含む)
cargo test

# 特定の統合テストのみ実行
cargo test --test monitor_flow
cargo test --test settings_flow
cargo test --test clips_flow
cargo test --test worker_recovery_flow

# ライブラリ API のドキュメントを生成
cargo doc --no-deps --open
```

加工ロジックは `clip_refiner` ライブラリクレートとしても利用できる。

```rust
use clip_refiner::{RefineContext, RefineMode, Refiner};

let ctx = RefineContext::default();
let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
assert_eq!(output, "hello world");
```

統合テスト (`crates/clip-refiner/tests/`) では監視ループ・ワーカー・履歴・正規表現・連続記録・登録クリップの主要経路を検証する。`ClipboardHarness` が監視ループとワーカーコマンドを横断的に駆動する。

| ファイル | 検証内容 |
| :------- | :------- |
| `monitor_flow.rs` | 監視加工、Undo、一時停止、履歴上限、同一テキストの再加工抑制、自身の書き戻しスキップ、パイプライン、連続記録中の自動加工停止 |
| `worker_flow.rs` | 手動加工 (`ProcessMode`)、変更なし加工、Undo 空実行、履歴復元後の再加工抑制 (Polling / Event) |
| `worker_recovery_flow.rs` | クリップボードワーカーの起動、`restart` による復旧、復旧メニュー表示 |
| `history_flow.rs` | 暗号化履歴の記録・重複処理・復号 |
| `regex_flow.rs` | `config` の正規表現設定、コンパイルキャッシュ、Event 方式 |
| `ocr_flow.rs` | OCR 結果のクリップボード書き込みと再加工抑制 |
| `record_flow.rs` | 連続記録キュー (FIFO、貼り付け排出、履歴との独立性) |
| `clips_flow.rs` | 登録クリップの保存・コピーと再加工抑制 |
| `settings_flow.rs` | 設定ウィンドウ IPC のマージ・パッチ適用・パイプラインとデフォルトモード |
| `notification_flow.rs` | 通知設定の旧項目移行・IPC パッチ・トレイメニュー連携 |
| `platform_flow.rs` | ホットキー解釈・OS 登録・自動起動状態照会 |
| `regex_preview_logic.rs` | 設定 UI の正規表現プレビュー JS ロジック (Node.js 実行) |
| `settings_ui_logic.rs` | 設定 UI のパッチマージ JS ロジック (`settings_patch.js` / Node.js 実行) |
| `settings_shared_logic.rs` | 設定 UI 共有純粋ロジック (`shared_logic.js` / Node.js 実行) |
| `oneshot_flow.rs` | ワンショット相当のパイプライン加工と正規表現 CLI オプション |
| `library_api.rs` | クレートルート re-export と core 連携 |

`clip-refiner-core/tests/property_transforms.rs` では `proptest` による URL・エスケープ・正規表現置換のプロパティテストを実行する。

UI ロジックテストは Node.js 未インストール時にスキップする (`note:` を出力)。`check.ps1` は Node.js を検証し `CLIP_REFINER_REQUIRE_NODE=1` で必須化する。意図的にスキップする場合は `CLIP_REFINER_SKIP_NODE_TESTS=1` を設定する。

### テスト用ヘルパー (`test_helpers`)

`clip_refiner::test_helpers` はデバッグビルド、`cargo test` 実行時、または `test-helpers` feature 有効時に利用可能 (feature `app` が必要)。リリースビルドで Clippy やドキュメントテストを走らせる場合は `--features test-helpers` を付ける。

| モジュール | 所在 | 内容 |
| :--------- | :--- | :--- |
| `clip_refiner::test_helpers` | `clip-refiner/src/test_helpers/` | アプリ層の統合テスト用 (`ClipboardHarness`, `SettingsTestContext` など) |
| `clip_refiner_core::test_helpers` | `clip-refiner-core/src/test_helpers/` | コア単体テスト用 (`InMemoryTextClipboard`, `with_temp_config_dir`) |

アプリ側の `test_helpers` は core のヘルパーを re-export する。設定ディレクトリを触るテストは `with_temp_config_dir` で一時ディレクトリへ切り替える。

#### `ClipboardHarness`

監視ループ (`handle_clipboard_update`) とワーカーコマンド (`handle_command`) を、OS クリップボードなしで横断検証するハーネス。`InMemoryTextClipboard` と `AppState` (テスト用初期化) を束ね、本番と同じ処理関数を直接呼び出す。

```rust
use clip_refiner::RefineMode;
use clip_refiner::test_helpers::ClipboardHarness;

let mut harness = ClipboardHarness::with_text("  hello  ")
    .with_mode(RefineMode::Trim);

assert!(harness.run_monitor_update(false));
assert_eq!(harness.clipboard_text(), "hello");

harness.undo();
assert_eq!(harness.clipboard_text(), "  hello  ");
```

**ビルダー (設定の事前セット)**

| メソッド | 用途 |
| :------- | :--- |
| `with_mode` | 監視・手動加工のデフォルトモード |
| `with_pipeline` | 監視時の加工パイプライン |
| `with_history` | 履歴の有効・無効 |
| `with_continuous_record` | 連続記録の有効・無効 |
| `with_paused` | 監視の一時停止 |
| `with_monitor_mode` | `MonitorMode::Polling` / `Event` |

**監視ループ経路**

| メソッド | 本番の対応 |
| :------- | :--------- |
| `run_monitor_update(event_driven)` | 監視ループ 1 ティック (`event_driven` は OS イベント有無) |
| `run_configured_monitor_update()` | 設定の `monitor_mode` に応じた `event_driven` で実行 |
| `replace_clipboard` / `reset_clipboard` | 外部コピー・クリップボード差し替えの模倣 |

戻り値 `bool` は加工が実行されたか (`handle_clipboard_update` と同じ)。

**ワーカーコマンド経路** (`ClipboardCommand` 相当)

| メソッド | コマンド |
| :------- | :------- |
| `process_mode` | `ProcessMode` |
| `set_text` | `SetText` (履歴復元) |
| `set_ocr_text` | `SetOcrText` |
| `undo` | `Undo` |
| `register_clip_from_clipboard` | `RegisterClipFromClipboard` |
| `copy_registered_clip` | `CopyRegisteredClip` |
| `discharge_continuous_record_on_paste` | `paste_watcher::try_paste_continuous_record_front` (`Ctrl+V` 検知経路と同じ) |
| `discharge_continuous_record_at` | `DischargeContinuousRecordAt` |
| `catch_up_continuous_record` | `CatchUpContinuousRecord` |

**検証用アクセサ**

`clipboard_text`, `history_len`, `history_entries`, `continuous_record_len`, `continuous_record_entries`, `registered_clip_count`, `registered_clip_text`, `take_undo_source`, `matches_last_written`, `with_config_mut` など。

新しい統合テストを追加するときは、既存の `*_flow.rs` に近い経路かどうかでファイルを選び、監視とワーカーの両方にまたがるケースは `ClipboardHarness` で往復検証する。設定ウィンドウの IPC とパッチ適用は `SettingsTestContext` を使う。

#### `SettingsTestContext`

設定ウィンドウの `apply` IPC・適用待ちキューのマージ・`config.toml` へのパッチ反映を、WebView なしで検証するヘルパー。`TrayMenu` とスタブ `ClipboardWorkerHandle` を束ね、本番と同じ `apply_settings_patch_without_ui` を呼び出す。

```rust
use clip_refiner::test_helpers::SettingsTestContext;

let ctx = SettingsTestContext::new();

assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"historyEnabled":true}}"#));
assert!(ctx.state().with_config(|c| c.history_enabled));
```

| メソッド | 用途 |
| :------- | :--- |
| `enqueue_apply_ipc` | JSON IPC の `apply` を適用待ちキューへ積む |
| `enqueue_patch_json` | 部分更新 JSON をキューへ積む (マージ検証用) |
| `take_pending_patch_json` | マージ済みパッチ JSON を取り出す |
| `apply_pending_patch` | キューから取り出して反映 |
| `apply_ipc` | JSON IPC を解釈して即時反映 |
| `state` / `menu` | 検証用アクセサ |

---

## 品質チェック

Windows の `check` スクリプト (`scripts/windows/check.ps1`) で、整形・Clippy・サプライチェーン監査・テストを一括実行できる。コード整形のみ行う場合は `format` スクリプト (`scripts/windows/format.ps1`) で rustfmt と Prettier を適用できる。

```powershell
./scripts/windows/check.ps1
./scripts/windows/format.ps1
```

### Git フック

コミット・プッシュ時に品質チェックを自動実行する Git フックをリポジトリ同梱している。クローン後に一度だけ有効化する:

```powershell
./scripts/windows/install-hooks.ps1
```

| フック | 実行内容 | 所要時間の目安 |
| :----- | :------- | :------------- |
| `pre-commit` | `cargo fmt --check` (`check-pre-commit.ps1`) | 数秒 |
| `pre-push` | `check.ps1` 一式 (fmt / clippy / deny / eslint / test) | 数分 |

無効化:

```powershell
./scripts/windows/uninstall-hooks.ps1
```

一時的なスキップ:

| 方法 | 用途 |
| :--- | :--- |
| `git commit --no-verify` | pre-commit のみスキップ |
| `git push --no-verify` | pre-push のみスキップ |
| `CLIP_REFINER_SKIP_HOOK=1` | 両フックをスキップ |

`core.hooksPath` はローカル Git 設定 (`git config core.hooksPath`) に保存される。フック本体は `scripts/git/hooks/` に置き、バージョン管理する。

手動でフックを試す場合:

```powershell
git hook run pre-commit
git hook run pre-push
```

個別コマンド:

```bash
cargo fmt
./scripts/windows/format.ps1
cargo fmt --check
cargo clippy --all-targets --features test-helpers -- -D warnings
cargo deny check
npm ci
npm run lint
npm run format:check
cargo test --all-targets --features test-helpers
cargo test --doc --features test-helpers
```

### JavaScript リンター (ESLint / Prettier)

UI / テスト用 JavaScript (`crates/clip-refiner/src/ui/`, `crates/clip-refiner/tests/`) は ESLint と Prettier で検証する。`check.ps1` / `format.ps1` は `scripts/common/npm.ps1` 経由で実行する。

| ツール | 役割 |
| :----- | :--- |
| [ESLint](https://eslint.org/) | 推奨ルール + `eslint-plugin-no-unsanitized` (`innerHTML` 等の XSS リスク検出) |
| [Prettier](https://prettier.io/) | JavaScript の整形 (タブ幅 4、シングルクォート) |

初回または `package.json` 更新後:

```powershell
npm ci
```

個別実行:

```powershell
npm run lint          # ESLint
npm run lint:fix      # ESLint (自動修正)
npm run format:check  # Prettier 整形チェック
npm run format        # Prettier 整形適用
```

設定ファイル: [`package.json`](package.json), [`eslint.config.js`](eslint.config.js), [`.prettierrc.json`](.prettierrc.json)

設定画面の JavaScript は Rust 側で 1 つの IIFE に連結する (`tray/settings/window.rs`)。連結後の共有バインディングは ESLint 単体では解決できないため、`shared.js` / `panels.js` / `events.js` / `main.js` では `no-undef` 等を無効化している。`escapeHtml` / `highlightText` および信頼済み HTML 組み立て関数の戻り値のみ `innerHTML` 代入を許可する。

### スクリプト構成

`scripts/windows/` は開発者が直接実行するエントリポイント。実処理は `scripts/common/` のドットソース用ヘルパーに集約する。

| `scripts/common/` | 役割 |
| :---------------- | :--- |
| `assert.ps1` | 外部コマンドの終了コード検証 (`Assert-LastExitCode`) |
| `cargo.ps1` | `cargo fmt` / `clippy` / `test` のラッパー |
| `npm.ps1` | `npm ci` / ESLint / Prettier のラッパー |
| `cargo-deny-check.ps1` | `cargo deny check` |
| `wix.ps1` | `cargo-wix` / WiX Toolset v3 の存在確認 |
| `verify-msi.ps1` | MSI メタデータ・展開内容の検証ロジック |
| `generate-third-party-notices.ps1` | `THIRD_PARTY_NOTICES` 生成 |
| `release-version.ps1` | リリース時のバージョン更新ヘルパー |
| `cd-project-root.ps1` | リポジトリルートへ移動 |
| `wait-if-double-clicked.ps1` | エクスプローラー起動時の Enter 待ち (`Invoke-ScriptMain`) |

| `scripts/windows/` | 役割 |
| :----------------- | :--- |
| `check.ps1` | 品質チェック一式 |
| `format.ps1` | rustfmt + Prettier 整形 |
| `test.ps1` / `clippy.ps1` | 個別の Cargo コマンド |
| `check-pre-commit.ps1` | pre-commit 用 (`cargo fmt --check`) |
| `build.ps1` / `build-msi.ps1` | ビルド・MSI 作成 |
| `verify-msi.ps1` | MSI 検証の CLI ラッパー |
| `release.ps1` | リリース手順一括実行 |
| `install-hooks.ps1` / `uninstall-hooks.ps1` | Git フック管理 |
| `generate-wix-assets.ps1` | WiX ブランド画像生成 |

### サプライチェーン監査 (`cargo-deny`)

[`deny.toml`](deny.toml) で依存関係の脆弱性・ライセンス・取得元を検証する。`check` スクリプトは `cargo deny check` を含む。

| チェック | 内容 |
| :------- | :--- |
| **advisories** | RustSec 公表の脆弱性・yanked crate。`unmaintained` はワークスペース直接依存のみ失敗 |
| **licenses** | 商用利用可能なパーミッシブライセンスのみ許可 (GPL / LGPL / MPL 等のコピーレフトは拒否) |
| **bans** | 同一 crate の多重バージョンなど |
| **sources** | crates.io 以外のレジストリ / git 依存の拒否 |

`graph.all-features` と Windows ターゲット (`x86_64-pc-windows-msvc`) を対象に、feature `app` 有効時の依存グラフを検査する。

許可リストはパーミッシブライセンス (MIT / Apache-2.0 / BSD 等) のみ。コピーレフト (GPL / LGPL / MPL 等) は拒否する。

### 第三者ライセンス表示 (`THIRD_PARTY_NOTICES`)

`LICENSE` は ClipRefiner 本体 (All Rights Reserved) の利用条件のみを示す。配布バイナリに含まれる OSS 依存の帰属・ライセンス全文は **`THIRD_PARTY_NOTICES`** にまとめる。

| 項目 | 内容 |
| :--- | :--- |
| 生成ツール | [cargo-about](https://github.com/EmbarkStudios/cargo-about) (`cargo install cargo-about --locked --features cli`) |
| 設定 | [`about.toml`](about.toml)、テンプレート [`scripts/templates/third-party-notices.hbs`](scripts/templates/third-party-notices.hbs) |
| 自クレート除外 | `about.toml` の `private.ignore` でワークスペース内クレート (`clip-refiner`, `clip-refiner-core`) を除外 (`deny.toml` と同方針) |
| 手動生成 | `scripts/common/generate-third-party-notices.ps1` |
| 出力 | リポジトリルートの `THIRD_PARTY_NOTICES` (`.gitignore` 対象。パッケージ作成時に自動生成) |

パッケージへの同梱:

| プラットフォーム | 同梱先 |
| :--------------- | :----- |
| **Windows MSI** | `%LOCALAPPDATA%\ClipRefiner\THIRD_PARTY_NOTICES` (`build-msi.ps1` が生成後に同梱) |

---

## ライブラリとしての利用

`ClipRefiner` はデスクトップ常駐アプリが主用途だが、加工ロジックは GUI 非依存の **`clip-refiner-core`** クレート (`clip_refiner_core`) として他プロジェクトから呼び出せる。`clip-refiner` パッケージは core を re-export しており、feature `app` (デフォルト有効) でトレイ常駐 UI と `run()` を追加する。

ライブラリ API は [Semantic Versioning](https://semver.org/lang/ja/) に従い、破壊的変更は [CHANGELOG.md](CHANGELOG.md) の **Breaking changes** 節に記載する。

### 依存の追加

GUI 依存 (`tao`, `wry`, `tray-icon` など) を避ける場合は、次のいずれかを使う。

**推奨: core クレートを直接依存**

```toml
[dependencies]
clip-refiner-core = { path = "crates/clip-refiner-core" }
```

**互換: `clip-refiner` で feature を無効化**

```toml
[dependencies]
clip-refiner = { path = "..", default-features = false }
```

Rust コード上のクレート名はいずれも `clip_refiner` (re-export) または `clip_refiner_core` を使える。

### 公開 API 一覧

| 種別 | 名前 | 説明 |
| :--- | :--- | :--- |
| モジュール | `refiner` | 加工モードとテキスト / クリップボード加工 |
| モジュール | `config` | 設定型と `config.toml` 永続化 |
| 加工 | `RefineMode`, `RefineCategory`, `Refiner` | 単一モードの適用 |
| 加工 | `apply_text_pipeline`, `apply_pipeline_to_text` | 複数モードの連鎖適用 |
| 加工 | `split_pipeline` | テキスト用モード列と末尾画像モードの分割 |
| 加工 | `REGEX_PREVIEW_SAMPLE`, `RegexPreviewResult`, `preview_regex_replace` | 正規表現置換のプレビュー (設定 UI と同一エンジン) |
| クリップボード | `process_clipboard`, `process_clipboard_pipeline` | `arboard::Clipboard` への読み書き |
| 結果 | `ClipboardProcessOutcome`, `ClipboardProcessError` | 加工結果と失敗理由 |
| 設定 | `AppConfig`, `NotificationSettings`, `RegexSettings`, `HotkeySettings` など | `config.toml` 対応型 |
| 設定 | `AppConfig::load` (`ConfigLoadOutcome`)、`AppConfig::save`, `AppConfig::reload_from_disk` | 設定ファイル I/O |
| 設定 | `RefineContext::from_config`, `RefineContext::with_regex` | 加工時コンテキスト構築 |
| 定数 | `CONFIG_VERSION` | 設定スキーマの現行バージョン |
| 上限 | `is_within_clipboard_limit`, `is_within_parser_limit` | 入力サイズ検証 |
| アプリ | `run()` | トレイ常駐アプリ全体の起動 (feature `app` が必要) |

### 使用例

単一モード:

```rust
use clip_refiner::{RefineContext, RefineMode, Refiner};

let ctx = RefineContext::default();
let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
assert_eq!(output, "hello world");
```

加工パイプライン:

```rust
use clip_refiner::{apply_text_pipeline, RefineContext, RefineMode};

let ctx = RefineContext::default();
let result = apply_text_pipeline(
    "  %E3%81%82  ",
    &[RefineMode::UrlDecode, RefineMode::Trim],
    &ctx,
);
assert_eq!(result.as_deref(), Some("あ"));
```

設定連携:

```rust
use clip_refiner::{AppConfig, RefineContext, apply_text_pipeline};

let load_outcome = AppConfig::load();
let config = load_outcome.config;
let ctx = RefineContext::from_config(&config);
let pipeline = config.effective_pipeline();
let _ = apply_text_pipeline("  hello  ", &pipeline, &ctx);
```

詳細は `cargo doc --open` で生成される API ドキュメントを参照。

---

## プロジェクト構成

```
clip-refiner/
├── Cargo.toml           # workspace ルート (virtual manifest)
├── package.json         # UI JavaScript の ESLint / Prettier (本番バイナリには同梱しない)
├── eslint.config.js
├── crates/
│   ├── clip-refiner/    # デスクトップアプリ (バイナリ + tray)
│   │   ├── src/
│   │   │   ├── platform/    # OS 連携 (通知・自動起動・OCR・画面キャプチャ)
│   │   │   ├── test_helpers/ # 統合テスト用 (`ClipboardHarness` など)
│   │   │   ├── tray/        # 常駐 UI・ワーカー・ホットキー・設定ウィンドウ
│   │   │   │   ├── features/    # トレイメニュー機能ドメイン (menu + event)
│   │   │   │   ├── menu/        # TrayMenu 型・全体構築・同期
│   │   │   │   ├── event/       # イベントループ制御・ルーティング
│   │   │   │   ├── selector/    # クイック/登録クリップセレクター WebView
│   │   │   │   ├── settings/    # 設定ウィンドウ (model, ipc, apply/, snapshot など)
│   │   │   │   ├── worker/      # クリップボードワーカー・監視ループ
│   │   │   │   └── ...
│   │   │   └── ui/          # WebView UI (セレクター・設定ウィンドウ)
│   │   ├── tests/           # 統合テスト (`*_flow.rs`)
│   │   └── build.rs
│   └── clip-refiner-core/   # 加工ロジック・設定型 (GUI 非依存)
│       └── src/
│           ├── config/
│           ├── platform/    # DPAPI・クリップボード画像など GUI 非依存の OS 連携
│           ├── refiner/
│           ├── security/
│           └── test_helpers/ # コア単体テスト用
├── assets/
├── scripts/
│   ├── common/          # ドットソース用ヘルパー (cargo / npm / wix / MSI 検証など)
│   ├── git/hooks/       # Git フック (pre-commit / pre-push)
│   └── windows/         # 開発者向けエントリポイント (check.ps1, format.ps1 など)
├── CONFIG.md
└── wix/
```

`clip-refiner` の `platform/` は Win32 / WinRT 依存の機能を `windows` クレート経由で集約する。`clip-refiner-core` の `platform/` は暗号化キー保護やクリップボード画像取得など、ライブラリ利用時にも必要な最小限の OS 連携のみを置く。

セレクター UI (`src/ui/`) は WebView 上で動作する。`selector-common.js` に共通ロジックを集約し、クイックセレクタ・登録クリップセレクタはそれぞれ `quick_selector.js`・`clip_selector.js` に UI ロジックを分割している。Rust 側 (`tray/selector/`) で CSS / JS をインライン展開して `WebView` に渡す。

### `tray/` の責務分担

トレイ常駐 UI は **インフラ層** (`menu/`, `event/`, `worker/` など) と **機能ドメイン層** (`features/`, `settings/apply/patch/`) に分ける。新機能を追加するときは、まず `features/` にメニュー構築とクリック処理をまとめ、設定ウィンドウ連携が必要なら `settings/apply/patch/` にパッチ適用を追加する。

| モジュール (`tray/`) | 責務 |
| :------------------- | :--- |
| `features/` | トレイメニュー項目ごとのドメイン実装。各サブモジュールが `menu.rs` (構築・更新) と `event.rs` (クリック処理) を担う |
| `menu/` | `TrayMenu` 型定義、`build.rs` (全体組み立て)、`sync.rs` (設定同期)、`icon.rs` |
| `event/` | tao イベントループへの委譲 (`loop_event`, `user_event`)、ホットキー・OCR・設定再読み込み、`handle_menu_event` ルーティング |
| `selector/` | クイックセレクター・登録クリップセレクター・OCR オーバーレイの WebView 束 (`SelectorUi`) |
| `settings/` | 設定ウィンドウ (下表参照) |
| `worker/` | クリップボード I/O 集約・監視ループ |
| `state/` | `AppState`, `AppEvent` (UI とワーカー共有) |
| `hotkey/` | グローバルホットキー登録・イベント変換 |
| `clipboard_monitor/` | 監視ループの判断・世代管理 |
| `notify/` | デスクトップ通知本文の組み立て (`NotificationSettings` 連動) |
| `history_store/` | 暗号化クリップボード履歴・連続記録のメモリストア (`EncryptedHistoryStore`) |
| `paste_watcher/` | 連続記録 ON 時の `Ctrl+V` フック |
| `ocr/` | 画面 OCR キャプチャ連携 |
| `app/` | `App` 構造体と起動時初期化 |
| `dispatch.rs` | ワーカー・イベントループへの送信ヘルパー |

`features/` のドメイン一覧:

| サブモジュール | 対応するトレイメニュー / 操作 |
| :------------- | :---------------------------- |
| `refine` | 変換モード・お気に入り (`favorites.rs`) |
| `monitor` | 監視方式・監視周期 |
| `clips` | 登録クリップ |
| `history` | 履歴メニュー (有効化・クリア・過去項目の選択) |
| `record` | 連続記録 |
| `notification` | 通知設定 |
| `app_control` | 終了・一時停止・設定ウィンドウ・監視再開 |

設定ウィンドウ (`src/ui/settings/`) は `main.js` (エントリ)、`panels.js` (タブ描画)、`events.js` (入力イベント)、`shared.js` (状態同期・debounce)、`shared_logic.js` (件数クランプ・パイプライン正規化・通知パッチ組み立てなどの純粋ロジック)、`settings_patch.js` (パッチ組み立て)、`regex_preview.js` (プレビュー表示) に分割している。`shared_logic.js` は Node.js 上の `settings_shared_logic.rs` でも検証する。Rust 側 (`tray/settings/window.rs`) で順序通りにインライン結合する。読み書き項目は `tray/settings/model.rs` の `SettingsWritable` / `SettingsPatch`、初期表示は `snapshot.rs` の `SettingsSnapshot` が担う。パッチ適用は `tray/settings/apply/patch/` でドメイン別 (監視・履歴・連続記録・正規表現・加工・通知・システム) に分割されている。

| モジュール (`tray/settings/`) | 責務 |
| :---------------------------- | :--- |
| `model` | `SettingsWritable` / `SettingsPatch` (マクロ定義による読み書き項目の一元化) |
| `snapshot` | `SettingsSnapshot`, `SettingsLimits` (初期表示 JSON と UI 上限) |
| `ipc` | WebView IPC メッセージの解析と適用待ちキュー |
| `apply/` | パッチ適用のオーケストレーション (`patch/` でドメイン別、`feedback` でエラー通知) |
| `regex_preview` | 正規表現プレビュー IPC 応答 (`REGEX_PREVIEW_SAMPLE` を使用) |
| `window` | WebView の組み立てと表示制御 |

---

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
        App --> Tray
        App --> QS
        App --> CS
        App --> SW
    end

    subgraph hotkey_thread ["ホットキーリスナースレッド"]
        GHK["GlobalHotKeyEvent::receiver()"]
    end

    subgraph worker ["クリップボードワーカースレッド"]
        WL["recv_timeout ループ"]
        Mon["監視ループ (Polling / Event)"]
        Cmd["ClipboardCommand 処理"]
        WL --> Mon
        WL --> Cmd
    end

    CB[("OS クリップボード")]
    State[("AppState (Arc)")]

    GHK -->|"AppEvent::Hotkey"| App
    App -->|"ClipboardCommand (mpsc)"| WL
    WL -->|"AppEvent (EventLoopProxy)"| App
    Cmd <--> CB
    Mon <--> CB
    App <--> State
    WL <--> State
```

### 各コンポーネント

| コンポーネント | スレッド | 責務 |
| :------------- | :------- | :--- |
| **Event Loop** (`tray/runner.rs`) | メイン | ウィンドウイベント、トレイメニュー、WebView IPC (セレクター・設定ウィンドウ)、`AppEvent` の受信と `App::handle_event` への振り分け |
| **Hotkey Listener** (`tray/hotkey.rs`) | 専用 | `global-hotkey` のイベントをブロッキング受信し、`EventLoopProxy` 経由で `AppEvent::Hotkey` をメインスレッドへ送る |
| **Clipboard Worker** (`tray/worker/`) | 専用 | すべてのクリップボード読み書き、監視ループ、設定ファイルの外部変更ポーリング。`recv_timeout` でコマンド受信と監視チックを交互に処理 |
| **AppState** (`tray/state/`) | 共有 (`Arc`) | 設定 (`RwLock`)、加工済み状態・履歴・Undo 用テキスト (`Mutex`)。UI とワーカーの両方から参照 |

### 通信の要点

- **UI → ワーカー**: `ClipboardCommand` を `mpsc` チャネルで送信 (手動加工、履歴復元、登録クリップコピーなど)
- **ワーカー → UI**: `EventLoopProxy<AppEvent>` でユーザーイベントを送る (例: 設定再読み込み完了)
- **ホットキー → UI**: 専用スレッドが `AppEvent::Hotkey` を送り、メインスレッドの `HotkeyHandler` が処理
- **クリップボード**: `arboard::Clipboard` はワーカースレッド内の 1 インスタンスのみが触る (競合回避)

加工ロジック自体 (`refiner/`) はスレッドを持たず、ワーカーまたはワンショット実行 (`bootstrap.rs`) から同期的に呼ばれる。

### デスクトップ通知

通知の呼び出しは 2 層に分ける。

| 層 | モジュール | 責務 |
| :--- | :--------- | :--- |
| **platform** | `platform/notify.rs` | Windows トーストの初期化と送信 (`init_notifications`, `show_notification`) |
| **tray** | `tray/notify/message.rs` | 加工完了・一時停止など設定連動の本文組み立て (`show_process_notification`, `show_when_enabled` など) |
| **features** | `tray/features/notification/` | トレイメニュー「通知」の構築・クリック処理 |

- エラー表示・設定読み込み失敗など固定メッセージは、呼び出し元から `platform::show_notification` を直接使う (`NotificationSettings.enabled` を経由しない)
- 監視モードの加工完了は `tray/notify` が `NotificationSettings` を反映して本文を組み立て、最終的に `platform::show_notification` へ渡す
- 起動時は `bootstrap.rs` で `init_notifications` を呼び、AUMID とアイコンを登録してから各所でトーストを送る

`NotificationSettings` (`clip-refiner-core/src/config/types/notification.rs`) はマスター (`enabled`) と種類別トグル (`notify_process` など) を保持する。旧 `notify_mode` / `notify_actions` は `migrate_legacy_fields` で読み込み時に移行し、保存時は出力しない。利用者向けの説明は [CONFIG.md の通知節](CONFIG.md#通知-notification_settings) を参照。

---

## ログ

ログファイルは設定ディレクトリ内の `logs/` フォルダに日次ローテーションで保存される (保存場所は [CONFIG.md のログ節](CONFIG.md#ログ) を参照)。

ログレベルは環境変数 `RUST_LOG` で制御できる (例: `RUST_LOG=debug`)。

| ビルド種別 | 出力先 |
| :--------- | :----- |
| デバッグビルド (`cargo build` / `cargo run`) | ログファイル + 標準出力 |
| リリースビルド | ログファイルのみ |

---

## リリースと変更履歴

バージョンごとの利用者向け変更内容は [CHANGELOG.md](CHANGELOG.md) に記載する ([Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) 形式)。

### リリーススクリプト (`release.ps1`)

`scripts/windows/release.ps1` で、バージョン更新から MSI 作成までを一括実行できる。

```powershell
# リリースブランチを作成して checkout (バージョンとブランチ名が一致必須)
git checkout -b release/v0.12.2

# 明示的なバージョン指定
./scripts/windows/release.ps1 -Version 0.12.2

# パッチリリース (0.12.1 -> 0.12.2)。事前に release/v0.12.2 ブランチへ checkout すること
./scripts/windows/release.ps1 -Bump Patch

# 変更内容の確認のみ (ファイル・Git・MSI は触らない)
./scripts/windows/release.ps1 -Version 0.12.2 -DryRun
```

**自動で行うこと:**

1. 現在のブランチが `release/v{version}` であることを検証 (不一致なら失敗)
2. `Cargo.toml` の `version` と `crates/clip-refiner/Cargo.toml` の path 依存 `version` を更新
3. [CHANGELOG.md](CHANGELOG.md) の `[Unreleased]` 節を `## [x.y.z] - YYYY-MM-DD` へ移し、空の `[Unreleased]` と compare リンクを更新
4. `crates/clip-refiner/src/lib.rs` のドキュメント内バージョン例を更新
5. `check.ps1` で品質チェック (fmt / clippy / deny / eslint / test)
6. `Release: v{version}` で Git コミット
7. `build-msi.ps1` で MSI を作成し内容を検証 (`THIRD_PARTY_NOTICES` 同梱)

**主なオプション:**

| オプション | 説明 |
| :--------- | :--- |
| `-Bump Patch\|Minor\|Major` | 現在のバージョンから SemVer インクリメント |
| `-Version x.y.z` | 新バージョンを明示指定 (`-Bump` と排他) |
| `-Date yyyy-MM-dd` | リリース日 (既定: 本日) |
| `-DryRun` | 更新予定を表示するのみ |
| `-SkipCheck` | 品質チェックをスキップ |
| `-SkipMsi` | MSI 作成をスキップ |
| `-NoCommit` | Git コミットをスキップ |
| `-AllowDirty` | 未コミット変更があっても続行 |

**前提:**

- `release/v{version}` ブランチ上で実行すること (例: `release/v0.12.2` で `-Version 0.12.2`)
- リリース前に [CHANGELOG.md](CHANGELOG.md) の `[Unreleased]` 節へ変更内容を記載しておく (空だと失敗する)
- 既定では作業ツリーがクリーンであること (`-AllowDirty` で上書き可)
- MSI 作成には [Windows パッケージ作成](#windows-パッケージ作成) と同じ前提 (cargo-wix, WiX Toolset v3)

**手動で残る作業:**

1. 設定スキーマ (`consts::CONFIG_VERSION`) を変更したリリースでは、CHANGELOG の **設定** 節に移行内容を書く
2. ユーザ向けの大きな変更があれば [README.md](README.md) / [CONFIG.md](CONFIG.md) も合わせて更新する
3. `git push -u origin release/v{version}` でリモートへ反映する
4. Gitea 上で `v{version}` タグを付与する

---

## コーディング規約

リポジトリ内の `.cursor/rules/` に、次の規約が定義されている。

| ファイル | 内容 |
| :------- | :--- |
| `imports.mdc` | `use` 文のグループ順・空行・アルファベット順 |
| `comments.mdc` | コメントの文体 (常体)、セクション区切り、`///` の書き方 |

Pull Request 前に [品質チェック](#品質チェック) を通すこと。
