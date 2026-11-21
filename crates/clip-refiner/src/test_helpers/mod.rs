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

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
mod ipc;

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
pub use worker::{
    is_worker_alive, set_worker_alive, set_worker_recovery_pending, stub_worker,
    test_app_state_arc, test_tray_menu, wait_until_worker_alive, with_clipboard_worker_spawn_lock,
};

#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use crate::ipc::{
    OneshotIpcResponse, OneshotIpcServer, OneshotIpcStatus, OneshotProcessResult,
};
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub use ipc::{
    process_oneshot_via_pipe, send_raw_oneshot_line, spawn_oneshot_stub_responder,
    start_oneshot_ipc_for_test, unique_oneshot_pipe_name, wait_for_oneshot_pipe,
};

pub use clip_refiner_core::config::MonitorMode;
pub use clip_refiner_core::test_helpers::{InMemoryTextClipboard, with_temp_config_dir};

/// クリップボード本文の処理上限 (バイト)
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = clip_refiner_core::consts::MAX_CLIPBOARD_TEXT_BYTES;
