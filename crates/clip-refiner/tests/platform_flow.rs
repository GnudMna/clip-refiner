//! OS 連携 (ホットキー・自動起動) の統合テスト

use clip_refiner::AppConfig;
use clip_refiner::hotkey_binding::parse_hotkey_binding;
use clip_refiner::test_helpers::{assert_test_hotkeys_register, query_autostart_enabled};

// ======================================================================
// テスト
// ======================================================================

/// デフォルト設定のホットキー文字列がすべて解釈できるようにすること
#[test]
fn default_hotkey_bindings_parse() {
    let config = AppConfig::default();
    let bindings = [
        ("クイックセレクター", &config.hotkeys.quick_selector),
        ("通知切替", &config.hotkeys.notification),
        ("一時停止", &config.hotkeys.pause),
        ("終了", &config.hotkeys.quit),
        ("取り消し", &config.hotkeys.undo),
        ("登録クリップ", &config.hotkeys.clip_selector),
        ("OCR", &config.hotkeys.ocr),
        ("連続記録", &config.hotkeys.continuous_record),
    ];

    for (label, binding) in bindings {
        parse_hotkey_binding(binding).unwrap_or_else(|error| {
            panic!("{label} ({binding}) の解釈に失敗: {error:?}");
        });
    }
}

/// テスト専用 F キー割り当てでグローバルホットキー登録が成功すること
#[test]
fn test_hotkey_bindings_register_with_os() {
    assert_test_hotkeys_register();
}

/// 不正なホットキー文字列はパースエラーを返すようにすること
#[test]
fn invalid_hotkey_binding_returns_parse_error() {
    let error =
        parse_hotkey_binding("NotAValidBinding").expect_err("不正なバインディングは失敗する");
    assert!(matches!(
        error,
        clip_refiner::HotkeyParseError::UnsupportedKey(_)
            | clip_refiner::HotkeyParseError::UnsupportedModifier(_)
    ));
}

/// 自動起動状態の照会がパニックしないようにすること
#[test]
fn autostart_enabled_query_does_not_panic() {
    let _ = query_autostart_enabled();
}
