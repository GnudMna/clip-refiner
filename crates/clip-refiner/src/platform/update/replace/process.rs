//! 差し替えヘルパーのプロセス操作
//!
//! 旧プロセスの終了待ち (画像名の検証を含む)、作業フォルダ削除の遅延予約、
//! 失敗時のメッセージボックス通知を担う

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::super::error::UpdateError;
use super::CREATE_NO_WINDOW;
use super::files::{path_to_string, portable_exe_name};
use super::validate::canonicalize_dir;
use crate::consts::APP_NAME;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    QueryFullProcessImageNameW, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

// ======================================================================
// 定数
// ======================================================================
/// 一時フォルダ削除を遅延させる秒数 (`ping` の回数。約 N-1 秒)
const CLEANUP_PING_COUNT: u32 = 3;
/// プロセス画像パス取得バッファ (UTF-16 単位)
const PROCESS_IMAGE_NAME_CHARS: usize = 32_768;

// ======================================================================
// 旧プロセス待機
// ======================================================================
/// 指定 PID の終了を待つ
///
/// `0` とヘルパー自身は拒否する。生存中なら画像名が `ClipRefiner.exe` であることを要求する
/// 既に終了している PID は旧プロセスが先に落ちた場合として許可する
pub(super) fn wait_for_pid_exit(pid: u32, timeout: Duration) -> Result<(), UpdateError> {
    if pid == 0 || pid == std::process::id() {
        crate::log_warn!("待ち受け PID が不正: {pid}");
        return Err(UpdateError::WaitPidInvalid);
    }

    // SAFETY: ポインタを受け取らず、成功時に返るハンドルはこの関数内で必ず閉じる
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
    };
    let handle = match handle {
        Ok(handle) => handle,
        Err(err) if err.code() == ERROR_INVALID_PARAMETER.into() => {
            crate::log_info!("差し替え待ちの PID は既に終了している: {pid}");
            return Ok(());
        }
        Err(err) => {
            crate::log_warn!("旧プロセスを開けない: {err}");
            return Err(UpdateError::WaitPidFailed);
        }
    };

    let image_ok = process_image_is_cliprefiner(handle);
    if !image_ok {
        // SAFETY: `OpenProcess` で取得した未解放のハンドルを 1 度だけ閉じる
        let _ = unsafe { CloseHandle(handle) };
        crate::log_warn!("待ち受けプロセスが ClipRefiner ではない: pid={pid}");
        return Err(UpdateError::WaitPidNotClipRefiner);
    }

    let timeout_ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `handle` は `PROCESS_SYNCHRONIZE` 付きで開いた生存中のハンドル
    let wait = unsafe { WaitForSingleObject(handle, timeout_ms) };
    // SAFETY: `OpenProcess` で取得した未解放のハンドルを 1 度だけ閉じる
    let _ = unsafe { CloseHandle(handle) };

    if wait == WAIT_OBJECT_0 {
        Ok(())
    } else if wait == WAIT_TIMEOUT {
        Err(UpdateError::WaitPidTimeout)
    } else {
        Err(UpdateError::WaitPidFailed)
    }
}

/// プロセス画像パスのファイル名が `ClipRefiner.exe` か
fn process_image_is_cliprefiner(handle: HANDLE) -> bool {
    let mut buf = vec![0u16; PROCESS_IMAGE_NAME_CHARS];
    let mut size = u32::try_from(buf.len()).unwrap_or(u32::MAX);
    // SAFETY: `buf` は `size` 文字分の書き込み可能な領域で、`size` は呼び出し中に生存する
    // 書き込まれた文字数は `size` に返り、読み取りは `buf.len()` で上限を切る
    let query = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut size,
        )
    };
    if query.is_err() {
        return false;
    }
    let len = usize::try_from(size).unwrap_or(0).min(buf.len());
    let image = String::from_utf16_lossy(&buf[..len]);
    is_cliprefiner_image_name(&image)
}

