//! クリップボードのテキストを加工するツール `ClipRefiner` のライブラリクレート
//!
//! 加工モード (`refiner`)、設定 (`config`)、常駐 UI (`tray`) を提供する

// アプリケーション本体を lib 化しているため、bin では出ない pedantic (must_use / Errors 節) を抑制する
#![allow(clippy::must_use_candidate, clippy::missing_errors_doc)]

mod app;
mod autostart;
mod config;
mod consts;
mod history_store;
mod hotkey_binding;
mod logger;
mod notification;
pub mod refiner;
mod tray;

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
    app::run()
}
