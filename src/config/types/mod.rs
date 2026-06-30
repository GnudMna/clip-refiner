//! 設定関連の型定義

mod app_config;
mod favorites;
mod hotkeys;
mod monitor;
mod notification;
mod regex;
mod registered_clip;

pub use app_config::AppConfig;
pub use favorites::{FavoriteMoveDirection, FavoriteToggleResult};
pub use hotkeys::HotkeySettings;
pub use monitor::MonitorMode;
pub use notification::NotificationSettings;
pub use regex::RegexSettings;
pub use registered_clip::{AddRegisteredClipError, RegisteredClip, ResolvedClip};
