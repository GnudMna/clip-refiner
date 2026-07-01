//! クリップボード操作と監視を単一スレッドで処理するワーカー

mod command;
mod handle;
mod handlers;
mod monitor_loop;

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use monitor_loop::{MonitorLoopState, should_run_monitor_tick, sync_monitor_generation};

pub(crate) use handlers::handle_command;

use super::clipboard_change::ChangeWatcher;
use super::dispatch;
use super::state::{AppEvent, AppState};

use arboard::Clipboard;

pub use command::ClipboardCommand;
pub use handle::ClipboardWorkerHandle;

/// 設定ファイルの外部変更を検知するポーリング間隔
const CONFIG_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

// ======================================================================
// ワーカースレッド
// ======================================================================
/// クリップボードワーカーのメインループを実行する
///
/// 初期化失敗・`Shutdown` コマンド・受信チャネル切断で終了する
fn run_worker_loop(state: &Arc<AppState>, rx: &mpsc::Receiver<ClipboardCommand>) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            crate::log_error!("クリップボード初期化エラー: {:?}", e);
            state.set_worker_recovery_pending(true);
            dispatch::send_app_event(&state.proxy, AppEvent::ClipboardWorkerStopped);
            return;
        }
    };

    let notify_recovery = state.take_worker_recovery_pending();
    state.set_worker_alive(true);
    if notify_recovery {
        dispatch::send_app_event(&state.proxy, AppEvent::ClipboardWorkerReady);
    }

    let watcher = ChangeWatcher::new();
    let mut monitor = MonitorLoopState::new();
    let mut last_config_poll = Instant::now();

    loop {
        sync_monitor_generation(&mut monitor, &mut clipboard, state, &watcher);

        if last_config_poll.elapsed() >= CONFIG_POLL_INTERVAL {
            last_config_poll = Instant::now();
            if state.has_external_config_change() {
                dispatch::send_app_event(&state.proxy, AppEvent::ReloadConfig);
            }
        }

        let timeout = monitor.recv_timeout(state, &watcher);
        let should_exit = match rx.recv_timeout(timeout) {
            Ok(ClipboardCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => true,
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
            monitor.tick(&mut clipboard, state, &watcher);
        }
    }

    state.set_worker_alive(false);
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

    use super::*;

    use std::time::Duration;

    use crate::config::MonitorMode;
    use crate::refiner::RefineContext;
    use crate::tray::clipboard_change::ChangeWatcher;
    use crate::tray::state::test_app_state;

    fn active_monitor(generation: u64) -> MonitorLoopState {
        let mut monitor = MonitorLoopState::new();
        monitor.tracked_generation = generation;
        monitor
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

    /// 一時停止中はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_paused() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = true);
        state.monitor_generation.store(1, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 監視世代が 0 の場合はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_generation_zero() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);

        let monitor = active_monitor(0);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 追跡中の世代と不一致の場合はティックを実行しないこと
    #[test]
    fn should_not_run_monitor_tick_when_generation_mismatch() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.is_paused = false);
        state.monitor_generation.store(2, Ordering::SeqCst);

        let monitor = active_monitor(1);
        assert!(!should_run_monitor_tick(&state, &monitor));
    }

    /// 一時停止中は `recv_timeout` が短い間隔を返すこと
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
        let watcher = ChangeWatcher::new();
        assert_eq!(
            monitor.recv_timeout(&state, &watcher),
            Duration::from_millis(100)
        );
    }

    /// 設定が Polling の場合は実効監視モードも Polling であること
    #[test]
    fn effective_mode_is_polling_when_configured() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);

        let watcher = ChangeWatcher::new();
        assert_eq!(
            MonitorLoopState::effective_mode(&watcher, &state),
            MonitorMode::Polling
        );
    }

    /// Event 設定でもウォッチャー非対応時は Polling にフォールバックすること
    #[test]
    fn effective_mode_falls_back_when_event_unsupported() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Event);

        let watcher = ChangeWatcher::new();
        let effective = MonitorLoopState::effective_mode(&watcher, &state);

        if watcher.is_supported() {
            assert_eq!(effective, MonitorMode::Event);
        } else {
            assert_eq!(effective, MonitorMode::Polling);
        }
    }

    /// クリップボード画像を登録できること
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

    /// 登録画像をクリップボードへコピーできること
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
