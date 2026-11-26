//! ポータブル差し替えの引数検証
//!
//! 受け取った展開元・差し替え先・削除対象が更新作業フォルダと
//! ポータブルインストールの前提を満たすかを判定する

use std::fs;
use std::path::{Path, PathBuf};

use clip_refiner_core::consts::PORTABLE_MARKER_FILE;

use super::super::UPDATE_WORK_DIR_NAME;
use super::super::error::UpdateError;
use super::files::find_portable_exe;

// ======================================================================
// 引数検証
// ======================================================================
/// 差し替え元・先・削除対象が安全か検証する
///
/// 展開元は `%TEMP%\clip-refiner-update\` 配下、差し替え先は既存のポータブル
/// フォルダ (`ClipRefiner.exe` と `portable` マーカー) に限る
/// 削除対象がある場合は同じ更新作業フォルダ配下かつ展開元の祖先である必要がある
pub(super) fn validate_replace_request(
    source_dir: &Path,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    let work_root = canonicalize_dir(&update_work_root()).map_err(|_| {
        tracing::warn!("更新作業フォルダが存在しない");
        UpdateError::SourceDirInvalid
    })?;
    let source = canonicalize_dir(source_dir).map_err(|_| {
        tracing::warn!("展開元を解決できない: {}", source_dir.display());
        UpdateError::SourceDirInvalid
    })?;
    let install = canonicalize_dir(install_dir).map_err(|_| {
        tracing::warn!("差し替え先を解決できない: {}", install_dir.display());
        UpdateError::InstallDirInvalid
    })?;

    if source == install {
        return Err(UpdateError::SameReplaceDirs);
    }
    if !is_strict_descendant(&source, &work_root) {
        tracing::warn!("展開元が更新作業フォルダの外: {}", source.display());
        return Err(UpdateError::SourceDirInvalid);
    }
    if is_path_under(&install, &work_root) {
        tracing::warn!("差し替え先が更新作業フォルダ配下: {}", install.display());
        return Err(UpdateError::InstallDirInvalid);
    }
    if find_portable_exe(&source).is_none() {
        tracing::warn!("展開元に ClipRefiner.exe が無い: {}", source.display());
        return Err(UpdateError::HelperExeMissing);
    }
    if !looks_like_portable_install(&install) {
        tracing::warn!(
            "差し替え先がポータブルフォルダではない: {}",
            install.display()
        );
        return Err(UpdateError::InstallDirInvalid);
    }

    if let Some(cleanup) = cleanup_dir {
        validate_cleanup_dir(cleanup, &source, &work_root)?;
    }

    Ok(())
}

/// 削除対象が更新作業フォルダ配下かつ展開元の祖先か検証する
fn validate_cleanup_dir(
    cleanup_dir: &Path,
    source: &Path,
    work_root: &Path,
) -> Result<(), UpdateError> {
    let cleanup = canonicalize_dir(cleanup_dir).map_err(|_| {
        tracing::warn!("削除対象を解決できない: {}", cleanup_dir.display());
        UpdateError::CleanupDirInvalid
    })?;
    if !is_strict_descendant(&cleanup, work_root) {
        tracing::warn!("削除対象が更新作業フォルダの外: {}", cleanup.display());
        return Err(UpdateError::CleanupDirInvalid);
    }
    if !is_path_under(source, &cleanup) {
        tracing::warn!(
            "削除対象が展開元の祖先ではない: cleanup={} source={}",
            cleanup.display(),
            source.display()
        );
        return Err(UpdateError::CleanupDirInvalid);
    }
    Ok(())
}

// ======================================================================
// パス判定
// ======================================================================
/// `%TEMP%\clip-refiner-update` を返す
pub(crate) fn update_work_root() -> PathBuf {
    std::env::temp_dir().join(UPDATE_WORK_DIR_NAME)
}

/// 既存ディレクトリを正規化する
pub(crate) fn canonicalize_dir(path: &Path) -> Result<PathBuf, UpdateError> {
    if !path.is_dir() {
        return Err(UpdateError::PathInvalid);
    }
    fs::canonicalize(path).map_err(|_| UpdateError::PathInvalid)
}

/// `path` が `ancestor` 自身またはその配下か
fn is_path_under(path: &Path, ancestor: &Path) -> bool {
    path.starts_with(ancestor)
}

/// `path` が `ancestor` の真の子孫か (同一は含まない)
pub(crate) fn is_strict_descendant(path: &Path, ancestor: &Path) -> bool {
    is_path_under(path, ancestor) && path != ancestor
}

