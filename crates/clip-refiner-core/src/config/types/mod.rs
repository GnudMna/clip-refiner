//! 設定関連の型定義

mod app_config;
mod favorites;
mod hotkeys;
mod monitor;
mod notification;
mod regex;
mod registered_clip;
mod selector_webview;

pub use app_config::AppConfig;
pub use favorites::{FavoriteMoveDirection, FavoriteToggleResult};
pub use hotkeys::{HotkeyFix, HotkeyFixKind, HotkeySettings, format_hotkey_fixes_message};
pub use monitor::MonitorMode;
pub use notification::NotificationSettings;
pub use regex::RegexSettings;
pub use registered_clip::{AddRegisteredClipError, RegisteredClip, ResolvedClip};
pub use selector_webview::SelectorWebviewRetention;
