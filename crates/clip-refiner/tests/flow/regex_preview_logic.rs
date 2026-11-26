//! 設定 UI 正規表現プレビューの TypeScript ロジックテスト

use clip_refiner::test_helpers::run_js_test_script;

// ======================================================================
// テスト
// ======================================================================

/// `regex_preview.ts` の純粋ロジックが Node.js 上で期待どおり動作すること
#[test]
fn regex_preview_js_logic_tests() {
    run_js_test_script("regex_preview_logic.ts");
}
