//! ファズターゲットの共通処理
//!
//! Windows では Rust のパニック (`abort`) を libFuzzer が捕捉できず、失敗した入力が
//! `artifacts/` に保存されない。パニック時に実行中の入力を自前で保存して再現できるようにする

use std::cell::RefCell;
use std::sync::Once;

// ======================================================================
// パブリック関数
// ======================================================================
/// UTF-8 として解釈できる入力だけを検証処理へ渡す
///
/// 検証処理がパニックした場合は、入力を `-artifact_prefix` の場所へ `crash-<ハッシュ>` として保存する
///
/// # Arguments
/// * `data` - libFuzzer が生成した入力
/// * `check` - 文字列に対する検証処理
pub fn fuzz_text(data: &[u8], check: impl FnOnce(&str)) {
    install_crash_writer();
    CURRENT_INPUT.with_borrow_mut(|input| {
        input.clear();
        input.extend_from_slice(data);
    });
    if let Ok(text) = std::str::from_utf8(data) {
        check(text);
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
thread_local! {
    /// 実行中の入力 (パニック時に保存する)
    static CURRENT_INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// パニック時に実行中の入力を保存するフックを 1 回だけ登録する
///
/// 既存のフック (libfuzzer-sys のメッセージ出力と `abort`) は保存後に呼ぶ
fn install_crash_writer() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let Some(prefix) = std::env::args()
            .find_map(|arg| arg.strip_prefix("-artifact_prefix=").map(str::to_string))
        else {
            return;
        };
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            CURRENT_INPUT.with_borrow(|input| {
                let path = format!("{prefix}crash-{:016x}", fnv1a(input));
                match std::fs::write(&path, input) {
                    Ok(()) => eprintln!("失敗した入力を保存した: {path}"),
                    Err(e) => eprintln!("失敗した入力を保存できない ({path}): {e}"),
                }
            });
            previous(info);
        }));
    });
}

/// 保存ファイル名に使う 64 bit FNV-1a ハッシュ
fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
