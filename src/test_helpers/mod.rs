//! 単体テスト・統合テストで共有するヘルパー
//!
//! デバッグビルド、`cargo test` 実行時、または `test-helpers` feature 有効時に利用可能

#![allow(clippy::missing_panics_doc)]

mod clipboard;
mod harness;

pub use crate::config::MonitorMode;

pub use clipboard::InMemoryTextClipboard;
pub use harness::ClipboardHarness;

/// クリップボード本文の処理上限 (バイト)
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = crate::consts::MAX_CLIPBOARD_TEXT_BYTES;
