use anyhow::{Context, Result};
use arboard::Clipboard;
use std::sync::{Arc, atomic::Ordering};
use std::thread;
use std::time::Duration;

use super::state::{AppEvent, AppState};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

/// クリップボード監視スレッドを開始する。
///
/// 現在の監視モード設定（ポーリングまたはイベント）に応じて、適切な監視スレッドを起動する。
/// スレッドの世代管理を行い、設定変更時に古いスレッドが自動的に終了するようにする。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
pub fn spawn_monitor_thread(state: Arc<AppState>) {
    let monitor_mode = state.get_monitor_mode();
    let generation = state.monitor_generation.fetch_add(1, Ordering::SeqCst) + 1;

    match monitor_mode {
        MonitorMode::Polling => spawn_polling_monitor_thread(state, generation),
        #[cfg(windows)]
        MonitorMode::Event => spawn_event_monitor_thread(state, generation),
    }
}

/// クリップボードの更新を検知し、必要であれば加工処理を行う
///
/// # Arguments
/// * `clipboard` - クリップボードのインスタンス
/// * `state` - アプリケーションの状態
///
/// # Returns
/// 加工が実行され、クリップボードへの書き込みも行われた場合は `true`、それ以外の場合は `false` を返す。
pub fn handle_clipboard_update(clipboard: &mut Clipboard, state: &Arc<AppState>) -> bool {
    if let Ok(text) = clipboard.get_text() {
        let shared_last = state.get_last_processed_text();

        if !text.is_empty() && text != shared_last {
            let current_mode = state.get_mode();
            if let Some(processed) = process_clipboard(clipboard, current_mode) {
                state.set_last_processed_text(processed.clone());
                show_process_notification(current_mode, &processed);

                if state.history_enabled.load(Ordering::Relaxed) {
                    state.add_to_history(processed);
                    let _ = state.proxy.send_event(AppEvent::RefreshHistory);
                }
                return true;
            }

            if state.history_enabled.load(Ordering::Relaxed) {
                state.add_to_history(text.clone());
                let _ = state.proxy.send_event(AppEvent::RefreshHistory);
            }
        }
        state.set_last_processed_text(text);
    }
    false // 加工されなかった
}

/// ポーリング方式でクリップボードを監視するスレッドを開始する。
///
/// 一定間隔でクリップボードの内容を確認し、変更があった場合に加工処理を呼び出す。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `generation` - このスレッドの世代番号。
pub fn spawn_polling_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                notification::error::show_anyhow_error("監視スレッドエラー", &e);
                return;
            }
        };

        {
            let current_text = clipboard.get_text().unwrap_or_default();
            state.set_last_processed_text(current_text);
        }

        loop {
            // 監視モード変更時にスレッドを終了（最新の世代でないなら終了）
            if state.monitor_generation.load(Ordering::SeqCst) != generation {
                break;
            }

            let interval = state.interval_ms.load(Ordering::Relaxed);
            thread::sleep(Duration::from_millis(interval));

            if state.paused.load(Ordering::Relaxed) {
                continue;
            }

            handle_clipboard_update(&mut clipboard, &state);
        }
    });
}

/// イベント方式でクリップボードを監視するスレッドを開始する（Windows限定）。
///
/// OSのクリップボード更新イベントをリッスンし、変更があった場合に加工処理を呼び出す。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態。
/// * `generation` - このスレッドの世代番号。
#[cfg(windows)]
pub fn spawn_event_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        use clipboard_win::raw::seq_num;

        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                notification::error::show_anyhow_error("監視スレッドエラー", &e);
                return;
            }
        };

        let current_text = clipboard.get_text().unwrap_or_default();
        state.set_last_processed_text(current_text);
        let mut last_seq = seq_num().map(|s| s.get()).unwrap_or(0);

        loop {
            // 監視モード変更時にスレッドを終了（最新の世代でないなら終了）
            if state.monitor_generation.load(Ordering::SeqCst) != generation {
                break;
            }

            if state.paused.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(200));
                continue;
            }

            // クリップボードのシーケンス番号をチェック
            if let Some(seq_nonzero) = seq_num() {
                let seq = seq_nonzero.get();
                if seq != last_seq {
                    last_seq = seq;

                    // クリップボードの更新を処理し、加工が行われたかチェック
                    if handle_clipboard_update(&mut clipboard, &state) {
                        // 加工が実行された場合、クリップボードが変更されたのでシーケンス番号を再取得して更新
                        last_seq = seq_num().map(|s| s.get()).unwrap_or(last_seq);
                    }
                }
            }

            // 変化がない時のCPU負荷を抑える
            thread::sleep(Duration::from_millis(100));
        }
    });
}

/// 処理完了通知を表示する
///
/// # Arguments
/// * `mode` - 実行された `RefineMode`。
/// * `text` - 加工後のテキスト。
#[cfg(debug_assertions)]
pub fn show_process_notification(mode: RefineMode, text: &str) {
    let snippet = if text.chars().count() > 50 {
        format!("{}...", text.chars().take(47).collect::<String>())
    } else {
        text.to_string()
    };
    notification::success::show_success_debug_notification(
        "変換完了",
        &format!("モード: {}\n内容: {}", mode.label(), snippet),
    );
}

/// 処理完了通知を表示する (リリースビルドでは何もしない)
///
/// # Arguments
/// * `_mode` - 実行された `RefineMode` (未使用)。
/// * `_text` - 加工後のテキスト (未使用)。
#[cfg(not(debug_assertions))]
pub fn show_process_notification(_mode: RefineMode, _text: &str) {}

/// クリップボード機能へのアクセスを初期化する。
///
/// # Returns
/// 初期化に成功した場合は`Ok(Clipboard)`、失敗した場合は`Err`を返す。
pub fn init_clipboard() -> Result<Clipboard> {
    Clipboard::new().context("クリップボードのアクセスに失敗しました")
}

/// 監視方式切り替え時のUI更新処理（OS依存）
///
/// # Arguments
/// * `menu` - トレイメニューのインスタンス。
/// * `monitor_mode` - 新しく選択された監視方式。
pub fn update_monitor_mode_impl(menu: &super::menu::TrayMenu, monitor_mode: MonitorMode) {
    #[cfg(windows)]
    match monitor_mode {
        MonitorMode::Event => menu.interval_submenu.set_enabled(false),
        MonitorMode::Polling => menu.interval_submenu.set_enabled(true),
    }

    #[cfg(not(windows))]
    match monitor_mode {
        MonitorMode::Polling => menu.interval_submenu.set_enabled(true),
    }
}
