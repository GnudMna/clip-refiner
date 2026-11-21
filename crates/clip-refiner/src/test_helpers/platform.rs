//! OS 連携の統合テスト用ヘルパー

use crate::config::HotkeySettings;
use crate::tray::hotkey::HotkeyHandler;

// ======================================================================
// パブリック関数
// ======================================================================
/// テスト専用の F キー割り当てでグローバルホットキー登録が成功することを検証する
///
/// 登録に失敗したホットキーがある場合はパニックする
pub fn assert_test_hotkeys_register() {
    let hotkeys = test_hotkey_settings();
    let (handler, failures) =
        HotkeyHandler::new(&hotkeys, &[]).expect("テスト用ホットキーハンドラの初期化に失敗");
    assert!(
        failures.is_empty(),
        "テスト用ホットキーの登録に失敗: {failures:?}"
    );
    drop(handler);
}

/// 自動起動が有効かどうかを OS から照会する
///
/// # Returns
/// * `bool` - Run キーに登録されている場合は `true`
#[must_use]
pub fn query_autostart_enabled() -> bool {
    crate::platform::autostart::is_enabled()
}

/// 現在の環境で `WebView2` Runtime が検出できるかどうかを返す
///
/// # Returns
/// * `bool` - レジストリ上 Runtime が利用可能な場合は `true`
#[must_use]
pub fn query_webview2_runtime_available() -> bool {
    crate::platform::webview2::is_runtime_available()
}

/// 本番デフォルトや他アプリと衝突しにくい F キー割り当てを返す
///
/// `Alt+Shift+F10` は環境によって OS / 他プロセスが占有していることがあるため使わない
fn test_hotkey_settings() -> HotkeySettings {
    HotkeySettings {
        quick_selector: "Alt+Shift+F1".to_string(),
        notification: "Alt+Shift+F2".to_string(),
        pause: "Alt+Shift+F3".to_string(),
        quit: "Alt+Shift+F4".to_string(),
        undo: "Alt+Shift+F5".to_string(),
        clip_selector: "Alt+Shift+F6".to_string(),
        ocr: "Alt+Shift+F7".to_string(),
        continuous_record: "Alt+Shift+F8".to_string(),
        continuous_record_undo: "Alt+Shift+F9".to_string(),
        continuous_record_paste: "Alt+Shift+F12".to_string(),
        favorite_mode_slots: Vec::new(),
    }
}
