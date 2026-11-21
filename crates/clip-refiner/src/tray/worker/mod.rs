//! クリップボード操作と監視を単一スレッドで処理するワーカー
//!
//! 本文 I/O は [`OwnedClipboard`] を 1 インスタンスだけ保持する
//! 変更検知は `clipboard_seq` (Open 不要) と `clipboard_listener` (本文非読) に分離する
//! 設定の外部編集は `config_watcher` で起床し、内容指紋で判定する

mod clipboard_listener;
mod clipboard_seq;
mod command;
mod config_watcher;
mod handle;
mod handlers;
mod monitor_loop;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use super::dispatch;
use super::state::{AppState, ConfigEvent, WorkerEvent};
use crate::refiner::OwnedClipboard;
use clipboard_listener::{ClipboardFormatListener, clear_clipboard_changed_pending};
use config_watcher::{ConfigFileWatcher, clear_config_changed_pending};
use monitor_loop::{MonitorLoopState, should_run_monitor_tick, sync_monitor_generation};

pub use command::ClipboardCommand;
pub use handle::ClipboardWorkerHandle;
pub(crate) use handlers::handle_command;

/// 設定ディレクトリ監視が使えないときの外部変更ポーリング間隔
pub(super) const CONFIG_POLL_INTERVAL: Duration = Duration::from_secs(2);
/// 設定ディレクトリ監視稼働時の安全網ポーリング間隔
const CONFIG_WATCHER_FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(30);

// ======================================================================
// ワーカースレッド
// ======================================================================
/// クリップボードワーカーのメインループを実行する
///
/// 初期化失敗・`Shutdown` コマンド・受信チャネル切断で終了する
fn run_worker_loop(
    state: &Arc<AppState>,
    rx: &mpsc::Receiver<ClipboardCommand>,
    listener_tx: Sender<ClipboardCommand>,
) {
    let mut clipboard = match OwnedClipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            crate::log_error!("クリップボード初期化エラー: {:?}", e);
            state.worker().set_recovery_pending(true);
            dispatch::send_app_event(&state.proxy, WorkerEvent::Stopped.into());
            return;
        }
    };

    let notify_recovery = state.worker().take_recovery_pending();
    state.worker().set_alive(true);
    if notify_recovery {
        dispatch::send_app_event(&state.proxy, WorkerEvent::Ready.into());
    }

    let pending_clipboard_change = Arc::new(AtomicBool::new(false));
    let config_watch_tx = listener_tx.clone();
    let listener = match ClipboardFormatListener::start(
        listener_tx,
        Arc::clone(&pending_clipboard_change),
    ) {
        Ok(listener) => {
            crate::log_info!("クリップボード変更リスナを開始しました");
            Some(listener)
        }
        Err(err) => {
            crate::log_warn!(
                "クリップボード変更リスナを開始できないため、シーケンストークンの周期監視にフォールバックします: {err}"
            );
            None
        }
    };
    let listener_active = listener.is_some();

    let pending_config_change = Arc::new(AtomicBool::new(false));
    let config_watcher = match ConfigFileWatcher::start(
        config_watch_tx,
        Arc::clone(&pending_config_change),
    ) {
        Ok(watcher) => {
            crate::log_info!("設定ディレクトリ監視を開始しました");
            Some(watcher)
        }
        Err(err) => {
            crate::log_warn!(
                "設定ディレクトリ監視を開始できないため、内容指紋の周期確認にフォールバックします: {err}"
            );
            None
        }
    };
    let config_poll_interval = if config_watcher.is_some() {
        CONFIG_WATCHER_FALLBACK_POLL_INTERVAL
    } else {
        CONFIG_POLL_INTERVAL
    };

    let mut monitor = MonitorLoopState::new();
    let mut last_config_poll = Instant::now();

    loop {
        sync_monitor_generation(&mut monitor, &mut clipboard, state);

        if last_config_poll.elapsed() >= config_poll_interval {
            last_config_poll = Instant::now();
            try_reload_if_config_changed(state);
        }

        let timeout = monitor.recv_timeout(state, listener_active, config_poll_interval);
        let should_exit = match rx.recv_timeout(timeout) {
            Ok(ClipboardCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => true,
            Ok(ClipboardCommand::ClipboardChanged) => {
                clear_clipboard_changed_pending(&pending_clipboard_change);
                false
            }
            Ok(ClipboardCommand::ConfigFileChanged) => {
                clear_config_changed_pending(&pending_config_change);
                try_reload_if_config_changed(state);
                false
            }
            Ok(cmd) => {
                handle_command(&mut clipboard, state, &mut monitor.refine_ctx, cmd);
                false
            }
            Err(RecvTimeoutError::Timeout) => false,
        };
        if should_exit {
            break;
        }

        if should_run_monitor_tick(state, &monitor) {
            monitor.tick(&mut clipboard, state);
        }
    }

    if let Some(watcher) = config_watcher {
        watcher.stop();
    }
    if let Some(listener) = listener {
        listener.stop();
    }
    state.worker().set_alive(false);
}

/// 内容指紋が最終同期と異なれば設定再読み込みを要求する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
fn try_reload_if_config_changed(state: &Arc<AppState>) {
    if state.config_sync().has_external_change() {
        dispatch::send_app_event(&state.proxy, ConfigEvent::Reload.into());
    }
}

