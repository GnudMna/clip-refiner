use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::super::dispatch;
use super::super::state::{AppState, LockExt, WorkerEvent};
use super::command::ClipboardCommand;
use super::run_worker_loop;
use crate::ipc::OneshotProcessResult;
use crate::refiner::{RefineContext, RefineMode};
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::user_error::{UserNotification, titles};

// ======================================================================
// ワーカーハンドル
// ======================================================================
/// クリップボードワーカーへのコマンド送信と再起動を管理する
pub struct ClipboardWorkerHandle {
    state: Arc<AppState>,
    tx: Mutex<Sender<ClipboardCommand>>,
    /// 稼働中ワーカーの `JoinHandle` (テスト用スタブでは `None`)
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl ClipboardWorkerHandle {
    /// ワーカースレッドを起動し、ハンドルを生成する
    ///
    /// # Arguments
    /// * `state` - 共有アプリケーション状態
    ///
    /// # Returns
    /// * `Arc<Self>` - 起動済みワーカーへのハンドル
    pub fn spawn(state: &Arc<AppState>) -> Arc<Self> {
        let (tx, rx) = mpsc::channel();
        let handle = Arc::new(Self {
            state: Arc::clone(state),
            tx: Mutex::new(tx),
            thread: Mutex::new(None),
        });
        Self::start_thread(&handle, rx);
        handle
    }

    /// コマンドをワーカーへ送信する
    ///
    /// ワーカーが停止中の場合は警告ログと通知を出し、送信しない
    ///
    /// # Arguments
    /// * `command` - ワーカーへ送るクリップボード操作コマンド
    pub fn send(&self, command: ClipboardCommand) {
        if !self.state.worker().is_alive() {
            crate::log_warn!("クリップボードワーカーが停止中のため操作を送信できません");
            UserNotification::static_message(
                titles::WORKER,
                "クリップボード監視が停止しています。トレイメニューの「クリップボード監視を再開」を実行してください",
            )
            .notify();
            return;
        }

        let tx = self.tx.lock_ignore_poison();
        if let Err(err) = tx.send(command) {
            crate::log_error!("クリップボードワーカーへのコマンド送信に失敗: {:?}", err);
            mark_worker_stopped(&self.state);
        }
    }

    /// ワンショット加工をワーカーへ依頼し、結果を待つ
    ///
    /// トレイ通知は出さず、監視の書き戻しエコー回避だけ行う
    ///
    /// # Arguments
    /// * `pipeline` - 適用する変換モード列
    /// * `refine_ctx` - CLI 側で解決済みの加工コンテキスト
    /// * `timeout` - 応答待ち上限
    ///
    /// # Returns
    /// * `Result<OneshotProcessResult, OneshotDispatchError>` - 加工結果
    ///
    /// # Errors
    /// ワーカー停止・送信失敗・応答タイムアウトのいずれか
    pub fn process_oneshot(
        &self,
        pipeline: Vec<RefineMode>,
        refine_ctx: RefineContext,
        timeout: Duration,
    ) -> Result<OneshotProcessResult, OneshotDispatchError> {
        if !self.state.worker().is_alive() {
            return Err(OneshotDispatchError::WorkerStopped);
        }

        let (reply_tx, reply_rx) = mpsc::channel();
        {
            let tx = self.tx.lock_ignore_poison();
            tx.send(ClipboardCommand::ProcessOneshotPipeline {
                pipeline,
                refine_ctx,
                reply: reply_tx,
            })
            .map_err(|_| OneshotDispatchError::WorkerStopped)?;
        }

        reply_rx
            .recv_timeout(timeout)
            .map_err(|_| OneshotDispatchError::Timeout)
    }

    /// 停止したワーカーを再起動する
    ///
    /// 旧スレッドへ `Shutdown` を送り、終了を確認してから新スレッドを起動する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardWorkerHandle`
    pub fn restart(self: &Arc<Self>) {
        self.shutdown_current_thread();

        let (tx, rx) = mpsc::channel();
        *self.tx.lock_ignore_poison() = tx;
        self.state.worker().set_alive(false);
        Self::start_thread(self, rx);
        bump_monitor_generation(&self.state);
        crate::log_info!("クリップボードワーカーの再起動を要求しました");
    }

    /// トレイメニューの再開項目の有効状態を同期する
    ///
    /// # Arguments
    /// * `menu` - 同期対象のトレイメニュー
    pub fn sync_menu_state(&self, menu: &super::super::menu::TrayMenu) {
        menu.set_clipboard_worker_retry_enabled(!self.state.worker().is_alive());
    }

    /// 稼働中スレッドへ `Shutdown` を送り、終了するまで待つ
    fn shutdown_current_thread(&self) {
        {
            let tx = self.tx.lock_ignore_poison();
            let _ = tx.send(ClipboardCommand::Shutdown);
        }
        if let Some(join_handle) = self.thread.lock_ignore_poison().take() {
            let _ = join_handle.join();
        }
    }

    fn start_thread(handle: &Arc<Self>, rx: Receiver<ClipboardCommand>) {
        // ワーカーは `Arc<Self>` を保持しない。保持するとテスト終了時の `Drop` が走らず
        // `OwnedClipboard` がスレッドに残留し、並列テストでヒープ破壊を起こす
        let state = Arc::clone(&handle.state);
        let listener_tx = handle.tx.lock_ignore_poison().clone();
        let join_handle = thread::spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                run_worker_loop(&state, &rx, listener_tx);
            }));
            if result.is_err() {
                crate::log_error!("クリップボードワーカーがパニックで終了しました");
            }
            mark_worker_stopped(&state);
        });
        *handle.thread.lock_ignore_poison() = Some(join_handle);
    }
}

