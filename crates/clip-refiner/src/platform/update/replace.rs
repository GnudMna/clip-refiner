//! ポータブル更新の差し替えヘルパー
//!
//! 展開済みの新しい `ClipRefiner.exe` を `--apply-portable-update` で起動し、
//! 旧プロセス終了後に実行フォルダへファイルをコピーして再起動する

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::error::UpdateError;

use crate::consts::{APP_NAME, PORTABLE_DATA_DIR_NAME};

use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::core::PCWSTR;

// ======================================================================
// 定数
// ======================================================================
/// 旧プロセス終了待ちの上限
const WAIT_PID_TIMEOUT: Duration = Duration::from_mins(2);
/// 一時フォルダ削除を遅延させる秒数 (`ping` の回数。約 N-1 秒)
const CLEANUP_PING_COUNT: u32 = 3;
/// コンソールウィンドウを出さないプロセス作成フラグ
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ======================================================================
// パブリック関数
// ======================================================================
/// ポータブル差し替えヘルパーを実行する (CLI エントリ)
///
/// 旧 PID の終了を待ち、自身のディレクトリからインストール先へファイルをコピーし、
/// 新しい常駐プロセスを起動する。`data` ディレクトリは上書きしない
///
/// # Arguments
/// * `wait_pid` - 終了を待つ旧プロセスの PID
/// * `install_dir` - 差し替え先 (現在の実行フォルダ)
/// * `cleanup_dir` - 終了後に削除する作業ディレクトリ (任意)
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(crate) fn run_portable_replace(
    wait_pid: u32,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    let source_dir = current_exe_dir()?;
    validate_replace_dirs(&source_dir, install_dir)?;

    if let Err(err) = wait_for_pid_exit(wait_pid, WAIT_PID_TIMEOUT) {
        show_replace_error(&err);
        return Err(err);
    }

    if let Err(err) = copy_portable_payload(&source_dir, install_dir) {
        show_replace_error(&err);
        return Err(err);
    }

    let new_exe = install_dir.join(portable_exe_name());
    if let Err(err) = Command::new(&new_exe).current_dir(install_dir).spawn() {
        let update_err = UpdateError::RestartFailed {
            detail: err.to_string(),
        };
        crate::log_warn!("{update_err}");
        show_replace_error(&update_err);
        return Err(update_err);
    }

    if let Some(dir) = cleanup_dir {
        schedule_dir_delete(dir);
    }

    Ok(())
}

/// 展開済みポータブルの差し替えヘルパーをバックグラウンド起動する
///
/// # Arguments
/// * `source_dir` - 展開ルート (新しい `ClipRefiner.exe` があるディレクトリ)
/// * `install_dir` - 差し替え先
/// * `cleanup_dir` - ヘルパー終了後に削除する作業ディレクトリ
///
/// # Returns
/// * `Result<(), UpdateError>` - 起動成功時 `Ok(())`
pub(crate) fn launch_portable_replace_helper(
    source_dir: &Path,
    install_dir: &Path,
    cleanup_dir: &Path,
) -> Result<(), UpdateError> {
    validate_replace_dirs(source_dir, install_dir)?;
    let helper_exe = find_portable_exe(source_dir).ok_or(UpdateError::HelperExeMissing)?;

    let wait_pid = std::process::id().to_string();
    let install_dir_str = path_to_string(install_dir)?;
    let cleanup_dir_str = path_to_string(cleanup_dir)?;

    let mut command = Command::new(&helper_exe);
    command
        .args([
            "--apply-portable-update",
            "--wait-pid",
            &wait_pid,
            "--install-dir",
            &install_dir_str,
            "--cleanup-dir",
            &cleanup_dir_str,
        ])
        .current_dir(source_dir)
        .creation_flags(CREATE_NO_WINDOW);

    match command.spawn() {
        Ok(_) => Ok(()),
        Err(err) => {
            crate::log_warn!("ポータブル差し替えヘルパーの起動に失敗: {err}");
            Err(UpdateError::HelperLaunch)
        }
    }
}

/// 現在の実行ファイルがあるディレクトリを返す
///
/// # Returns
/// * `Result<PathBuf, UpdateError>` - 実行ディレクトリ
pub(crate) fn current_install_dir() -> Result<PathBuf, UpdateError> {
    current_exe_dir()
}

