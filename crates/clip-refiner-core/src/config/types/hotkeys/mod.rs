//! グローバルホットキー設定と正規化
//!
//! [`HotkeySettings`] の既定値・不正 / 重複の補正・設定 UI 向け修正メッセージを担う

mod fix;
mod normalize;
mod settings;

pub use fix::{HotkeyFix, HotkeyFixKind, format_hotkey_fixes_message};
pub use settings::HotkeySettings;