/// 差し替え先が既存のポータブルインストールか
fn looks_like_portable_install(install_dir: &Path) -> bool {
    find_portable_exe(install_dir).is_some() && install_dir.join(PORTABLE_MARKER_FILE).is_file()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::test_support::{cleanup_layout, prepare_valid_replace_layout, unique_name};
    use super::*;

    /// 正規の展開元とポータブル差し替え先を受け入れるようにすること
    #[test]
    fn validate_replace_request_accepts_portable_layout() {
        let (_work_root, work_dir, source, install) = prepare_valid_replace_layout();
        validate_replace_request(&source, &install, Some(&work_dir)).expect("valid layout");
        cleanup_layout(&work_dir, &install);
    }

    /// 同一ディレクトリへの差し替えを拒否すること
    #[test]
    fn validate_replace_request_rejects_same_path() {
        let (_work_root, work_dir, source, install) = prepare_valid_replace_layout();
        let err = validate_replace_request(&source, &source, None).expect_err("same");
        assert_eq!(err, UpdateError::SameReplaceDirs);
        cleanup_layout(&work_dir, &install);
    }

    /// 差し替え先に `portable` マーカーが無いと拒否すること
    #[test]
    fn validate_replace_request_rejects_install_without_marker() {
        let (_work_root, work_dir, source, install) = prepare_valid_replace_layout();
        fs::remove_file(install.join(PORTABLE_MARKER_FILE)).expect("remove marker");
        let err =
            validate_replace_request(&source, &install, Some(&work_dir)).expect_err("no marker");
        assert_eq!(err, UpdateError::InstallDirInvalid);
        cleanup_layout(&work_dir, &install);
    }

    /// 差し替え先が更新作業フォルダ配下だと拒否すること
    #[test]
    fn validate_replace_request_rejects_install_under_work_root() {
        let (_work_root, work_dir, source, install) = prepare_valid_replace_layout();
        let nested_install = work_dir.join("fake-install");
        fs::create_dir_all(&nested_install).expect("nested install");
        fs::write(nested_install.join("ClipRefiner.exe"), b"old").expect("exe");
        fs::write(nested_install.join(PORTABLE_MARKER_FILE), b"").expect("marker");
        let err = validate_replace_request(&source, &nested_install, Some(&work_dir))
            .expect_err("install in work");
        assert_eq!(err, UpdateError::InstallDirInvalid);
        cleanup_layout(&work_dir, &install);
    }

    /// 展開元が更新作業フォルダの外だと拒否すること
    #[test]
    fn validate_replace_request_rejects_source_outside_work_root() {
        let (_work_root, work_dir, _source, install) = prepare_valid_replace_layout();
        let outside = std::env::temp_dir().join(unique_name("replace-outside-src"));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("ClipRefiner.exe"), b"new").expect("exe");
        let err = validate_replace_request(&outside, &install, None).expect_err("outside source");
        assert_eq!(err, UpdateError::SourceDirInvalid);
        let _ = fs::remove_dir_all(&outside);
        cleanup_layout(&work_dir, &install);
    }

    /// 削除対象が展開元の祖先でないと拒否すること
    #[test]
    fn validate_replace_request_rejects_unrelated_cleanup_dir() {
        let (_work_root, work_dir, source, install) = prepare_valid_replace_layout();
        let other = update_work_root().join(unique_name("replace-other-cleanup"));
        let _ = fs::remove_dir_all(&other);
        fs::create_dir_all(&other).expect("other");
        let err = validate_replace_request(&source, &install, Some(&other))
            .expect_err("unrelated cleanup");
        assert_eq!(err, UpdateError::CleanupDirInvalid);
        let _ = fs::remove_dir_all(&other);
        cleanup_layout(&work_dir, &install);
    }

    /// 削除対象が更新作業フォルダそのものだと拒否すること
    #[test]
    fn validate_replace_request_rejects_cleanup_at_work_root() {
        let (work_root, work_dir, source, install) = prepare_valid_replace_layout();
        let err = validate_replace_request(&source, &install, Some(&work_root))
            .expect_err("cleanup work root");
        assert_eq!(err, UpdateError::CleanupDirInvalid);
        cleanup_layout(&work_dir, &install);
    }

    /// `clip-refiner-update` と接頭辞が同じ別フォルダを子孫とみなさないようにすること
    #[test]
    fn is_path_under_does_not_match_name_prefix() {
        let ancestor = Path::new(r"C:\Temp\clip-refiner-update");
        let sibling = Path::new(r"C:\Temp\clip-refiner-update-evil\extracted");
        assert!(!is_path_under(sibling, ancestor));
        assert!(!is_strict_descendant(sibling, ancestor));
        assert!(is_strict_descendant(
            Path::new(r"C:\Temp\clip-refiner-update\1.0.0"),
            ancestor
        ));
    }
}