/// ポータブル更新の配布物をインストール先へコピーする
///
/// 送信元の `data` はスキップする。宛先に既にある `data` は保持する
///
/// # Arguments
/// * `source_dir` - 展開ルート
/// * `dest_dir` - 差し替え先
///
/// # Returns
/// * `Result<(), UpdateError>` - 成功時 `Ok(())`
pub(crate) fn copy_portable_payload(source_dir: &Path, dest_dir: &Path) -> Result<(), UpdateError> {
    if !source_dir.is_dir() {
        return Err(UpdateError::SourceMissing);
    }
    fs::create_dir_all(dest_dir).map_err(|err| {
        crate::log_warn!("差し替え先ディレクトリの作成に失敗: {err}");
        UpdateError::DestPrepare
    })?;

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

// ======================================================================
// プライベート関数
// ======================================================================
/// 実行ファイル名 (`ClipRefiner.exe`)
fn portable_exe_name() -> String {
    format!("{APP_NAME}.exe")
}

/// 自身の実行ディレクトリを返す
fn current_exe_dir() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|err| {
        crate::log_warn!("実行ファイルパスの取得に失敗: {err}");
        UpdateError::ExePath
    })?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or(UpdateError::ExeDir)
}

/// 展開ルート直下の `ClipRefiner.exe` を探す
fn find_portable_exe(source_dir: &Path) -> Option<PathBuf> {
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

/// 差し替え元と先が安全か検証する
fn validate_replace_dirs(source_dir: &Path, install_dir: &Path) -> Result<(), UpdateError> {
    let source = fs::canonicalize(source_dir).unwrap_or_else(|_| source_dir.to_path_buf());
    let install = fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());
    if source == install {
        return Err(UpdateError::SameReplaceDirs);
    }
    Ok(())
}

/// パスをコマンドライン引数用の文字列にする
fn path_to_string(path: &Path) -> Result<String, UpdateError> {
    path.to_str()
        .map(str::to_string)
        .ok_or(UpdateError::PathInvalid)
}

/// 指定 PID の終了を待つ
fn wait_for_pid_exit(pid: u32, timeout: Duration) -> Result<(), UpdateError> {
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
        return Ok(());
    };

    let timeout_ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    let wait = unsafe { WaitForSingleObject(handle, timeout_ms) };
    let _ = unsafe { CloseHandle(handle) };

    if wait == WAIT_OBJECT_0 {
        Ok(())
    } else if wait == WAIT_TIMEOUT {
        Err(UpdateError::WaitPidTimeout)
    } else {
        Err(UpdateError::WaitPidFailed)
    }
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

/// プロセス終了後にディレクトリ削除を予約する
fn schedule_dir_delete(dir: &Path) {
    let Ok(dir_str) = path_to_string(dir) else {
        return;
    };
    let script = format!("ping 127.0.0.1 -n {CLEANUP_PING_COUNT} >nul & rmdir /s /q \"{dir_str}\"");
    let mut command = Command::new("cmd.exe");
    command
        .args(["/C", &script])
        .creation_flags(CREATE_NO_WINDOW);
    if let Err(err) = command.spawn() {
        crate::log_warn!("更新一時フォルダの削除予約に失敗: {err}");
    }
}

/// 差し替え失敗をユーザーへ通知する
fn show_replace_error(err: &UpdateError) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

    let title = wide_null(&format!("{APP_NAME} の更新"));
    let body = wide_null(&err.to_string());
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// UTF-16 null 終端へ変換する
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// `data` を残したまま配布ファイルを上書きコピーできること
    #[test]
    fn copy_portable_payload_skips_data_and_overwrites_files() {
        let root =
            std::env::temp_dir().join(format!("clip-refiner-replace-copy-{}", std::process::id()));
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

    /// 展開直下の exe を検出できること
    #[test]
    fn find_portable_exe_finds_case_insensitive_name() {
        let root =
            std::env::temp_dir().join(format!("clip-refiner-replace-find-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("cliprefiner.EXE"), b"x").expect("exe");

        let found = find_portable_exe(&root).expect("found");
        let name = found.file_name().and_then(|value| value.to_str()).unwrap();
        assert!(name.eq_ignore_ascii_case(&portable_exe_name()));

        let _ = fs::remove_dir_all(&root);
    }

    /// 同一ディレクトリへの差し替えを拒否すること
    #[test]
    fn validate_replace_dirs_rejects_same_path() {
        let root =
            std::env::temp_dir().join(format!("clip-refiner-replace-same-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        assert!(validate_replace_dirs(&root, &root).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
