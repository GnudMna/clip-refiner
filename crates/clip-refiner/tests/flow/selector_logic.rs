//! セレクター UI 純粋ロジックの Node.js テスト

use clip_refiner::test_helpers::run_js_test_script;

// ======================================================================
// テスト
// ======================================================================

/// `selector_common` / `quick_selector_logic` / `clip_selector_logic` が Node.js 上で期待どおり動作すること
#[test]
fn selector_js_logic_tests() {
    run_js_test_script("selector_logic.ts");
}
