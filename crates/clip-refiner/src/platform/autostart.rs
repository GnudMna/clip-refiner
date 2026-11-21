//! ログイン時の自動起動を OS ネイティブの仕組みで制御する

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

use crate::consts;

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
        run.set_value(run_value_name(), &format_run_value(&exe_path))
            .context("自動起動レジストリ値の書き込みに失敗")?;
    } else {
        let _ = run.delete_value(run_value_name());
    }

    Ok(())
}

// ======================================================================
// プライベート関数
// ======================================================================
/// Run キーの値名を返す
///
/// 開発版 (debug ビルド) は製品版の登録を上書き・削除しないよう別名にする
const fn run_value_name() -> &'static str {
    if cfg!(debug_assertions) {
        consts::DEV_DATA_DIR_NAME
    } else {
        consts::APP_NAME
    }
}

/// 現在の実行ファイルパスを正規化して返す
fn current_exe_path() -> Result<PathBuf> {
    std::env::current_exe()
        .context("実行ファイルパスの取得に失敗")
        .map(normalize_path)
}

/// パスを正規化する (`canonicalize` 失敗時は入力パスをそのまま返す)
///
/// Windows の `canonicalize` が付ける `\\?\` プレフィックスは除去する
fn normalize_path(path: PathBuf) -> PathBuf {
    strip_verbatim_prefix(path.canonicalize().unwrap_or(path))
}

/// ドライブ形式または UNC 形式のパスから `\\?\` プレフィックスを除去する
///
/// `\\?\Volume{...}` など通常形式に戻せないパスはそのまま返す
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();

    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }

    match text.strip_prefix(r"\\?\") {
        Some(rest) if has_drive_prefix(rest) => PathBuf::from(rest),
        _ => path,
    }
}

/// `C:\` や `C:` のようにドライブ文字で始まるかどうかを返す
fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Run キーに書き込む値を組み立てる (空白を含むパスに備えて引用符で囲む)
fn format_run_value(exe_path: &Path) -> String {
    format!("\"{}\"", exe_path.display())
}

/// Run キーの値から実行ファイルパスを取り出す
///
/// 引用符で囲まれている場合は閉じ引用符までを、囲まれていない場合は値全体をパスとみなす
fn parse_run_value(value: &str) -> PathBuf {
    let value = value.trim();

    match value.strip_prefix('"') {
        Some(rest) => PathBuf::from(rest.split('"').next().unwrap_or(rest)),
        None => PathBuf::from(value),
    }
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

    match run.get_value::<String, _>(run_value_name()) {
        Ok(value) => Ok(Some(normalize_path(parse_run_value(&value)))),
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

    /// ドライブ形式の `\\?\` プレフィックスを除去すること
    #[test]
    fn strip_verbatim_prefix_removes_drive_prefix() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\C:\Apps\ClipRefiner.exe")),
            PathBuf::from(r"C:\Apps\ClipRefiner.exe")
        );
    }

    /// UNC 形式の `\\?\UNC\` プレフィックスを `\\` に戻すこと
    #[test]
    fn strip_verbatim_prefix_restores_unc_path() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\ClipRefiner.exe")),
            PathBuf::from(r"\\server\share\ClipRefiner.exe")
        );
    }

    /// 通常形式に戻せないパスと通常のパスは変更しないこと
    #[test]
    fn strip_verbatim_prefix_keeps_other_paths() {
        for raw in [
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\ClipRefiner.exe",
            r"C:\Apps\ClipRefiner.exe",
        ] {
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from(raw)),
                PathBuf::from(raw)
            );
        }
    }

    /// 空白を含むパスを引用符で囲んで書き込むこと
    #[test]
    fn format_run_value_quotes_path_with_spaces() {
        assert_eq!(
            format_run_value(Path::new(r"C:\Program Files\ClipRefiner\ClipRefiner.exe")),
            r#""C:\Program Files\ClipRefiner\ClipRefiner.exe""#
        );
    }

    /// 引用符付きの値と引用符なしの値のどちらからもパスを取り出せること
    #[test]
    fn parse_run_value_handles_quoted_and_unquoted() {
        // 引用符付きは閉じ引用符以降の引数を無視
        assert_eq!(
            parse_run_value(r#""C:\Program Files\ClipRefiner.exe" --minimized"#),
            PathBuf::from(r"C:\Program Files\ClipRefiner.exe")
        );
        // MSI が書く引用符なしの値は全体をパスとして扱う
        assert_eq!(
            parse_run_value(r"C:\Program Files\ClipRefiner.exe"),
            PathBuf::from(r"C:\Program Files\ClipRefiner.exe")
        );
    }

    /// 書き込んだ値を読み取ると元のパスに戻ること
    #[test]
    fn run_value_round_trips() {
        let path = Path::new(r"C:\Program Files\ClipRefiner\ClipRefiner.exe");
        assert_eq!(parse_run_value(&format_run_value(path)), path);
    }
}