/// ワーカー停止を状態へ反映し、必要なら UI へ通知する
///
/// # Arguments
/// * `state` - 共有アプリケーション状態
fn mark_worker_stopped(state: &AppState) {
    if state.worker().is_alive() {
        state.worker().set_alive(false);
        state.worker().set_recovery_pending(true);
        dispatch::send_app_event(&state.proxy, WorkerEvent::Stopped.into());
    }
}

impl Drop for ClipboardWorkerHandle {
    fn drop(&mut self) {
        self.shutdown_current_thread();
    }
}

/// ワンショット加工をワーカーへ依頼できなかった理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneshotDispatchError {
    /// クリップボードワーカーが停止している
    WorkerStopped,
    /// ワーカーからの応答がタイムアウトした
    Timeout,
}

impl std::fmt::Display for OneshotDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WorkerStopped => {
                write!(
                    f,
                    "クリップボード監視が停止しています。トレイメニューの「クリップボード監視を再開」を実行してください"
                )
            }
            Self::Timeout => write!(f, "常駐プロセスからの応答がタイムアウトしました"),
        }
    }
}

impl std::error::Error for OneshotDispatchError {}

// ======================================================================
// テスト
// ======================================================================
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
impl ClipboardWorkerHandle {
    /// テスト用にチャネル送信のみ行うハンドルを生成する
    ///
    /// # Arguments
    /// * `state` - 共有アプリケーション状態
    /// * `tx` - コマンド送信用チャネル
    ///
    /// # Returns
    /// * `Arc<Self>` - ワーカースレッドを起動しないテスト用ハンドル
    pub fn for_test(state: Arc<AppState>, tx: Sender<ClipboardCommand>) -> Arc<Self> {
        state.worker().set_alive(true);
        Arc::new(Self {
            state,
            tx: Mutex::new(tx),
            thread: Mutex::new(None),
        })
    }
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
    use crate::refiner::RefineMode;
    use crate::test_helpers::{
        TrayTestContext, stub_worker, test_app_state_arc, with_clipboard_worker_spawn_lock,
    };
    use crate::tray::state::test_app_state;

    /// ワーカー停止中はコマンドを送信しないようにすること
    #[test]
    fn send_skips_command_when_worker_stopped() {
        let state = test_app_state_arc();
        let (worker, rx) = stub_worker(&state);
        state.worker().set_alive(false);

        worker.send(ClipboardCommand::ProcessMode(RefineMode::Trim));

        assert!(rx.try_recv().is_err());
    }

    /// ワーカー停止時は再開メニューを有効化すること
    #[test]
    fn sync_menu_state_enables_retry_when_worker_stopped() {
        let ctx = TrayTestContext::new();
        ctx.state().worker().set_alive(false);

        ctx.worker().sync_menu_state(ctx.menu());

        assert!(ctx.menu().retry_clipboard_worker_item.is_enabled());
    }

    /// ワーカー稼働中は再開メニューを無効化すること
    #[test]
    fn sync_menu_state_disables_retry_when_worker_alive() {
        let ctx = TrayTestContext::new();
        ctx.state().worker().set_alive(true);

        ctx.worker().sync_menu_state(ctx.menu());

        assert!(!ctx.menu().retry_clipboard_worker_item.is_enabled());
    }

    /// `restart` で監視世代が進むとすること
    #[test]
    fn restart_bumps_monitor_generation() {
        with_clipboard_worker_spawn_lock(|| {
            let state = Arc::new(test_app_state());
            let worker = ClipboardWorkerHandle::spawn(&state);
            assert!(crate::test_helpers::wait_until_worker_alive(
                &state,
                Duration::from_secs(3)
            ));

            let generation_before = state.monitor_generation.load(Ordering::SeqCst);
            worker.restart();
            assert!(crate::test_helpers::wait_until_worker_alive(
                &state,
                Duration::from_secs(3)
            ));

            let generation_after = state.monitor_generation.load(Ordering::SeqCst);
            assert!(generation_after > generation_before);
        });
    }

    /// `restart` 完了時点で旧ワーカーの `JoinHandle` が差し替わっているようにすること
    #[test]
    fn restart_replaces_join_handle_after_previous_exit() {
        with_clipboard_worker_spawn_lock(|| {
            let state = Arc::new(test_app_state());
            let worker = ClipboardWorkerHandle::spawn(&state);
            assert!(crate::test_helpers::wait_until_worker_alive(
                &state,
                Duration::from_secs(3)
            ));

            let thread_id_before = {
                let guard = worker.thread.lock_ignore_poison();
                guard
                    .as_ref()
                    .map(|h| h.thread().id())
                    .expect("起動直後は JoinHandle がある")
            };

            worker.restart();
            assert!(crate::test_helpers::wait_until_worker_alive(
                &state,
                Duration::from_secs(3)
            ));

            let thread_id_after = {
                let guard = worker.thread.lock_ignore_poison();
                guard
                    .as_ref()
                    .map(|h| h.thread().id())
                    .expect("再起動後も JoinHandle がある")
            };
            assert_ne!(thread_id_before, thread_id_after);
        });
    }

    /// `Drop` でワーカーが停止すること
    #[test]
    fn drop_shuts_down_worker_thread() {
        with_clipboard_worker_spawn_lock(|| {
            let state = Arc::new(test_app_state());
            let worker = ClipboardWorkerHandle::spawn(&state);
            assert!(crate::test_helpers::wait_until_worker_alive(
                &state,
                Duration::from_secs(3)
            ));

            drop(worker);

            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < deadline {
                if !state.worker().is_alive() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            panic!("Drop 後もワーカーが稼働したまま");
        });
    }
}
