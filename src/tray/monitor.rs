use std::sync::{Arc, atomic::Ordering};
use std::thread;
use std::time::Duration;

use super::notifier;
use super::state::AppState;
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::process_clipboard;

use anyhow::{Context, Result};
use arboard::Clipboard;

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
                notifier::show_process_notification(state, current_mode, &processed);

                if state.is_history_enabled() {
                    state.add_to_history(processed);
                }
                return true;
            }

            if state.is_history_enabled() {
                state.add_to_history(text.clone());
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
                notification::show_anyhow_error("監視スレッドエラー", &e);
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

            let interval = state.interval_ms();
            thread::sleep(Duration::from_millis(interval));

            if state.is_paused() {
                break;
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
                notification::show_anyhow_error("監視スレッドエラー", &e);
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

            if state.is_paused() {
                break;
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
        MonitorMode::Event => menu.interval.main_submenu.set_enabled(false),
        MonitorMode::Polling => menu.interval.main_submenu.set_enabled(true),
    }

    #[cfg(not(windows))]
    match monitor_mode {
        MonitorMode::Polling => menu.interval.main_submenu.set_enabled(true),
    }
}
