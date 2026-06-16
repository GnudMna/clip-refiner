use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use super::clipboard_change::ChangeWatcher;
use super::notifier;
use super::state::{AppState, MonitorSnapshot, ProcessedState};
use crate::config::MonitorMode;
use crate::notification;
use crate::refiner::process_clipboard;

use anyhow::{Context, Result};
use arboard::Clipboard;

// ======================================================================
// 監視スレッド管理
// ======================================================================
/// クリップボード監視スレッドを開始する
///
/// 現在の監視モード設定（ポーリングまたはイベント）に基づいて、適切な監視スレッドを起動します。
/// 一時停止中（`is_paused == true`）の場合はスレッドを起動しません。
/// スレッドの世代管理を行い、設定変更時に古いスレッドが自動的に終了するように制御します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn spawn_monitor_thread(state: Arc<AppState>) {
    if state.with_config(|c| c.is_paused) {
        return;
    }

    let monitor_mode = state.with_config(|c| c.monitor_mode);
    let generation = state.monitor_generation.fetch_add(1, Ordering::SeqCst) + 1;

    match monitor_mode {
        MonitorMode::Polling => spawn_polling_monitor_thread(state, generation),
        MonitorMode::Event => {
            if ChangeWatcher::new().is_supported() {
                spawn_event_monitor_thread(state, generation);
            } else {
                crate::log_warn!(
                    "イベント監視が利用できないため、ポーリング監視にフォールバックします"
                );
                spawn_polling_monitor_thread(state, generation);
            }
        }
    }
}

// ======================================================================
// クリップボード更新処理
// ======================================================================
/// 加工を試みるべきか判定する。スキップする場合は `processed_state` を更新する。
///
/// # Returns
/// * `true` - 加工を試みる
/// * `false` - スキップする
pub(crate) fn should_process_clipboard(
    ps: &mut ProcessedState,
    text: &str,
    event_driven: bool,
) -> bool {
    if text.is_empty() {
        return false;
    }

    // 自身の書き戻しによるクリップボード変更イベントを1回無視
    if ps.last_written_text.as_deref() == Some(text) {
        ps.last_written_text = None;
        ps.last_seen_text = text.to_string();
        return false;
    }

    // ポーリング: 前回と同じ内容なら加工しない
    if !event_driven && text == ps.last_seen_text {
        return false;
    }

    true
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
/// * `event_driven` - イベント駆動監視の場合は `true`、ポーリングの場合は `false`
///
/// # Returns
/// * `bool` - 加工が実行され、クリップボードが更新された場合は `true`、それ以外は `false` を返します。
pub fn handle_clipboard_update(
    clipboard: &mut Clipboard,
    state: &Arc<AppState>,
    snap: &MonitorSnapshot,
    event_driven: bool,
) -> bool {
    let Ok(text) = clipboard.get_text() else {
        return false;
    };

    let should_process = state.with_processed_state(|ps| {
        should_process_clipboard(ps, &text, event_driven)
    });

    if !should_process {
        return false;
    }

    if let Some(processed) = process_clipboard(clipboard, snap.mode) {
        state.record_processing_success(&processed);
        notifier::show_process_notification(state, snap.mode, &processed);

        if snap.history_enabled {
            state.add_to_history(processed);
        }
        return true;
    }

    state.record_clipboard_observed(&text);

    if snap.history_enabled {
        state.add_to_history(text);
    }

    false
}

// ======================================================================
// ポーリング監視
// ======================================================================
/// ポーリングスリープを分割するチック間隔（ミリ秒）
///
/// この値ごとに停止条件を確認するため、スレッド停止の最大遅延がこの値に抑えられます。
const POLL_TICK_MS: u64 = 50;

/// イベント監視ループのスリープ間隔（ミリ秒）
const EVENT_POLL_MS: u64 = 100;

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
            state.record_clipboard_observed(&current_text);
        }

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

            // interval_ms を POLL_TICK_MS 刻みで分割してスリープし、
            // 各チックで停止条件を確認することでスレッド停止の最大遅延を POLL_TICK_MS に抑える
            let mut elapsed = 0u64;
            while elapsed < snap.interval_ms {
                let tick = POLL_TICK_MS.min(snap.interval_ms - elapsed);
                thread::sleep(Duration::from_millis(tick));
                elapsed += tick;
                if state.monitor_generation.load(Ordering::SeqCst) != generation
                    || state.with_config(|c| c.is_paused)
                {
                    return;
                }
            }

            handle_clipboard_update(&mut clipboard, &state, &snap, false);
        }
    });
}

