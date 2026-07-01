//! クリップボード加工ロジックと設定型を提供する GUI 非依存クレート
//!
//! デスクトップ常駐 UI は [`clip-refiner`](https://github.com/) パッケージ (feature `app`) を参照。
//! 加工 API の利用例はクレートルートのドキュメントコメントを参照
//!
//! # 単一モードの加工
//!
//! ```
//! use clip_refiner_core::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
//! assert_eq!(output, "hello world");
//! ```
//!
//! # 加工パイプライン
//!
//! ```
//! use clip_refiner_core::{apply_text_pipeline, RefineContext, RefineMode};
//!
//! let ctx = RefineContext::default();
//! let result = apply_text_pipeline(
//!     "  %E3%81%82  ",
//!     &[RefineMode::UrlDecode, RefineMode::Trim],
//!     &ctx,
//! );
//! assert_eq!(result.as_deref(), Some("あ"));
//! ```

#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub mod config;
pub mod consts;
pub mod hotkey_binding;
mod platform;
pub mod refiner;
pub mod security;

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

#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
pub mod test_helpers;
