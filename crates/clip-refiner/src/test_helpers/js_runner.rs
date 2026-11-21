//! Node.js 上で実行する UI ロジックテスト用ランナー

use std::path::{Path, PathBuf};
use std::process::Command;

// ======================================================================
// パブリック関数
// ======================================================================
/// `tests/` 配下の ESM JavaScript テストスクリプトを Node.js で実行する
///
/// Node.js 未インストール時は `CLIP_REFINER_REQUIRE_NODE=1` のときのみ失敗する
/// 意図的にスキップする場合は `CLIP_REFINER_SKIP_NODE_TESTS=1` を設定する
///
/// # Arguments
/// * `script_name` - `tests/` からの相対パス (例: `settings_ui_logic.js`)
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js が見つからない、またはスクリプトが非ゼロ終了で終了した場合
pub fn run_js_test_script(script_name: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir.join("tests").join(script_name);
    run_js_script_at(&manifest_dir, &script);
}

/// 指定パスの JavaScript テストスクリプトを Node.js で実行する
///
/// # Arguments
/// * `working_dir` - `node` 実行時のカレントディレクトリ
/// * `script` - 実行するスクリプトの絶対パス
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js が見つからない、またはスクリプトが非ゼロ終了で終了した場合
pub fn run_js_script_at(working_dir: &Path, script: &Path) {
    if std::env::var("CLIP_REFINER_SKIP_NODE_TESTS").is_ok_and(|value| value == "1") {
        eprintln!(
            "note: {} をスキップ (CLIP_REFINER_SKIP_NODE_TESTS=1)",
            script.display()
        );
        return;
    }

    let require_node = std::env::var("CLIP_REFINER_REQUIRE_NODE").is_ok_and(|value| value == "1");

    let output = match Command::new("node")
        .arg(script)
        .current_dir(working_dir)
        .output()
    {
        Ok(output) => output,
        Err(error) if require_node => {
            panic!(
                "Node.js が見つかりません。品質チェックでは UI ロジックテストに Node.js が必要です: {error}"
            );
        }
        Err(_) => {
            eprintln!(
                "note: {} をスキップ (Node.js 未インストール。品質チェック時は Node.js を用意する)",
                script.display()
            );
            return;
        }
    };

    assert!(
        output.status.success(),
        "JavaScript テストが失敗:\nscript: {}\nstdout: {}\nstderr: {}",
        script.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
