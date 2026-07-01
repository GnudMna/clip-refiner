use crate::config::HotkeySettings;
use crate::consts;
use crate::hotkey_binding::resolve_hotkey;

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
    #[cfg(screen_ocr)]
    pub ocr: HotKey,
}

impl ResolvedHotkeys {
    /// 設定から各ホットキーを解決する
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
            #[cfg(screen_ocr)]
            ocr: resolve_hotkey(&hotkeys.ocr, consts::DEFAULT_HOTKEY_OCR, "ocr"),
        }
    }

    /// 登録済みホットキーを配列として返す
    pub fn registered_hotkeys(&self) -> Vec<HotKey> {
        let mut hotkeys = vec![
            self.quick_selector,
            self.notification,
            self.pause,
            self.quit,
            self.undo,
            self.clip_selector,
        ];
        #[cfg(screen_ocr)]
        hotkeys.push(self.ocr);
        hotkeys
    }
}
