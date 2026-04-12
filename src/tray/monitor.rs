use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use super::notifier;
use super::state::{AppState, MonitorSnapshot};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::process_clipboard;

use anyhow::{Context, Result};
use arboard::Clipboard;

/// クリップボード監視スレッドを開始する
///
/// 現在の監視モード設定（ポーリングまたはイベント）に基づいて、適切な監視スレッドを起動します。
/// スレッドの世代管理を行い、設定変更時に古いスレッドが自動的に終了するように制御します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn spawn_monitor_thread(state: Arc<AppState>) {
    let monitor_mode = state.get_monitor_mode();
    let generation = state.monitor_generation.fetch_add(1, Ordering::SeqCst) + 1;

    match monitor_mode {
        MonitorMode::Polling => spawn_polling_monitor_thread(state, generation),
        #[cfg(windows)]
        MonitorMode::Event => spawn_event_monitor_thread(state, generation),
    }
}

/// クリップボードの内容更新を検知し、必要に応じて加工処理を行う
///
/// 内容に変更があった場合、現在の加工モードを適用し、結果をクリップボードに書き戻します。
/// 通知の表示や履歴への追加もここで行われます。
///
/// # Arguments
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `snap` - ループ先頭で取得済みの設定スナップショット
///
/// # Returns
/// * `bool` - 加工が実行され、クリップボードが更新された場合は `true`、それ以外は `false` を返します。
pub fn handle_clipboard_update(
    clipboard: &mut Clipboard,
    state: &Arc<AppState>,
    snap: &MonitorSnapshot,
) -> bool {
    if let Ok(text) = clipboard.get_text() {
        let shared_last = state.get_last_processed_text();

        if !text.is_empty() && text != shared_last {
            if let Some(processed) = process_clipboard(clipboard, snap.mode) {
                state.set_last_processed_text(processed.clone());
                notifier::show_process_notification(state, snap.mode, &processed);

                if snap.history_enabled {
                    state.add_to_history(processed);
                }
                return true;
            }

            if snap.history_enabled {
                state.add_to_history(text.clone());
            }
        }
        state.set_last_processed_text(text);
    }
    false // 加工されなかった
}

/// ポーリング（定時確認）方式でクリップボードを監視するスレッドを開始する
///
/// 一定間隔（デフォルト1秒など）でクリップボードの内容を確認します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `generation` - このスレッドの世代番号。最新でない世代のスレッドは自動終了します。
pub fn spawn_polling_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                crate::log_error!("ポーリング監視スレッド初期化エラー: {:?}", e);
                notification::show_notification(
                    "監視スレッドエラー",
                    "クリップボードへのアクセスに失敗しました。クリップボード監視は停止します。",
                );
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

            // config RwLock を1回のみ取得してスナップショットを作成
            let snap = state.monitor_snapshot();
            thread::sleep(Duration::from_millis(snap.interval_ms));

            if snap.is_paused {
                break;
            }

            handle_clipboard_update(&mut clipboard, &state, &snap);
        }
    });
}

/// OSのイベント通知方式でクリップボードを監視するスレッドを開始する（Windows限定）
///
/// クリップボードの内容が書き換わった際にOSから送られる通知をリッスンします。
/// ポーリングよりも低負荷かつ低遅延で動作します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `generation` - このスレッドの世代番号。
#[cfg(windows)]
pub fn spawn_event_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        use clipboard_win::raw::seq_num;

        let mut clipboard = match init_clipboard() {
            Ok(cb) => cb,
            Err(e) => {
                crate::log_error!("イベント監視スレッド初期化エラー: {:?}", e);
                notification::show_notification(
                    "監視スレッドエラー",
                    "クリップボードへのアクセスに失敗しました。クリップボード監視は停止します。",
                );
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

            // config RwLock を1回のみ取得してスナップショットを作成
            let snap = state.monitor_snapshot();
            if snap.is_paused {
                break;
            }

            // クリップボードのシーケンス番号をチェック
            if let Some(seq_nonzero) = seq_num() {
                let seq = seq_nonzero.get();
                if seq != last_seq {
                    last_seq = seq;

                    // クリップボードの更新を処理し、加工が行われたかチェック
                    if handle_clipboard_update(&mut clipboard, &state, &snap) {
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

/// クリップボード機能への初期アクセスを確立する
///
/// # Returns
/// * `Result<Clipboard>` - 初期化された `Clipboard` インスタンス。失敗した場合はエラーを返します。
pub fn init_clipboard() -> Result<Clipboard> {
    Clipboard::new().context("クリップボードのアクセスに失敗しました")
}

/// 監視方式の切り替えに伴い、関連するUIコンポーネントの状態を更新する
///
/// 例えば、イベントモード時は「監視周期」の設定メニューを無効化します。
///
/// # Arguments
/// * `menu` - トレイメニュー構造体
/// * `monitor_mode` - 新しく選択された監視方式
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
