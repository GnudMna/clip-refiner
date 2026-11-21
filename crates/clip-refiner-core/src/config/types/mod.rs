//! 設定関連の型定義
//!
//! 正規表現型は加工 API が常に利用する。それ以外は feature `config`

#[cfg(feature = "config")]
mod app_config;
#[cfg(feature = "config")]
mod favorites;
#[cfg(feature = "config")]
mod hotkeys;
#[cfg(feature = "config")]
mod monitor;
#[cfg(feature = "config")]
mod notification;
mod regex;
#[cfg(feature = "config")]
mod registered_clip;
#[cfg(feature = "config")]
mod selector_webview;

#[cfg(feature = "config")]
pub use app_config::AppConfig;
#[cfg(feature = "config")]
pub use favorites::{FavoriteMoveDirection, FavoriteToggleResult};
#[cfg(feature = "config")]
pub use hotkeys::{HotkeyFix, HotkeyFixKind, HotkeySettings, format_hotkey_fixes_message};
#[cfg(feature = "config")]
pub use monitor::MonitorMode;
#[cfg(feature = "config")]
pub use notification::NotificationSettings;
pub use regex::{RegexPreset, RegexSettings, RegexStep};
#[cfg(feature = "config")]
pub use registered_clip::{AddRegisteredClipError, RegisteredClip, ResolvedClip};
#[cfg(feature = "config")]
pub use selector_webview::SelectorWebviewRetention;
