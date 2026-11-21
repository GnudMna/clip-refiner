//! 公開 API 経由の全 `RefineMode` 回帰テスト

mod common;

// ======================================================================
// テスト
// ======================================================================

/// クレート外部から全変換モードを呼び出したとき期待どおり変換されるようにすること
#[test]
fn all_refine_modes_regression() {
    common::run_all_refine_mode_regression();
}

/// 新規モード追加時に回帰ケース追加漏れを検知できるようにすること
#[test]
fn all_refine_modes_have_cases() {
    common::assert_all_modes_covered();
}
