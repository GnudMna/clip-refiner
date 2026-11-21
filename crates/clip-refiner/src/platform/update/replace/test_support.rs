//! 差し替えヘルパーのテスト用ディレクトリ構成
//!
//! 検証を通る展開元・差し替え先・作業ディレクトリを一意な名前で用意する

use std::fs;
use std::path::{Path, PathBuf};

use super::validate::update_work_root;
use crate::consts::PORTABLE_MARKER_FILE;

// ======================================================================
// テスト補助
// ======================================================================
/// テスト用の一意な一時ディレクトリ名を返す
pub(super) fn unique_name(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    )
}

/// 検証に通る展開元・差し替え先・作業ディレクトリを用意する
pub(super) fn prepare_valid_replace_layout() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let work_root = update_work_root();
    let work_dir = work_root.join(unique_name("replace-work"));
    let source = work_dir.join("extracted");
    let install = std::env::temp_dir().join(unique_name("replace-install"));
    let _ = fs::remove_dir_all(&work_dir);
    let _ = fs::remove_dir_all(&install);
    fs::create_dir_all(&source).expect("source");
    fs::create_dir_all(&install).expect("install");
    fs::write(source.join("ClipRefiner.exe"), b"new").expect("source exe");
    fs::write(install.join("ClipRefiner.exe"), b"old").expect("install exe");
    fs::write(install.join(PORTABLE_MARKER_FILE), b"").expect("portable marker");
    (work_root, work_dir, source, install)
}

/// テスト用ディレクトリを削除する
pub(super) fn cleanup_layout(work_dir: &Path, install: &Path) {
    let _ = fs::remove_dir_all(work_dir);
    let _ = fs::remove_dir_all(install);
}
