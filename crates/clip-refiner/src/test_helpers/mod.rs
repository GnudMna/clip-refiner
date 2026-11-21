//! 単体テスト・統合テストで共有するヘルパー
//!
//! デバッグビルド、`cargo test` 実行時、または `test-helpers` feature 有効時に利用可能

#![allow(clippy::missing_panics_doc)]

mod harness;

pub use clip_refiner_core::config::MonitorMode;
pub use clip_refiner_core::test_helpers::{InMemoryTextClipboard, with_temp_config_dir};

pub use harness::ClipboardHarness;

/// クリップボード本文の処理上限 (バイト)
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = clip_refiner_core::consts::MAX_CLIPBOARD_TEXT_BYTES;
