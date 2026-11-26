//! 設定 UI パネル / イベント純粋ロジックの Node.js テスト

use clip_refiner::test_helpers::run_js_test_script;

// ======================================================================
// テスト
// ======================================================================

/// `panels_logic.ts` が Node.js 上で期待どおり動作すること
#[test]
fn settings_panels_logic_js_tests() {
    run_js_test_script("settings_panels_logic.ts");
}
