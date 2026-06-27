//! クリップボードのテキストを加工するツール `ClipRefiner` のライブラリクレート
//!
//! 加工モード ([`RefineMode`])、設定 (`config`)、常駐 UI (`tray`) を提供する
//!
//! # Examples
//!
//! ```
//! use clip_refiner::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
//! assert_eq!(output, "hello world");
//! ```

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

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub mod test_helpers;

pub use refiner::RefineMode;
pub use refiner::{RefineContext, Refiner};

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