/// ワーカー停止時にユーザーへ案内する通知本文
pub(super) fn worker_stopped_notification_body() -> &'static str {
    "クリップボードの初期化に失敗したか、監視処理が停止しました。トレイメニューの「クリップボード監視を再開」を実行してください"
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::*;
    use crate::config::MonitorMode;
    use crate::refiner::RefineContext;
    use crate::tray::state::test_app_state;
    use crate::tray::worker::clipboard_seq;

    fn active_monitor(generation: u64) -> MonitorLoopState {
        let mut monitor = MonitorLoopState::new();
        monitor.tracked_generation = generation;
        monitor
    }

    /// Event 監視の状態と監視ループを生成する (実効モードが Event にならない環境では `None`)
    fn event_mode_monitor() -> Option<(Arc<AppState>, MonitorLoopState)> {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.is_paused = false;
            c.monitor_mode = MonitorMode::Event;
        });
        state.monitor_generation.store(1, Ordering::SeqCst);

        (MonitorLoopState::effective_mode(MonitorMode::Event) == MonitorMode::Event)
            .then(|| (state, active_monitor(1)))
    }

    /// 監視中かつ世代が一致する場合はティックを実行すること
    #[test]
    fn should_run_monitor_tick_when_active() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);
        state.monitor_generation.store(2, Ordering::SeqCst);

        let monitor = active_monitor(2);
        assert!(should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中はティックを実行しないようにすること
    #[test]
    fn should_not_run_monitor_tick_when_paused() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = true);
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 監視世代が 0 の場合はティックを実行しないようにすること
    #[test]
    fn should_not_run_monitor_tick_when_generation_zero() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);

        let monitor = active_monitor(0);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 追跡中の世代と不一致の場合はティックを実行しないようにすること
    #[test]
    fn should_not_run_monitor_tick_when_generation_mismatch() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);
        state.monitor_generation.store(2, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中でも連続記録が有効ならティックを実行すること
    #[test]
    fn should_run_monitor_tick_when_paused_with_continuous_record() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.is_paused = true;
            c.continuous_record_enabled = true;
        });
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中は `recv_timeout` が短い間隔を返すようにすること
    #[test]
    fn recv_timeout_is_short_when_paused() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.is_paused = true;
            c.monitor_mode = MonitorMode::Polling;
            c.interval_ms = 5000;
        });
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert_eq!(
            monitor.recv_timeout(&state, true, CONFIG_POLL_INTERVAL),
            Duration::from_millis(100)
        );
    }

    /// Event 監視でリスナ稼働中は設定ポーリング間隔まで待機すること
    #[test]
    fn recv_timeout_is_idle_when_event_listener_active() {
        let Some((state, monitor)) = event_mode_monitor() else {
            return;
        };

        assert_eq!(
            monitor.recv_timeout(&state, true, CONFIG_POLL_INTERVAL),
            CONFIG_POLL_INTERVAL
        );
    }

    /// Event 監視でリスナ未使用時は短周期 peek 間隔となるようにすること
    #[test]
    fn recv_timeout_is_short_when_event_listener_inactive() {
        use crate::tray::clipboard_monitor::{EVENT_POLL_MS, POLL_TICK_MS};

        let Some((state, monitor)) = event_mode_monitor() else {
            return;
        };

        assert_eq!(
            monitor.recv_timeout(&state, false, CONFIG_POLL_INTERVAL),
            Duration::from_millis(POLL_TICK_MS.min(EVENT_POLL_MS))
        );
    }

    /// 連続記録でリスナ稼働中は設定ポーリング間隔まで待機すること
    #[test]
    fn recv_timeout_is_idle_when_continuous_record_listener_active() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.is_paused = false;
            c.monitor_mode = MonitorMode::Polling;
            c.continuous_record_enabled = true;
            c.interval_ms = 5000;
        });
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert_eq!(
            monitor.recv_timeout(&state, true, CONFIG_POLL_INTERVAL),
            CONFIG_POLL_INTERVAL
        );
    }

    /// 設定が Polling の場合は実効監視モードも Polling となるようにすること
    #[test]
    fn effective_mode_is_polling_when_configured() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);

        assert_eq!(
            MonitorLoopState::effective_mode(state.with_config(|c| c.monitor_mode)),
            MonitorMode::Polling
        );
    }

    /// Event 設定でもウォッチャー非対応時は Polling にフォールバックすること
    #[test]
    fn effective_mode_falls_back_when_event_unsupported() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Event);

        let effective = MonitorLoopState::effective_mode(state.with_config(|c| c.monitor_mode));

        if clipboard_seq::supports_event_monitor() {
            assert_eq!(effective, MonitorMode::Event);
        } else {
            assert_eq!(effective, MonitorMode::Polling);
        }
    }

    /// クリップボード画像を登録できるようにすること
    #[test]
    fn register_clip_from_clipboard_saves_image() {
        use crate::test_helpers::InMemoryTextClipboard;

        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            let rgba = vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ];
            let mut clipboard =
                InMemoryTextClipboard::with_text("ignored").with_source_image(2, 2, rgba);

            handlers::register_clip_from_clipboard(&mut clipboard, &state);

            let is_image = state.with_config(|c| {
                c.clips
                    .first()
                    .and_then(|e| e.image_file.as_ref())
                    .is_some()
            });
            assert!(is_image);
        });
    }

    /// 登録画像をクリップボードへコピーできるようにすること
    #[test]
    fn copy_registered_writes_image_to_clipboard() {
        use crate::test_helpers::InMemoryTextClipboard;

        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            let rgba = vec![
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
            ];
            state.with_config_mut(|c| {
                c.add_registered_image(2, 2, &rgba).expect("register image");
            });

            let mut clipboard = InMemoryTextClipboard::with_text("");
            let mut refine_ctx = RefineContext::default();
            handle_command(
                &mut clipboard,
                &state,
                &mut refine_ctx,
                ClipboardCommand::CopyRegisteredClip(0),
            );

            assert_eq!(clipboard.written_image_size(), Some((2, 2)));
        });
    }
}
