# clip-refiner-core

`ClipRefiner` はデスクトップ常駐アプリが主用途だが、加工ロジックは GUI 非依存の **`clip-refiner-core`** クレート (`clip_refiner_core`) として他プロジェクトから呼び出せる。`clip-refiner` パッケージはアプリ起動 (`run`) とトレイ常駐 UI (feature `app`) 専用で、core の API は re-export しない。

以前の `clip-refiner` (`default-features = false`) による facade 経路は廃止済み。ライブラリ利用は依存先を `clip-refiner-core` へ切り替える。アプリ起動だけなら従来どおり `clip_refiner::run` (feature `app`) を使う。

ライブラリ API は [Semantic Versioning](https://semver.org/lang/ja/) に従う。破壊的変更は [CHANGELOG.md](../../CHANGELOG.md) の該当バージョンにある **Breaking changes** / **Changed** / **Removed** 節へ明記し、設定スキーマ変更は **Settings** 節にも移行方法を記載する。アプリ (`clip-refiner`) と加工・設定 (`clip-refiner-core`) の semver はクレートごとに独立して扱う。

## 依存の追加

```toml
[dependencies]
# 既定: テキスト変換 + OS クリップボード I/O + 設定永続化 (Windows)
clip-refiner-core = { path = "crates/clip-refiner-core" }

# テキスト変換のみ (Windows 以外でもビルド可)
# clip-refiner-core = { path = "crates/clip-refiner-core", default-features = false }
```

| feature | 既定 | 内容 |
| :--- | :--- | :--- |
| (なし / `default-features = false`) | — | `RefineMode` などのテキスト変換。`RegexSettings` / `RefineContext::with_regex` |
| `clipboard` | 有効 | `OwnedClipboard` と OS クリップボード本文 I/O (`arboard` / `CF_DIB`)。Windows 専用 |
| `config` | 有効 | `AppConfig` の `config.toml` 永続化、DPAPI、登録クリップ暗号。Windows 専用 |

`clipboard` または `config` を非 Windows で有効にするとコンパイルエラーになる。公開ドキュメントは `cargo doc -p clip-refiner-core --open` で確認する。

## 公開 API 一覧

| 種別 | 名前 | 説明 |
| :--- | :--- | :--- |
| モジュール | `refiner` | 変換モードとテキスト / クリップボード加工 |
| モジュール | `config` | 正規表現型。`config.toml` 永続化は feature `config` |
| 加工 | `RefineMode`, `RefineCategory`, `Refiner` | 単一モードの適用 |
| 加工 | `apply_text_pipeline`, `apply_pipeline_to_text` | 複数モードの連鎖適用 |
| 加工 | `split_pipeline` | テキスト用モード列と末尾画像モードの分割 |
| 加工 | `OrderedValue` | キー順序保持の JSON / YAML 中間表現 |
| 加工 | `refiner::{REGEX_PREVIEW_SAMPLE, RegexPreviewResult, preview_regex_replace, validate_regex_settings}` | 正規表現置換のプレビューと設定検証 (設定 UI と同一エンジン) |
| クリップボード | `OwnedClipboard` (feature `clipboard`), `process_clipboard`, `process_clipboard_pipeline` | OS クリップボード本文 I/O の所有者と、トレイト経由の加工 API |
| 結果 | `ClipboardProcessOutcome`, `ClipboardProcessError` | 加工結果と失敗理由 |
| 設定 | `RegexSettings`, `RegexPreset`, `RegexStep` | 正規表現加工用 (feature 不要) |
| 設定 | `AppConfig`, `NotificationSettings`, `HotkeySettings`, `SelectorWebviewRetention` など (feature `config`) | `config.toml` 対応型 |
| 設定 | `AppConfig::load` (`ConfigLoadOutcome` / `ConfigLoadFallback`)、`AppConfig::save`, `AppConfig::reload_from_disk` (`ConfigDiskReload`) | 設定ファイル I/O。未知キーは `unknown_keys` / `format_unknown_config_keys_message` で警告。移行手順が無い `version` は `unsupported_version_kept` / `format_unsupported_config_version_message` で既存項目を維持して警告する。feature `config` |
| 設定 | `get_config_dir`, `is_portable_mode`, `open_config_file`, `disk_config_modified_time`, `disk_config_content_fingerprint` (`ConfigFileFingerprint`) | 設定ディレクトリ・ファイル操作。内容指紋は外部編集検知用。feature `config` |
| 設定 | `format_hotkey_fixes_message` | ホットキーの不正・重複補正を UI 向け文言にする。feature `config` |
| 設定 | `RefineContext::from_config` (feature `config`), `RefineContext::with_regex` | 加工時コンテキスト構築 |
| エラー | `Error`, `ClipboardProcessError`, `ClipboardIoError`, `HotkeyParseError` | 加工・ホットキーの失敗理由 |
| エラー | `ConfigError`, `CipherError`, `ConfigReloadError`, `AddRegisteredClipError` | 設定 I/O・AEAD の失敗理由。feature `config` |
| 定数 | `CONFIG_VERSION` | 設定スキーマの現行バージョン。feature `config` |
| 上限 | `is_within_clipboard_limit`, `is_within_parser_limit` | 入力サイズ検証 |

アプリ起動は `clip-refiner` の `run()` (feature `app`) を使う。

## 使用例

単一モード:

```rust
use clip_refiner_core::{RefineContext, RefineMode, Refiner};

let ctx = RefineContext::default();
let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
assert_eq!(output, "hello world");
```

加工パイプライン:

```rust
use clip_refiner_core::{apply_text_pipeline, RefineContext, RefineMode};

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
use clip_refiner_core::{AppConfig, RefineContext, apply_text_pipeline};

let load_outcome = AppConfig::load();
let config = load_outcome.config;
let ctx = RefineContext::from_config(&config);
let pipeline = config.effective_pipeline();
let _ = apply_text_pipeline("  hello  ", &pipeline, &ctx);
```

詳細は `cargo doc -p clip-refiner-core --open` で生成される API ドキュメントを参照。
