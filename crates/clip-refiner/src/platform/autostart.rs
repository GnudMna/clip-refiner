//! ログイン時の自動起動を OS ネイティブの仕組みで制御する

use std::path::{Path, PathBuf};

use crate::consts;

use anyhow::{Context, Result};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

// ======================================================================
// パブリック関数
// ======================================================================
/// ログイン時の自動起動が有効かどうかを返す
///
/// # Returns
/// * `bool` - 現在の実行ファイルが Run キーに登録されている場合は `true`
pub fn is_enabled() -> bool {
    let Ok(exe_path) = current_exe_path() else {
        return false;
    };

    match read_run_value() {
        Ok(Some(value)) => paths_match(&value, &exe_path),
        _ => false,
    }
}

/// ログイン時の自動起動を有効または無効にする
///
/// # Arguments
/// * `enabled` - 有効にする場合は `true`、無効にする場合は `false`
///
/// # Returns
/// * `Result<()>` - 設定変更に成功した場合は `Ok(())`、失敗した場合は `Err` を返す
pub fn set_enabled(enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ)
        .context("自動起動レジストリキーのオープンに失敗")?;

    if enabled {
        let exe_path = current_exe_path()?;
        let path_str = exe_path.to_string_lossy().into_owned();
        run.set_value(consts::APP_NAME, &path_str)
            .context("自動起動レジストリ値の書き込みに失敗")?;
    } else {
        let _ = run.delete_value(consts::APP_NAME);
    }

    Ok(())
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 現在の実行ファイルパスを正規化して返す
fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe()
        .context("実行ファイルパスの取得に失敗")
        .map(normalize_path)
}

/// パスを正規化する (`canonicalize` 失敗時は入力パスをそのまま返す)
fn normalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// 2 つのパスが同一の実行ファイルを指すかどうかを判定する
pub(crate) fn paths_match(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Run キーに登録されている実行ファイルパスを読み取る
fn read_run_value() -> Result<Option<PathBuf>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .context("自動起動レジストリキーの読み取りに失敗")?;

    match run.get_value::<String, _>(consts::APP_NAME) {
        Ok(value) => Ok(Some(normalize_path(PathBuf::from(value)))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("自動起動レジストリ値の読み取りに失敗"),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// パス比較が大文字小文字を区別しないようにすること
    #[test]
    fn paths_match_ignore_case_on_windows() {
        assert!(paths_match(
            Path::new(r"C:\Apps\ClipRefiner.exe"),
            Path::new(r"c:\apps\cliprefiner.exe")
        ));
    }

    /// 同一パスは一致すること
    #[test]
    fn paths_match_same_path() {
        let path = Path::new(r"C:\Apps\ClipRefiner.exe");
        assert!(paths_match(path, path));
    }

    /// 異なるパスは一致しないようにすること
    #[test]
    fn paths_match_different_paths() {
        assert!(!paths_match(
            Path::new(r"C:\Apps\ClipRefiner.exe"),
            Path::new(r"D:\Apps\ClipRefiner.exe")
        ));
    }
}
