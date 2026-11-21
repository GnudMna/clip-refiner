//! クリップボードのテキストを加工するツール `ClipRefiner` のライブラリクレート
//!
//! 加工モード (`refiner`)、設定 (`config`)、常駐 UI (`tray`) を提供する

// アプリケーション本体を lib 化しているため、bin では出ない pedantic (must_use / Errors 節) を抑制する
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

mod autostart;
mod bootstrap;
mod config;
mod consts;
mod hotkey_binding;
mod logger;
mod platform;
pub mod refiner;
mod security;
mod tray;

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;

pub use refiner::RefineMode;

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションを起動する
///
/// ロギングの初期化、コマンドライン引数の解析、常駐またはワンショット実行を行う
///
/// # Returns
/// * `anyhow::Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
pub fn run() -> anyhow::Result<()> {
    bootstrap::run()
}
