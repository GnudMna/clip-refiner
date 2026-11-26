//! 設定画面の共有状態 (`ctx.ts`) と、それを参照する DOM 非依存モジュールの Node.js テスト

use clip_refiner::test_helpers::run_js_test_script;

// ======================================================================
// テスト
// ======================================================================

/// `ctx.ts` / `html_builders.ts` / `limits.ts` / `regex_validation.ts` が Node.js 上で期待どおり動作すること
#[test]
fn settings_context_js_logic_tests() {
    run_js_test_script("settings_context_logic.ts");
}
