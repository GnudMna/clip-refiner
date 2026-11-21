//! クリップボード加工ロジックと設定型を提供する GUI 非依存クレート
//!
//! 既定 feature は `clipboard` と `config`。テキスト変換だけ使う場合は
//! `default-features = false` にする
//! デスクトップ常駐 UI は `clip-refiner` パッケージ (feature `app`) を参照
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

#[cfg(all(
    not(target_os = "windows"),
    any(feature = "clipboard", feature = "config")
))]
compile_error!("clip-refiner-core の clipboard / config feature は Windows 専用です");

pub mod clip_template;
pub mod config;
pub mod consts;
pub mod error;
pub mod hotkey_binding;
pub use hotkey_binding::{
    HotkeyKey, HotkeyModifiers, HotkeyParseError, ParsedHotkey, parse_hotkey_binding,
    parse_hotkey_or_default, parse_hotkey_or_default_labeled,
};
#[cfg(any(feature = "clipboard", feature = "config"))]
mod native;
pub mod refiner;
pub mod security;

pub use clip_template::{
    ClipTemplateContext, ClipTemplateError, clip_template_uses_clipboard,
    clip_template_uses_counter, expand_clip_template,
};
#[cfg(feature = "config")]
pub use config::{
    AppConfig, CONFIG_VERSION, ConfigDiskReload, ConfigFileFingerprint, ConfigLoadFallback,
    ConfigLoadOutcome, ConfigReloadError, FavoriteMoveDirection, FavoriteToggleResult, HotkeyFix,
    HotkeyFixKind, HotkeySettings, MonitorMode, NotificationSettings, ResolvedClip,
    SelectorWebviewRetention, UpdateRegisteredClipError, disk_config_content_fingerprint,
    disk_config_modified_time, format_hotkey_fixes_message, format_unknown_config_keys_message,
    format_unsupported_config_version_message, get_config_dir, is_portable_mode, open_config_file,
};
pub use config::{RegexPreset, RegexSettings, RegexStep};
pub use error::Error;
#[cfg(feature = "config")]
pub use error::{AddRegisteredClipError, CipherError, ConfigError};
#[cfg(feature = "clipboard")]
pub use refiner::OwnedClipboard;
pub use refiner::{
    ClipboardIoError, ClipboardProcessError, ClipboardProcessOutcome, OrderedValue, ParseFailure,
    REGEX_PREVIEW_SAMPLE, RefineCategory, RefineContext, RefineMode, Refiner, RegexPreviewResult,
    apply_pipeline_to_text, apply_text_pipeline, preview_regex_replace, process_clipboard,
    process_clipboard_pipeline, split_pipeline,
};
pub use security::{is_within_clipboard_limit, is_within_parser_limit};

// テスト専用 API。デバッグビルドでも見えるため、利用者向けドキュメントには出さない
#[cfg(any(test, debug_assertions, feature = "test-helpers"))]
#[doc(hidden)]
pub mod test_helpers;
