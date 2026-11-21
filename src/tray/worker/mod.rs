//! クリップボード操作と監視を単一スレッドで処理するワーカー

mod command;
mod handlers;
mod monitor_loop;

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use monitor_loop::{MonitorLoopState, should_run_monitor_tick, sync_monitor_generation};

pub(crate) use handlers::handle_command;

use super::clipboard_change::ChangeWatcher;
use super::dispatch;
use super::state::{AppEvent, AppState};

use crate::platform;

use arboard::Clipboard;

pub use command::ClipboardCommand;

/// 設定ファイルの外部変更を検知するポーリング間隔
const CONFIG_POLL_INTERVAL: Duration = Duration::from_secs(2);

// ======================================================================
// ワーカースレッド
// ======================================================================
/// クリップボード操作と監視を単一スレッドで処理するワーカーを開始する
///
/// すべてのクリップボード読み書きはこのスレッドに集約され、
/// UI からのコマンドと監視ループが `recv_timeout` で交互に処理される
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `Sender<ClipboardCommand>` - ワーカーに操作を依頼するためのチャネル送信端
pub fn spawn_clipboard_worker(state: Arc<AppState>) -> Sender<ClipboardCommand> {
    let (tx, rx): (Sender<ClipboardCommand>, Receiver<ClipboardCommand>) = mpsc::channel();

    thread::spawn(move || run_worker_loop(&state, &rx));

    tx
}

/// クリップボードワーカースレッドのメインループを実行する
///
/// クリップボードの初期化、変更検知ウォッチャーの生成、監視ループ状態の管理を行い、
/// コマンド受信と監視処理を交互に実行する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `rx` - ワーカーに操作を依頼するためのチャネル受信端
fn run_worker_loop(state: &Arc<AppState>, rx: &Receiver<ClipboardCommand>) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            crate::log_error!("クリップボード初期化エラー: {:?}", e);
            platform::show_notification(
                "クリップボードエラー",
                "クリップボードの初期化に失敗しました。監視処理は停止します。",
            );
            return;
        }
    };

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
        match rx.recv_timeout(timeout) {
            Ok(cmd) => handle_command(&mut clipboard, state, &mut monitor.refine_ctx, cmd),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if should_run_monitor_tick(state, &monitor) {
            monitor.tick(&mut clipboard, state, &watcher);
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::*;

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
