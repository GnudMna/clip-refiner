//! 設定 UI 正規表現プレビューの JavaScript ロジックテスト

use std::path::PathBuf;
use std::process::Command;

// ======================================================================
// テスト
// ======================================================================
/// `regex_preview.js` の純粋ロジックが Node.js 上で期待どおり動作すること
///
/// Node.js が未インストールの環境ではスキップすること
#[test]
fn regex_preview_js_logic_tests() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir.join("tests/regex_preview_logic.js");

    let Ok(output) = Command::new("node")
        .arg(&script)
        .current_dir(&manifest_dir)
        .output()
    else {
        eprintln!("note: regex_preview_js_logic_tests をスキップ (Node.js 未インストール)");
        return;
    };

    assert!(
        output.status.success(),
        "regex_preview_logic.js が失敗:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
