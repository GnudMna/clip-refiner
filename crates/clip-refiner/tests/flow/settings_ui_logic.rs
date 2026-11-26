//! 設定 UI パッチマージの TypeScript ロジックテスト

use clip_refiner::test_helpers::run_js_test_script;

// ======================================================================
// テスト
// ======================================================================

/// `settings_patch.ts` の純粋ロジックが Node.js 上で期待どおり動作すること
#[test]
fn settings_ui_logic_tests() {
    run_js_test_script("settings_ui_logic.ts");
}