// ======================================================================
// イベント監視
// ======================================================================
/// OSの変更トークンを利用してクリップボードを監視するスレッドを開始する
///
/// クリップボード本文の定期読み取りを避け、変更トークンの変化時のみ内容を取得します。
/// ポーリングより低遅延かつ低CPU負荷で動作します。
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `generation` - このスレッドの世代番号。
pub fn spawn_event_monitor_thread(state: Arc<AppState>, generation: u64) {
    thread::spawn(move || {
        let watcher = ChangeWatcher::new();

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
        state.record_clipboard_observed(&current_text);
        let mut last_token = watcher.token().unwrap_or(0);

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

            if let Some(token) = watcher.token()
                && token != last_token
            {
                last_token = token;

                // クリップボードの更新を処理し、加工が行われたかチェック
                if handle_clipboard_update(&mut clipboard, &state, &snap, true) {
                    // 加工が実行された場合、クリップボードが変更されたのでトークンを再取得して更新
                    last_token = watcher.token().unwrap_or(last_token);
                }
            }

            // 変化がない時のCPU負荷を抑えつつ、停止条件を定期的に確認する
            let mut elapsed = 0u64;
            while elapsed < EVENT_POLL_MS {
                let tick = POLL_TICK_MS.min(EVENT_POLL_MS - elapsed);
                thread::sleep(Duration::from_millis(tick));
                elapsed += tick;
                if state.monitor_generation.load(Ordering::SeqCst) != generation
                    || state.with_config(|c| c.is_paused)
                {
                    return;
                }
            }
        }
    });
}

// ======================================================================
// ユーティリティ
// ======================================================================
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
    match monitor_mode {
        MonitorMode::Event => menu.interval.main_submenu.set_enabled(false),
        MonitorMode::Polling => menu.interval.main_submenu.set_enabled(true),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_skip_own_write_back_echo() {
        let mut ps = ProcessedState {
            last_seen_text: "input".to_string(),
            last_written_text: Some("output".to_string()),
        };

        assert!(!should_process_clipboard(&mut ps, "output", true));
        assert_eq!(ps.last_written_text, None);
        assert_eq!(ps.last_seen_text, "output");
    }

    #[test]
    fn polling_skips_unchanged_text() {
        let mut ps = ProcessedState {
            last_seen_text: "same".to_string(),
            last_written_text: None,
        };

        assert!(!should_process_clipboard(&mut ps, "same", false));
        assert!(should_process_clipboard(&mut ps, "same", true));
    }

    #[test]
    fn event_mode_allows_recopy_of_processed_output() {
        let mut ps = ProcessedState {
            last_seen_text: "processed".to_string(),
            last_written_text: None,
        };

        // 加工結果と同じ文字列の再コピー（イベント駆動）も加工対象とする
        assert!(should_process_clipboard(&mut ps, "processed", true));
    }

    #[test]
    fn event_mode_allows_recopy_of_source_text() {
        let mut ps = ProcessedState {
            last_seen_text: "processed".to_string(),
            last_written_text: None,
        };

        assert!(should_process_clipboard(&mut ps, "  source  ", true));
    }

    #[test]
    fn spawn_monitor_thread_skips_when_paused() {
        use crate::tray::state::AppEvent;
        use std::sync::{Arc, atomic::Ordering};
        use tao::event_loop::EventLoopBuilder;
        #[cfg(windows)]
        use tao::platform::windows::EventLoopBuilderExtWindows;

        #[cfg(windows)]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event()
            .with_any_thread(true)
            .build();
        #[cfg(not(windows))]
        let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();

        let state = Arc::new(AppState::new(event_loop.create_proxy()));
        state.with_config_mut(|c| c.is_paused = true);

        spawn_monitor_thread(Arc::clone(&state));
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }
}
