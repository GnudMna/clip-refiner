//! 設定ウィンドウの情報タブに載せた著作権表記とライセンス形態が `LICENSE` と一致することのテスト
//!
//! `LICENSE` だけ更新して `panels.js` の表記を直し忘れた状態を `cargo test` で検出する

use std::fs;
use std::path::{Path, PathBuf};

// ======================================================================
// 定数
// ======================================================================

/// ライセンスカードを組み立てる UI ソース (リポジトリルート相対)
const PANELS_JS: &str = "crates/clip-refiner/src/ui/settings/panels.js";

/// 情報タブに載せる `LICENSE` 冒頭の行数 (著作権表記とライセンス形態)
const NOTICE_LINES: usize = 2;

// ======================================================================
// ヘルパー
// ======================================================================

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ======================================================================
// テスト
// ======================================================================

/// `LICENSE` の著作権表記とライセンス形態 (冒頭 2 行) が情報タブのライセンスカードに載っていること
#[test]
fn about_panel_shows_copyright_and_license_type() {
    let root = repo_root();
    let license = fs::read_to_string(root.join("LICENSE")).expect("LICENSE を読めない");
    let panels = fs::read_to_string(root.join(PANELS_JS)).expect("panels.js を読めない");

    let lines: Vec<&str> = license
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(NOTICE_LINES)
        .collect();
    assert_eq!(lines.len(), NOTICE_LINES, "LICENSE の冒頭が足りない");

    for line in lines {
        let paragraph = format!("<p class=\"hint\">{line}</p>");
        assert!(panels.contains(&paragraph), "情報タブに表記が無い: {line}");
    }
}
