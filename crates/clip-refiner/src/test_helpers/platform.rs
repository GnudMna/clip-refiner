//! OS 連携の統合テスト用ヘルパー

use crate::config::HotkeySettings;
use crate::tray::hotkey::HotkeyHandler;

// ======================================================================
// パブリック関数
// ======================================================================
/// テスト専用の `Alt+Ctrl+Shift` 系割り当てでグローバルホットキー登録が成功することを検証する
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

/// 常駐中の本体や他アプリと衝突しにくいキー割り当てを返す
///
/// 本体の既定 (固定キー・お気に入りスロットとも `Alt+Shift` 系) と重ならないよう `Alt+Ctrl+Shift` 系を使う
fn test_hotkey_settings() -> HotkeySettings {
    HotkeySettings {
        quick_selector: "Alt+Ctrl+Shift+A".to_string(),
        notification: "Alt+Ctrl+Shift+B".to_string(),
        auto_refine: "Alt+Ctrl+Shift+C".to_string(),
        quit: "Alt+Ctrl+Shift+D".to_string(),
        undo: "Alt+Ctrl+Shift+E".to_string(),
        clip_selector: "Alt+Ctrl+Shift+F".to_string(),
        ocr: "Alt+Ctrl+Shift+G".to_string(),
        continuous_record: "Alt+Ctrl+Shift+H".to_string(),
        continuous_record_undo: "Alt+Ctrl+Shift+I".to_string(),
        continuous_record_paste: "Alt+Ctrl+Shift+J".to_string(),
        favorite_mode_slots: Vec::new(),
    }
}
