//! クラッシュ耐性のあるファイル置き換え
//!
//! 一時ファイルへ書き込んでからリネームで置き換え、途中書き込みの破損を防ぐ

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

// ======================================================================
// 定数
// ======================================================================
/// 一時ファイル名の連番 (同一プロセス内の並行書き込みで名前を衝突させない)
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);
/// プロセス内の置き換えを直列化するロック
///
/// Windows では同じ出力先への `MoveFileExW` が同時に走ると `ERROR_ACCESS_DENIED` で失敗するため
static REPLACE_LOCK: Mutex<()> = Mutex::new(());

// ======================================================================
// パブリック関数
// ======================================================================
/// バイト列を一時ファイル経由で原子的に書き込む
///
/// 一時ファイル名は呼び出しごとに一意で、同一プロセス内の並行書き込みは直列に処理する。
/// 最終的な内容は、最後に完了した呼び出しのものになる
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
    // 保持するのは `()` のみで、poison されても状態は壊れない
    let _guard = REPLACE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path = sibling_with_suffix(path, &format!(".{}.{seq}.tmp", std::process::id()))?;
    if let Err(err) = write_temp_file(&tmp_path, bytes) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
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
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp_path)?;
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
    // SAFETY: `tmp_wide` / `dest_wide` は NUL 終端の UTF-16 で、呼び出し中は生存する
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
        assert_no_leftovers(&dir);

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
        assert_no_leftovers(&dir);

        let _ = fs::remove_dir_all(&dir);
    }

    /// 同じ出力先への並行書き込みで一時ファイル名が衝突しないこと
    ///
    /// 最終的な内容は、いずれかのスレッドが書いた内容が欠けずに残っていればよい
    #[test]
    fn write_bytes_atomically_uses_distinct_temp_paths_for_concurrent_writes() {
        let dir = unique_temp_dir("atomic-concurrent");
        let path = dir.join("config.toml");

        let tmp_a = sibling_with_suffix(&path, ".a.tmp").expect("パス生成");
        assert_ne!(
            tmp_a,
            sibling_with_suffix(&path, ".b.tmp").expect("パス生成")
        );

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for j in 0..25 {
                        let body = format!("writer-{i}-{j}\n").repeat(512);
                        write_text_atomically(&path, &body).expect("並行の原子的置き換え");
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("スレッドの終了");
        }

        let content = fs::read_to_string(&path).expect("読み取り");
        let first = content.lines().next().expect("内容が空");
        assert_eq!(content, format!("{first}\n").repeat(512), "内容が混ざった");
        assert_no_leftovers(&dir);

        let _ = fs::remove_dir_all(&dir);
    }

    /// 書き込み失敗時に一時ファイルを残さないこと
    #[test]
    fn write_bytes_atomically_cleans_up_temp_file_on_failure() {
        let dir = unique_temp_dir("atomic-failure");
        // 出力先がディレクトリのため、置き換えに失敗する
        let path = dir.join("config.toml");
        fs::create_dir_all(&path).expect("ディレクトリ作成");

        assert!(write_text_atomically(&path, "new\n").is_err());
        assert_no_leftovers(&dir);

        let _ = fs::remove_dir_all(&dir);
    }

    /// 一時ファイルと `.prev` が残っていないことを確認する
    fn assert_no_leftovers(dir: &Path) {
        for entry in fs::read_dir(dir).expect("ディレクトリ読み取り") {
            let name = entry.expect("エントリ").file_name();
            let name = name.to_string_lossy();
            assert!(
                !name.ends_with(".tmp") && !name.ends_with(".prev"),
                "一時ファイルが残っている: {name}"
            );
        }
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