/// 画像パスのファイル名が `ClipRefiner.exe` か (大文字小文字無視)
fn is_cliprefiner_image_name(image_path: &str) -> bool {
    Path::new(image_path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(&portable_exe_name()))
}

// ======================================================================
// 後片付け / 通知
// ======================================================================
/// `cmd.exe` の `rmdir` に渡せるよう拡張プレフィックスを外し、危険文字を拒否する
fn path_for_cmd_rmdir(path: &Path) -> Result<String, UpdateError> {
    let canonical = canonicalize_dir(path)?;
    let raw = path_to_string(&canonical)?;
    let stripped = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| raw.strip_prefix(r"\\?\").map(str::to_string))
        .unwrap_or(raw);
    if stripped
        .chars()
        .any(|ch| matches!(ch, '"' | '&' | '|' | '%' | '<' | '>' | '\r' | '\n'))
    {
        crate::log_warn!("削除対象パスに不正な文字がある");
        return Err(UpdateError::PathInvalid);
    }
    Ok(stripped)
}

/// プロセス終了後にディレクトリ削除を予約する
pub(super) fn schedule_dir_delete(dir: &Path) {
    let Ok(dir_str) = path_for_cmd_rmdir(dir) else {
        crate::log_warn!(
            "更新一時フォルダの削除をスキップ (パス不正): {}",
            dir.display()
        );
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
pub(super) fn show_replace_error(err: &UpdateError) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

    let title = wide_null(&format!("{APP_NAME} の更新"));
    let body = wide_null(&err.to_string());
    // SAFETY: `title` / `body` は NUL 終端の UTF-16 で、呼び出し中は生存する
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
    use super::super::test_support::{cleanup_layout, prepare_valid_replace_layout};
    use super::*;

    /// 画像パスのファイル名で `ClipRefiner` を判定できるようにすること
    #[test]
    fn is_cliprefiner_image_name_matches_exe() {
        assert!(is_cliprefiner_image_name(r"C:\Apps\ClipRefiner.exe"));
        assert!(is_cliprefiner_image_name(r"C:\Apps\cliprefiner.EXE"));
        assert!(!is_cliprefiner_image_name(r"C:\Windows\System32\cmd.exe"));
        assert!(!is_cliprefiner_image_name(""));
    }

    /// PID `0` と自身の PID を拒否すること
    #[test]
    fn wait_for_pid_exit_rejects_zero_and_self() {
        assert_eq!(
            wait_for_pid_exit(0, Duration::from_millis(10)),
            Err(UpdateError::WaitPidInvalid)
        );
        assert_eq!(
            wait_for_pid_exit(std::process::id(), Duration::from_millis(10)),
            Err(UpdateError::WaitPidInvalid)
        );
    }

    /// 生存中の非 `ClipRefiner` プロセスを待ち受け対象にしないようにすること
    #[test]
    fn wait_for_pid_exit_rejects_non_cliprefiner_process() {
        let mut child = Command::new("cmd.exe")
            .args(["/C", "ping", "127.0.0.1", "-n", "8", ">nul"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .expect("spawn cmd");
        let pid = child.id();
        let result = wait_for_pid_exit(pid, Duration::from_secs(1));
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(result, Err(UpdateError::WaitPidNotClipRefiner));
    }

    /// 存在しない PID は旧プロセス終了済みとして許可すること
    #[test]
    fn wait_for_pid_exit_accepts_missing_pid() {
        wait_for_pid_exit(0xFFFF_FFFC, Duration::from_secs(1)).expect("missing pid");
    }

    /// `rmdir` 用パスから拡張プレフィックスを外せるようにすること
    #[test]
    fn path_for_cmd_rmdir_strips_extended_prefix() {
        let (_work_root, work_dir, _source, install) = prepare_valid_replace_layout();
        let cmd_path = path_for_cmd_rmdir(&work_dir).expect("cmd path");
        assert!(!cmd_path.starts_with(r"\\?\"), "{cmd_path}");
        assert!(!cmd_path.contains('"'));
        cleanup_layout(&work_dir, &install);
    }
}
