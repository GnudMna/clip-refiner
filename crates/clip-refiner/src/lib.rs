//! クリップボードのテキストを加工するツール `ClipRefiner` のアプリクレート
//!
//! feature `app` (デフォルト有効) でトレイ常駐 UI と [`run`] を提供する
//! 加工ロジック・設定型などのライブラリ API は [`clip_refiner_core`] を直接依存する
//!
//! # ライブラリとして使う場合
//!
//! GUI 依存を避けるには本クレートではなく [`clip-refiner-core`](clip_refiner_core) を使う
//!
//! ```toml
//! [dependencies]
//! clip-refiner-core = { version = "1.3.2" }
//! # テキスト変換のみ: default-features = false
//! ```
//!
//! ```
//! use clip_refiner_core::{RefineContext, RefineMode, Refiner};
//!
//! let ctx = RefineContext::default();
//! let output = RefineMode::UrlDecode.refine("hello%20world", &ctx);
//! assert_eq!(output, "hello world");
//! ```
//!
//! # 互換性
//!
//! アプリの起動 API ([`run`]) と `test_helpers` の変更は `Cargo.toml` の semver に従う
//! 加工・設定の公開 API は `clip-refiner-core` 側の semver に従う
//! 破壊的変更は CHANGELOG の **Breaking changes** 節に記載する

#[cfg(not(target_os = "windows"))]
compile_error!("ClipRefiner は Windows 専用です");

#[cfg(feature = "app")]
mod bootstrap;
#[cfg(feature = "app")]
mod ipc;
#[cfg(feature = "app")]
mod logger;
#[cfg(feature = "app")]
mod platform;
#[cfg(feature = "app")]
mod tray;
#[cfg(feature = "app")]
mod user_error;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
#[cfg(feature = "app")]
pub mod test_helpers;

// ======================================================================
// 内部モジュールエイリアス
// ======================================================================
// アプリ内部向け。公開 API には含めない (ライブラリ利用は clip_refiner_core)
pub(crate) use clip_refiner_core::clip_template;
pub(crate) use clip_refiner_core::config;
pub(crate) use clip_refiner_core::consts;
pub(crate) use clip_refiner_core::refiner;
pub(crate) use clip_refiner_core::security;

// ======================================================================
// エントリポイント
// ======================================================================
/// アプリケーションを起動する
///
/// ロギングの初期化、コマンドライン引数の解析、常駐またはワンショット実行を行う
///
/// feature `app` が必要
///
/// # Returns
/// * `anyhow::Result<()>` - 正常終了時は `Ok(())`、エラー発生時は `Err` を返す
///
/// # Errors
/// ログ初期化、引数解析、クリップボード初期化、常駐ループのいずれかが失敗した場合
#[cfg(feature = "app")]
pub fn run() -> anyhow::Result<()> {
    bootstrap::run()
}
