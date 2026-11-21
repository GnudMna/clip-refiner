//! 単体テスト・統合テストで共有するヘルパー
//!
//! デバッグビルド、`cargo test` 実行時、または `test-helpers` feature 有効時に利用可能

#![allow(clippy::missing_panics_doc)]

mod harness;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
mod settings;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
mod js_runner;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
mod platform;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
mod worker;

pub use harness::ClipboardHarness;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use js_runner::run_js_test_script;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use platform::{assert_test_hotkeys_register, query_autostart_enabled};

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use settings::SettingsTestContext;

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use worker::{stub_worker, test_app_state_arc, test_tray_menu, wait_until_worker_alive};

pub use clip_refiner_core::config::MonitorMode;
pub use clip_refiner_core::test_helpers::{InMemoryTextClipboard, with_temp_config_dir};

/// クリップボード本文の処理上限 (バイト)
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = clip_refiner_core::consts::MAX_CLIPBOARD_TEXT_BYTES;
