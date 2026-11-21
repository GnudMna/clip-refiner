//! シェル経由での URL・フォルダの起動
//!
//! `cmd /C start` や名前だけの `explorer` 起動は、`&` を含む引数の再解釈や
//! 検索パス上の偽物を拾う余地がある。`ShellExecuteW` に直接渡して避ける

use std::io;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

// ======================================================================
// 定数
// ======================================================================
/// `ShellExecuteW` が成功時に返す値の下限 (これ以下はエラーコード)
const SHELL_EXECUTE_SUCCESS_MIN: isize = 32;

// ======================================================================
// パブリック関数
// ======================================================================
/// URL またはパスを既定の関連付けで開く
///
/// コマンドラインを経由しないため、`&` や `%` を含む対象もそのまま渡る
///
/// # Arguments
/// * `target` - 開く URL またはファイル・フォルダのパス
///
/// # Returns
/// * `io::Result<()>` - 起動を受け付けた場合は `Ok(())`
pub(crate) fn shell_open(target: &str) -> io::Result<()> {
    let file = HSTRING::from(target);
    // SAFETY: `file` は NUL 終端の UTF-16 で呼び出し中は生存し、そのほかの引数は NULL か定数
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &file,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // 戻り値は `HINSTANCE` 型だが、32 以下ならエラーコード
    let code = result.0 as isize;
    if code > SHELL_EXECUTE_SUCCESS_MIN {
        return Ok(());
    }
    Err(io::Error::from_raw_os_error(
        i32::try_from(code).unwrap_or(0),
    ))
}
