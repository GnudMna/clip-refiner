//! クラッシュ耐性のあるファイル置き換え
//!
//! 一時ファイルへ書き込んでからリネームで置き換え、途中書き込みの破損を防ぐ

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

// ======================================================================
// パブリック関数
// ======================================================================
/// バイト列を一時ファイル経由で原子的に書き込む
///
/// # Arguments
/// * `path` - 最終的な出力先
/// * `bytes` - 書き込む内容
///
/// # Returns
/// * `io::Result<()>` - 置き換えに成功した場合は `Ok(())`
///
/// # Errors
/// 一時ファイル作成・書き込み・リネームのいずれかに失敗した場合
pub(super) fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp_path = sibling_with_suffix(path, ".tmp")?;
    write_temp_file(&tmp_path, bytes)?;
    replace_file(&tmp_path, path)
}

/// テキストを一時ファイル経由で原子的に書き込む
///
/// # Arguments
/// * `path` - 最終的な出力先
/// * `content` - 書き込む文字列
///
/// # Returns
/// * `io::Result<()>` - 置き換えに成功した場合は `Ok(())`
///
/// # Errors
/// 一時ファイル作成・書き込み・リネームのいずれかに失敗した場合
pub(super) fn write_text_atomically(path: &Path, content: &str) -> io::Result<()> {
    write_bytes_atomically(path, content.as_bytes())
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 同じディレクトリに接尾辞付きパスを作る
fn sibling_with_suffix(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "対象パスにファイル名がありません",
        )
    })?;
    let mut tmp_name = OsString::from(file_name);
    tmp_name.push(suffix);
    Ok(path.with_file_name(tmp_name))
}

/// 一時ファイルへ内容を書き出し、ディスクへ同期する
fn write_temp_file(tmp_path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(tmp_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

/// 一時ファイルを最終パスへ置き換える
///
/// Windows では `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` で上書きし、
/// 旧 `.prev` 退避方式のような「最終パスが瞬間的に存在しない」隙間を作らない
fn replace_file(tmp: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        replace_file_windows(tmp, dest)
    }
    #[cfg(not(windows))]
    {
        rename_or_cleanup(tmp, dest)
    }
}

/// リネームし、失敗時は一時ファイルを削除する
#[cfg(not(windows))]
fn rename_or_cleanup(tmp: &Path, dest: &Path) -> io::Result<()> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(tmp);
            Err(err)
        }
    }
}

/// Windows 向けの上書きリネーム
#[cfg(windows)]
fn replace_file_windows(tmp: &Path, dest: &Path) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let tmp_wide = path_to_wide(tmp)?;
    let dest_wide = path_to_wide(dest)?;
    let result = unsafe {
        MoveFileExW(
            PCWSTR(tmp_wide.as_ptr()),
            PCWSTR(dest_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(tmp);
            Err(io::Error::other(format!(
                "MoveFileExW による置き換えに失敗: {err}"
            )))
        }
    }
}

/// `Path` を NUL 終端の UTF-16 へ変換する
#[cfg(windows)]
fn path_to_wide(path: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "パスに NUL 文字が含まれています",
        ));
    }
    wide.push(0);
    Ok(wide)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// 新規ファイルを原子的に作成し、一時ファイルを残さないようにすること
    #[test]
    fn write_bytes_atomically_creates_new_file() {
        let dir = unique_temp_dir("atomic-create");
        let path = dir.join("config.toml");

        write_text_atomically(&path, "mode = \"Trim\"\n").expect("原子的書き込み");

        assert_eq!(
            fs::read_to_string(&path).expect("読み取り"),
            "mode = \"Trim\"\n"
        );
        assert!(!dir.join("config.toml.tmp").exists());
        assert!(!dir.join("config.toml.prev").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 既存ファイルを置き換え、一時ファイルと `.prev` を残さないようにすること
    #[test]
    fn write_bytes_atomically_replaces_existing_file() {
        let dir = unique_temp_dir("atomic-replace");
        let path = dir.join("config.toml");
        fs::write(&path, "old\n").expect("初期書き込み");

        write_text_atomically(&path, "new\n").expect("原子的置き換え");

        assert_eq!(fs::read_to_string(&path).expect("読み取り"), "new\n");
        assert!(!dir.join("config.toml.tmp").exists());
        assert!(!dir.join("config.toml.prev").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 置き換え中も最終パスが読み取り可能となるようにすること
    ///
    /// 旧実装の `.prev` 退避では `rename(dest, .prev)` と `rename(tmp, dest)` の間に
    /// 最終パスが消える隙間があり、並行読取が `ENOENT` になることがあった
    #[test]
    fn write_bytes_atomically_keeps_destination_readable_during_replace() {
        let dir = unique_temp_dir("atomic-readable");
        let path = dir.join("config.toml");
        fs::write(&path, "old\n").expect("初期書き込み");

        for i in 0..200 {
            write_text_atomically(&path, &format!("new-{i}\n")).expect("原子的置き換え");
            let content = fs::read_to_string(&path).expect("置き換え直後も読める");
            assert!(
                content.starts_with("new-"),
                "置き換え後の内容が読めること: {content}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    /// テスト用の一意な一時ディレクトリを作る
    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("時計")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("clip-refiner-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("一時ディレクトリ");
        dir
    }
}
