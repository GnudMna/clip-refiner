//! クリップボードのテキストを加工するツール `ClipRefiner` のライブラリクレート
//!
//! デスクトップアプリ (`run()`) としての常駐 UI に加え、加工ロジックを他クレートから
//! 呼び出すための API を [`refiner`] と [`config`] に公開する
//!
//! # クレート構成
//!
//! | モジュール | 用途 |
//! | :--------- | :--- |
//! | [`refiner`] | 加工モード ([`RefineMode`]) とテキスト / クリップボード加工 |
//! | [`config`] | [`AppConfig`] など設定型と `config.toml` の読み書き |
//! | [`run`] | トレイ常駐アプリの起動 (CLI と同等) |
//!
//! よく使う型はクレートルートからも re-export されている
//!
//! # 単一モードの加工
//!
//! [`Refiner`] トレイトで 1 モードを適用する
//!
//! ```
//! use clip_refiner::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
//! assert_eq!(output, "hello world");
//! ```
//!
//! # 加工パイプライン
//!
//! 複数モードを順に適用する。変更がなければ [`apply_text_pipeline`] は `None` を返す
//!
//! ```
//! use clip_refiner::{apply_text_pipeline, RefineContext, RefineMode};
//!
//! let ctx = RefineContext::default();
//! let result = apply_text_pipeline(
//!     "  %E3%81%82  ",
//!     &[RefineMode::UrlDecode, RefineMode::Trim],
//!     &ctx,
//! );
//! assert_eq!(result.as_deref(), Some("あ"));
//! ```
//!
//! 入力サイズ検証付きで [`ClipboardProcessOutcome`] を返す場合は [`apply_pipeline_to_text`] を使う
//!
//! # 設定連携
//!
//! ```
//! use clip_refiner::{AppConfig, RefineContext, RefineMode, Refiner, apply_text_pipeline};
//!
//! let config = AppConfig::default();
//! let ctx = RefineContext::from_config(&config);
//! let pipeline = config.effective_pipeline();
//! let _ = apply_text_pipeline("  hello  ", &pipeline, &ctx);
//! assert_eq!(RefineMode::UrlDecode.refine("a%2Fb", &ctx), "a/b");
//! ```
//!
//! # クリップボード
//!
//! `arboard::Clipboard` に対して加工結果を書き戻す場合は [`process_clipboard`] または
//! [`process_clipboard_pipeline`] を使う
//!
//! # 互換性
//!
//! ライブラリ API は [`Cargo.toml`] の semver に従う。破壊的変更は CHANGELOG の
//! **Breaking changes** 節に記載する

// アプリケーション本体を lib 化しているため、bin では出ない pedantic (must_use / Errors 節) を抑制する
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

mod autostart;
mod bootstrap;
pub mod config;
mod consts;
mod hotkey_binding;
mod logger;
mod platform;
pub mod refiner;
mod security;
mod tray;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub mod test_helpers;

pub use config::{
    AppConfig, CONFIG_VERSION, ConfigReloadError, FavoriteMoveDirection, FavoriteToggleResult,
    HotkeySettings, MonitorMode, NotificationSettings, RegexSettings, ResolvedClip,
    disk_config_modified_time, get_config_dir, open_config_file,
};
pub use refiner::{
    ClipboardProcessError, ClipboardProcessOutcome, OrderedValue, RefineCategory, RefineContext,
    RefineMode, Refiner, apply_pipeline_to_text, apply_text_pipeline, process_clipboard,
    process_clipboard_pipeline, split_pipeline,
};
pub use security::{is_within_clipboard_limit, is_within_parser_limit};

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションを起動する
///
/// ロギングの初期化、コマンドライン引数の解析、常駐またはワンショット実行を行う
///
/// # Returns
/// * `anyhow::Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
pub fn run() -> anyhow::Result<()> {
    bootstrap::run()
}
