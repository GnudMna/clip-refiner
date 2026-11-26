//! Node.js 上で実行する UI ロジックテスト用ランナー

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// ======================================================================
// パブリック関数
// ======================================================================
/// `tests/ui/` 配下の ESM TypeScript テストスクリプトを Node.js で実行する
///
/// Node.js 未インストール時は `CLIP_REFINER_REQUIRE_NODE=1` のときのみ失敗する
/// 意図的にスキップする場合は `CLIP_REFINER_SKIP_NODE_TESTS=1` を設定する
///
/// # Arguments
/// * `script_name` - `tests/ui/` からの相対パス (例: `settings_ui_logic.ts`)
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js が見つからない、またはスクリプトが非ゼロ終了で終了した場合
pub fn run_js_test_script(script_name: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script = manifest_dir.join("tests/ui").join(script_name);
    run_js_script_at(&manifest_dir, &script);
}

/// 指定パスの TypeScript テストスクリプトを Node.js で実行する
///
/// # Arguments
/// * `working_dir` - `node` 実行時のカレントディレクトリ
/// * `script` - 実行するスクリプトの絶対パス
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js が見つからない、またはスクリプトが非ゼロ終了で終了した場合
pub fn run_js_script_at(working_dir: &Path, script: &Path) {
    let label = script.display().to_string();
    let Some(output) = run_node(working_dir, &[script.as_os_str()], &label) else {
        return;
    };

    assert!(
        output.status.success(),
        "UI ロジックテストが失敗:\nscript: {}\nstdout: {}\nstderr: {}",
        script.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// リポジトリの `node_modules` にある TypeScript で型チェックだけを行う (`tsc --noEmit -p`)
///
/// Node.js または `typescript` パッケージ (`npm ci` で入る) が無いときは
/// `CLIP_REFINER_REQUIRE_NODE=1` のときのみ失敗する
///
/// # Arguments
/// * `project` - `tsconfig.json` の絶対パス
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js / TypeScript が見つからない、または型エラーがある場合
pub fn run_typescript_check(project: &Path) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tsc = repo_root.join("node_modules/typescript/bin/tsc");
    let label = project.display().to_string();
    if !tsc.is_file() {
        assert!(
            !require_node(),
            "TypeScript が見つかりません ({})。リポジトリ直下で `npm ci` を実行してください",
            tsc.display()
        );
        eprintln!("note: {label} の型チェックをスキップ (`npm ci` 未実行で TypeScript が無い)");
        return;
    }

    let Some(output) = run_node(
        &repo_root,
        &[
            tsc.as_os_str(),
            "--noEmit".as_ref(),
            "--pretty".as_ref(),
            "false".as_ref(),
            "-p".as_ref(),
            project.as_os_str(),
        ],
        &label,
    ) else {
        return;
    };

    assert!(
        output.status.success(),
        "TypeScript の型チェックが失敗:\nproject: {}\n{}{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `CLIP_REFINER_REQUIRE_NODE=1` が設定されているか
fn require_node() -> bool {
    std::env::var("CLIP_REFINER_REQUIRE_NODE").is_ok_and(|value| value == "1")
}

/// Node.js を実行する
///
/// スキップ条件 (`CLIP_REFINER_SKIP_NODE_TESTS=1`、または Node.js 未インストールかつ
/// `CLIP_REFINER_REQUIRE_NODE` 未設定) では実行せず `None` を返す
///
/// # Arguments
/// * `working_dir` - `node` 実行時のカレントディレクトリ
/// * `args` - `node` に渡す引数
/// * `label` - スキップ時のメッセージに出す対象名
///
/// # Returns
/// * `Option<Output>` - 実行した場合は終了状態と出力
///
/// # Panics
/// `CLIP_REFINER_REQUIRE_NODE=1` かつ Node.js が見つからない場合
fn run_node(working_dir: &Path, args: &[&OsStr], label: &str) -> Option<Output> {
    if std::env::var("CLIP_REFINER_SKIP_NODE_TESTS").is_ok_and(|value| value == "1") {
        eprintln!("note: {label} をスキップ (CLIP_REFINER_SKIP_NODE_TESTS=1)");
        return None;
    }

    match Command::new("node")
        .args(args)
        .current_dir(working_dir)
        .output()
    {
        Ok(output) => Some(output),
        Err(error) if require_node() => {
            panic!(
                "Node.js が見つかりません。品質チェックでは UI ロジックテストに Node.js が必要です: {error}"
            );
        }
        Err(_) => {
            eprintln!(
                "note: {label} をスキップ (Node.js 未インストール。品質チェック時は Node.js を用意する)"
            );
            None
        }
    }
}
