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
/// `ShellExecuteW` の戻り値の成否を分けるしきい値 (これを超えれば成功、これ以下はエラーコード)
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
    shell_execute(target, None)
}

/// 指定したプログラムでファイルを開く
///
/// 拡張子の関連付けが無いファイルを特定のアプリで開くときに使う
/// ファイルのパスは引用符で囲んで 1 つの引数として渡す
///
/// # Arguments
/// * `program` - 起動するプログラムのフルパス
/// * `file` - プログラムに渡すファイルのパス
///
/// # Returns
/// * `io::Result<()>` - 起動を受け付けた場合は `Ok(())`
pub(crate) fn shell_open_with(program: &str, file: &str) -> io::Result<()> {
    shell_execute(program, Some(&format!("\"{file}\"")))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// `ShellExecuteW` の `open` 動詞を呼び出す
fn shell_execute(file: &str, parameters: Option<&str>) -> io::Result<()> {
    let file = HSTRING::from(file);
    let parameters = parameters.map(HSTRING::from);
    let parameters = parameters
        .as_ref()
        .map_or_else(PCWSTR::null, |parameters| PCWSTR(parameters.as_ptr()));
    // SAFETY: `file` と `parameters` は NUL 終端の UTF-16 (または NULL) で呼び出し中は生存し、そのほかの引数は NULL か定数
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &file,
            parameters,
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
