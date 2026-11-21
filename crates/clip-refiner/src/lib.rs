//! クリップボードのテキストを加工するツール `ClipRefiner` のライブラリクレート
//!
//! 加工ロジックと設定型は [`clip_refiner_core`] クレートに分離されている
//! feature `app` (デフォルト有効) ではトレイ常駐 UI と [`run`] を提供する
//!
//! # ライブラリのみ利用する場合
//!
//! GUI 依存を避けるには `default-features = false` を指定し、
//! [`clip-refiner-core`](clip_refiner_core) 相当の API のみをリンクする
//!
//! ```toml
//! [dependencies]
//! clip-refiner = { version = "0.12.2", default-features = false }
//! ```
//!
//! または [`clip-refiner-core`] を直接依存に追加する
//!
//! ```toml
//! [dependencies]
//! clip-refiner-core = { version = "0.12.2" }
//! ```
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
//! # 互換性
//!
//! ライブラリ API は [`Cargo.toml`] の semver に従う。破壊的変更は CHANGELOG の
//! **Breaking changes** 節に記載する

#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

#[cfg(feature = "app")]
mod bootstrap;
#[cfg(feature = "app")]
mod logger;
#[cfg(feature = "app")]
mod platform;
#[cfg(feature = "app")]
mod tray;
#[cfg(feature = "app")]
mod user_error;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
#[cfg(feature = "app")]
pub mod test_helpers;

pub use clip_refiner_core::config;
pub use clip_refiner_core::consts;
pub use clip_refiner_core::hotkey_binding;
pub use clip_refiner_core::refiner;
pub use clip_refiner_core::security;
pub use clip_refiner_core::{
    AddRegisteredClipError, AppConfig, CONFIG_VERSION, ClipboardIoError, ClipboardProcessError,
    ClipboardProcessOutcome, ConfigError, ConfigLoadFallback, ConfigLoadOutcome, ConfigReloadError,
    Error, FavoriteMoveDirection, FavoriteToggleResult, HotkeyParseError, HotkeySettings,
    MonitorMode, NotificationSettings, OrderedValue, RefineCategory, RefineContext, RefineMode,
    Refiner, RegexSettings, ResolvedClip, apply_pipeline_to_text, apply_text_pipeline,
    disk_config_modified_time, get_config_dir, is_within_clipboard_limit, is_within_parser_limit,
    open_config_file, process_clipboard, process_clipboard_pipeline, split_pipeline,
};

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションを起動する
///
/// ロギングの初期化、コマンドライン引数の解析、常駐またはワンショット実行を行う
///
/// feature `app` が必要
///
/// # Returns
/// * `anyhow::Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
///
/// # Errors
/// ログ初期化、引数解析、クリップボード初期化、常駐ループのいずれかが失敗した場合
#[cfg(feature = "app")]
pub fn run() -> anyhow::Result<()> {
    bootstrap::run()
}
