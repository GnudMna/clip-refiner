//! 設定 UI パッチマージの JavaScript ロジックテスト

use std::path::PathBuf;
use std::process::Command;

// ======================================================================
// テスト
// ======================================================================
/// `settings_patch.js` の純粋ロジックが Node.js 上で期待どおり動作すること
///
/// Node.js が未インストールの環境ではスキップすること
#[test]
fn settings_ui_logic_tests() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir.join("tests/settings_ui_logic.js");

    let Ok(output) = Command::new("node")
        .arg(&script)
        .current_dir(&manifest_dir)
        .output()
    else {
        eprintln!("note: settings_ui_logic_tests をスキップ (Node.js 未インストール)");
        return;
    };

    assert!(
        output.status.success(),
        "settings_ui_logic.js が失敗:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
