use super::convert::to_global_hotkey;
use crate::config::HotkeySettings;
use crate::consts;

use clip_refiner_core::hotkey_binding::parse_hotkey_or_default_labeled;

use global_hotkey::hotkey::HotKey;

// ======================================================================
// 解決済みホットキー
// ======================================================================
/// 設定から解決した固定ホットキー割り当て
pub(super) struct ResolvedHotkeys {
    pub quick_selector: HotKey,
    pub notification: HotKey,
    pub pause: HotKey,
    pub quit: HotKey,
    pub undo: HotKey,
    pub clip_selector: HotKey,
    pub continuous_record: HotKey,
    pub ocr: HotKey,
}

impl ResolvedHotkeys {
    /// 設定から各ホットキーを解決する
    ///
    /// # Arguments
    /// * `hotkeys` - 設定ファイルから読み込んだホットキー割り当て
    ///
    /// # Returns
    /// * `Self` - 解決済み固定ホットキー割り当て
    pub fn from_settings(hotkeys: &HotkeySettings) -> Self {
        Self {
            quick_selector: resolve_hotkey(
                &hotkeys.quick_selector,
                consts::DEFAULT_HOTKEY_QUICK_SELECTOR,
                "quick_selector",
            ),
            notification: resolve_hotkey(
                &hotkeys.notification,
                consts::DEFAULT_HOTKEY_NOTIFICATION,
                "notification",
            ),
            pause: resolve_hotkey(&hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE, "pause"),
            quit: resolve_hotkey(&hotkeys.quit, consts::DEFAULT_HOTKEY_QUIT, "quit"),
            undo: resolve_hotkey(&hotkeys.undo, consts::DEFAULT_HOTKEY_UNDO, "undo"),
            clip_selector: resolve_hotkey(
                &hotkeys.clip_selector,
                consts::DEFAULT_HOTKEY_CLIP_SELECTOR,
                "clip_selector",
            ),
            continuous_record: resolve_hotkey(
                &hotkeys.continuous_record,
                consts::DEFAULT_HOTKEY_CONTINUOUS_RECORD,
                "continuous_record",
            ),
            ocr: resolve_hotkey(&hotkeys.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr"),
        }
    }
}

/// 設定値を解析し、失敗時はデフォルトへフォールバックする
fn resolve_hotkey(binding: &str, default_binding: &str, label: &str) -> HotKey {
    to_global_hotkey(parse_hotkey_or_default_labeled(
        binding,
        default_binding,
        label,
    ))
}
