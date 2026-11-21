//! ポータブル更新の差し替えヘルパー
//!
//! 展開済みの新しい `ClipRefiner.exe` を `--apply-portable-update` で起動し、
//! 旧プロセス終了後に実行フォルダへファイルをコピーして再起動する
//! CLI の `--install-dir` / `--cleanup-dir` / `--wait-pid` は検証してから使う

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::UPDATE_WORK_DIR_NAME;
use super::error::UpdateError;
use crate::consts::{APP_NAME, PORTABLE_DATA_DIR_NAME, PORTABLE_MARKER_FILE};

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
/// 旧プロセス終了待ちの上限
const WAIT_PID_TIMEOUT: Duration = Duration::from_mins(2);
/// 一時フォルダ削除を遅延させる秒数 (`ping` の回数。約 N-1 秒)
const CLEANUP_PING_COUNT: u32 = 3;
/// コンソールウィンドウを出さないプロセス作成フラグ
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// プロセス画像パス取得バッファ (UTF-16 単位)
const PROCESS_IMAGE_NAME_CHARS: usize = 32_768;

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
    match run_portable_replace_inner(wait_pid, install_dir, cleanup_dir) {
        Ok(()) => Ok(()),
        Err(err) => {
            show_replace_error(&err);
            Err(err)
        }
    }
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
    validate_replace_request(source_dir, install_dir, Some(cleanup_dir))?;
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
/// 差し替え先ディレクトリは既存である必要がある (新規作成しない)
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

// ======================================================================
// 差し替え実行
// ======================================================================
/// 検証・待機・コピー・再起動・掃除予約を順に行う
fn run_portable_replace_inner(
    wait_pid: u32,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    let source_dir = current_exe_dir()?;
    validate_replace_request(&source_dir, install_dir, cleanup_dir)?;
    wait_for_pid_exit(wait_pid, WAIT_PID_TIMEOUT)?;
    copy_portable_payload(&source_dir, install_dir)?;

    let new_exe = install_dir.join(portable_exe_name());
    if let Err(err) = Command::new(&new_exe).current_dir(install_dir).spawn() {
        let update_err = UpdateError::RestartFailed {
            detail: err.to_string(),
        };
        crate::log_warn!("{update_err}");
        return Err(update_err);
    }

    if let Some(dir) = cleanup_dir {
        schedule_dir_delete(dir);
    }

    Ok(())
}

// ======================================================================
// 引数検証
// ======================================================================
/// 差し替え元・先・削除対象がヘルパーとして安全か検証する
///
/// 展開元は `%TEMP%\clip-refiner-update\` 配下、差し替え先は既存のポータブル
/// フォルダ (`ClipRefiner.exe` と `portable` マーカー) に限る
/// 削除対象がある場合は同じ更新作業フォルダ配下かつ展開元の祖先である必要がある
fn validate_replace_request(
    source_dir: &Path,
    install_dir: &Path,
    cleanup_dir: Option<&Path>,
) -> Result<(), UpdateError> {
    let work_root = canonicalize_dir(&update_work_root()).map_err(|_| {
        crate::log_warn!("更新作業フォルダが存在しない");
        UpdateError::SourceDirInvalid
    })?;
    let source = canonicalize_dir(source_dir).map_err(|_| {
        crate::log_warn!("展開元を解決できない: {}", source_dir.display());
        UpdateError::SourceDirInvalid
    })?;
    let install = canonicalize_dir(install_dir).map_err(|_| {
        crate::log_warn!("差し替え先を解決できない: {}", install_dir.display());
        UpdateError::InstallDirInvalid
    })?;

    if source == install {
        return Err(UpdateError::SameReplaceDirs);
    }
    if !is_strict_descendant(&source, &work_root) {
        crate::log_warn!("展開元が更新作業フォルダの外: {}", source.display());
        return Err(UpdateError::SourceDirInvalid);
    }
    if is_path_under(&install, &work_root) {
        crate::log_warn!("差し替え先が更新作業フォルダ配下: {}", install.display());
        return Err(UpdateError::InstallDirInvalid);
    }
    if find_portable_exe(&source).is_none() {
        crate::log_warn!("展開元に ClipRefiner.exe が無い: {}", source.display());
        return Err(UpdateError::HelperExeMissing);
    }
    if !looks_like_portable_install(&install) {
        crate::log_warn!(
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
        crate::log_warn!("削除対象を解決できない: {}", cleanup_dir.display());
        UpdateError::CleanupDirInvalid
    })?;
    if !is_strict_descendant(&cleanup, work_root) {
        crate::log_warn!("削除対象が更新作業フォルダの外: {}", cleanup.display());
        return Err(UpdateError::CleanupDirInvalid);
    }
    if !is_path_under(source, &cleanup) {
        crate::log_warn!(
            "削除対象が展開元の祖先ではない: cleanup={} source={}",
            cleanup.display(),
            source.display()
        );
        return Err(UpdateError::CleanupDirInvalid);
    }
    Ok(())
}

/// `%TEMP%\clip-refiner-update` を返す
fn update_work_root() -> PathBuf {
    std::env::temp_dir().join(UPDATE_WORK_DIR_NAME)
}

/// 既存ディレクトリを正規化する
fn canonicalize_dir(path: &Path) -> Result<PathBuf, UpdateError> {
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
fn is_strict_descendant(path: &Path, ancestor: &Path) -> bool {
    is_path_under(path, ancestor) && path != ancestor
}

/// 差し替え先が既存のポータブルインストールか
fn looks_like_portable_install(install_dir: &Path) -> bool {
    find_portable_exe(install_dir).is_some() && install_dir.join(PORTABLE_MARKER_FILE).is_file()
}

// ======================================================================
// パス / プロセス
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

/// パスをコマンドライン引数用の文字列にする
fn path_to_string(path: &Path) -> Result<String, UpdateError> {
    path.to_str()
        .map(str::to_string)
        .ok_or(UpdateError::PathInvalid)
}

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

/// 指定 PID の終了を待つ
///
/// `0` とヘルパー自身は拒否する。生存中なら画像名が `ClipRefiner.exe` であることを要求する
/// 既に終了している PID は旧プロセスが先に落ちた場合として許可する
fn wait_for_pid_exit(pid: u32, timeout: Duration) -> Result<(), UpdateError> {
    if pid == 0 || pid == std::process::id() {
        crate::log_warn!("待ち受け PID が不正: {pid}");
        return Err(UpdateError::WaitPidInvalid);
    }

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
        let _ = unsafe { CloseHandle(handle) };
        crate::log_warn!("待ち受けプロセスが ClipRefiner ではない: pid={pid}");
        return Err(UpdateError::WaitPidNotClipRefiner);
    }

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

/// プロセス画像パスのファイル名が `ClipRefiner.exe` か
fn process_image_is_cliprefiner(handle: HANDLE) -> bool {
    let mut buf = vec![0u16; PROCESS_IMAGE_NAME_CHARS];
    let mut size = u32::try_from(buf.len()).unwrap_or(u32::MAX);
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

    /// テスト用の一意な一時ディレクトリ名を返す
    fn unique_name(prefix: &str) -> String {
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
    fn prepare_valid_replace_layout() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
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
    fn cleanup_layout(work_dir: &Path, install: &Path) {
        let _ = fs::remove_dir_all(work_dir);
        let _ = fs::remove_dir_all(install);
    }

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
