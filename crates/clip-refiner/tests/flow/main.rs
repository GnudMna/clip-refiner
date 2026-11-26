//! 統合テストの単一エントリ
//!
//! アプリ全体 (wry / windows / tray-icon など) をリンクする実行ファイルが重いため、
//! 統合テストは 1 つの実行ファイルにまとめている。テストを足すときは `tests/flow/` に
//! ファイルを作り、ここへ `mod` を 1 行足す。絞り込みは `cargo test --test flow monitor::`

mod architecture_doc;
mod clips;
mod history;
mod license_ui;
mod monitor;
mod notification;
mod ocr;
mod oneshot;
mod oneshot_ipc;
mod platform;
mod record;
mod regex;
mod regex_preview_logic;
mod selector_logic;
mod settings;
mod settings_context_logic;
mod settings_panels_logic;
mod settings_shared_logic;
mod settings_ui_logic;
mod ui_bundle;
mod worker;
mod worker_recovery;
