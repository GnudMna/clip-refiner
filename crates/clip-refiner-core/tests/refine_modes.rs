//! 公開 API 経由の全 `RefineMode` 回帰テスト

mod common;

/// クレート外部から全加工モードを呼び出したとき期待どおり変換されること
#[test]
fn all_refine_modes_regression() {
    common::run_all_refine_mode_regression();
}

/// 新規モード追加時に回帰ケース追加漏れを検知すること
#[test]
fn all_refine_modes_have_cases() {
    common::assert_all_modes_covered();
}
