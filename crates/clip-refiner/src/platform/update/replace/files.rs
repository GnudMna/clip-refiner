//! 差し替えヘルパーのファイル操作
//!
//! 実行ファイルの探索と、展開済み配布物のインストール先へのコピーを担う
//! 利用者データ (`data`) はコピー対象から外す

use std::fs;
use std::path::{Path, PathBuf};

use super::super::error::UpdateError;
use crate::consts::{APP_NAME, PORTABLE_DATA_DIR_NAME};

// ======================================================================
// 実行ファイル
// ======================================================================
/// 実行ファイル名 (`ClipRefiner.exe`)
pub(super) fn portable_exe_name() -> String {
    format!("{APP_NAME}.exe")
}

/// 自身の実行ディレクトリを返す
pub(super) fn current_exe_dir() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|err| {
        crate::log_warn!("実行ファイルパスの取得に失敗: {err}");
        UpdateError::ExePath
    })?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or(UpdateError::ExeDir)
}

/// 展開ルート直下の `ClipRefiner.exe` を探す
pub(super) fn find_portable_exe(source_dir: &Path) -> Option<PathBuf> {
    let direct = source_dir.join(portable_exe_name());
    if direct.is_file() {
        return Some(direct);
    }

    let entries = fs::read_dir(source_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(&portable_exe_name()) {
            return Some(path);
        }
    }
    None
}

/// パスをコマンドライン引数用の文字列にする
pub(super) fn path_to_string(path: &Path) -> Result<String, UpdateError> {
    path.to_str()
        .map(str::to_string)
        .ok_or(UpdateError::PathInvalid)
}

// ======================================================================
// コピー
// ======================================================================
/// ポータブル更新の配布物をインストール先へコピーする
///
/// 送信元の `data` はスキップする。宛先に既にある `data` は保持する
/// 差し替え先ディレクトリは既存である必要がある (新規作成しない)
///
/// # Arguments
/// * `source_dir` - 展開ルート
/// * `dest_dir` - 差し替え先
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(super) fn copy_portable_payload(source_dir: &Path, dest_dir: &Path) -> Result<(), UpdateError> {
    if !source_dir.is_dir() {
        return Err(UpdateError::SourceMissing);
    }
    if !dest_dir.is_dir() {
        crate::log_warn!("差し替え先がディレクトリではない: {}", dest_dir.display());
        return Err(UpdateError::DestPrepare);
    }

    let entries = fs::read_dir(source_dir).map_err(|err| {
        crate::log_warn!("展開元の列挙に失敗: {err}");
        UpdateError::SourceRead
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            crate::log_warn!("展開元エントリの取得に失敗: {err}");
            UpdateError::SourceRead
        })?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case(PORTABLE_DATA_DIR_NAME))
        {
            continue;
        }

        let from = entry.path();
        let to = dest_dir.join(&name);
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    crate::log_warn!("差し替え先親ディレクトリの作成に失敗: {err}");
                    UpdateError::FileReplace
                })?;
            }
            fs::copy(&from, &to).map_err(|err| {
                crate::log_warn!(
                    "ファイルコピー失敗: {} -> {}: {err}",
                    from.display(),
                    to.display()
                );
                UpdateError::FileCopy {
                    name: name.to_string_lossy().into_owned(),
                }
            })?;
        }
    }

    Ok(())
}

/// ディレクトリを再帰コピーする
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), UpdateError> {
    fs::create_dir_all(to).map_err(|err| {
        crate::log_warn!("ディレクトリ作成失敗: {}: {err}", to.display());
        UpdateError::DirReplace
    })?;

    let entries = fs::read_dir(from).map_err(|err| {
        crate::log_warn!("ディレクトリ列挙失敗: {}: {err}", from.display());
        UpdateError::DirReplace
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| {
            crate::log_warn!("ディレクトリエントリ取得失敗: {err}");
            UpdateError::DirReplace
        })?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        if from_path.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path).map_err(|err| {
                crate::log_warn!(
                    "ファイルコピー失敗: {} -> {}: {err}",
                    from_path.display(),
                    to_path.display()
                );
                UpdateError::FileReplace
            })?;
        }
    }
    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::test_support::unique_name;
    use super::*;

    /// `data` を残したまま配布ファイルを上書きコピーできるようにすること
    #[test]
    fn copy_portable_payload_skips_data_and_overwrites_files() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-copy"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        let dest = root.join("dest");
        fs::create_dir_all(source.join("nested")).expect("source nested");
        fs::create_dir_all(source.join(PORTABLE_DATA_DIR_NAME)).expect("source data");
        fs::create_dir_all(dest.join(PORTABLE_DATA_DIR_NAME)).expect("dest data");
        fs::write(source.join("ClipRefiner.exe"), b"new-exe").expect("source exe");
        fs::write(source.join("PORTABLE.txt"), b"new").expect("source readme");
        fs::write(source.join("nested").join("a.txt"), b"nested-new").expect("nested");
        fs::write(
            source
                .join(PORTABLE_DATA_DIR_NAME)
                .join("should-not-copy.toml"),
            b"nope",
        )
        .expect("source data file");
        fs::write(dest.join("ClipRefiner.exe"), b"old-exe").expect("dest exe");
        fs::write(
            dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml"),
            b"keep-me",
        )
        .expect("dest config");

        copy_portable_payload(&source, &dest).expect("copy");

        assert_eq!(fs::read(dest.join("ClipRefiner.exe")).unwrap(), b"new-exe");
        assert_eq!(fs::read(dest.join("PORTABLE.txt")).unwrap(), b"new");
        assert_eq!(
            fs::read(dest.join("nested").join("a.txt")).unwrap(),
            b"nested-new"
        );
        assert_eq!(
            fs::read_to_string(dest.join(PORTABLE_DATA_DIR_NAME).join("config.toml")).unwrap(),
            "keep-me"
        );
        assert!(
            !dest
                .join(PORTABLE_DATA_DIR_NAME)
                .join("should-not-copy.toml")
                .exists()
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// 差し替え先が無いときはコピーしないようにすること
    #[test]
    fn copy_portable_payload_rejects_missing_dest() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-nodest"));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("source");
        fs::create_dir_all(&source).expect("source");
        fs::write(source.join("ClipRefiner.exe"), b"x").expect("exe");
        let dest = root.join("missing");

        let err = copy_portable_payload(&source, &dest).expect_err("dest missing");
        assert_eq!(err, UpdateError::DestPrepare);

        let _ = fs::remove_dir_all(&root);
    }

    /// 展開直下の exe を検出できるようにすること
    #[test]
    fn find_portable_exe_finds_case_insensitive_name() {
        let root = std::env::temp_dir().join(unique_name("clip-refiner-replace-find"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("cliprefiner.EXE"), b"x").expect("exe");

        let found = find_portable_exe(&root).expect("found");
        let name = found.file_name().and_then(|value| value.to_str()).unwrap();
        assert!(name.eq_ignore_ascii_case(&portable_exe_name()));

        let _ = fs::remove_dir_all(&root);
    }
}
