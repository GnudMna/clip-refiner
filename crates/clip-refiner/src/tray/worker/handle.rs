use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use super::command::ClipboardCommand;
use super::run_worker_loop;

use super::super::dispatch;
use super::super::state::{AppEvent, AppState, LockExt};
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::user_error::{UserNotification, titles};

// ======================================================================
// ワーカーハンドル
// ======================================================================
/// クリップボードワーカーへのコマンド送信と再起動を管理する
pub struct ClipboardWorkerHandle {
    state: Arc<AppState>,
    tx: std::sync::Mutex<Sender<ClipboardCommand>>,
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
            tx: std::sync::Mutex::new(tx),
        });
        Self::start_thread(Arc::clone(&handle), rx);
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
            self.mark_worker_stopped();
        }
    }

    /// 停止したワーカーを再起動する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardWorkerHandle`
    pub fn restart(self: &Arc<Self>) {
        {
            let tx = self.tx.lock_ignore_poison();
            let _ = tx.send(ClipboardCommand::Shutdown);
        }
        thread::sleep(Duration::from_millis(50));

        let (tx, rx) = mpsc::channel();
        *self.tx.lock_ignore_poison() = tx;
        self.state.worker().set_alive(false);
        Self::start_thread(Arc::clone(self), rx);
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

    fn start_thread(handle: Arc<Self>, rx: Receiver<ClipboardCommand>) {
        let state = Arc::clone(&handle.state);
        thread::spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| run_worker_loop(&state, &rx)));
            if result.is_err() {
                crate::log_error!("クリップボードワーカーがパニックで終了しました");
            }
            handle.mark_worker_stopped();
        });
    }

    fn mark_worker_stopped(&self) {
        if self.state.worker().is_alive() {
            self.state.worker().set_alive(false);
            self.state.worker().set_recovery_pending(true);
            dispatch::send_app_event(&self.state.proxy, AppEvent::ClipboardWorkerStopped);
        }
    }
}

impl Drop for ClipboardWorkerHandle {
    fn drop(&mut self) {
        let tx = self.tx.lock_ignore_poison();
        let _ = tx.send(ClipboardCommand::Shutdown);
    }
}

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
            tx: std::sync::Mutex::new(tx),
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
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    use crate::refiner::RefineMode;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::test_app_state;

    fn with_stub_worker(
        state: &Arc<AppState>,
    ) -> (Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>) {
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(Arc::clone(state), tx);
        (worker, rx)
    }

    /// ワーカー停止中はコマンドを送信しないであるようにすること
    #[test]
    fn send_skips_command_when_worker_stopped() {
        let state = Arc::new(test_app_state());
        let (worker, rx) = with_stub_worker(&state);
        state.worker().set_alive(false);

        worker.send(ClipboardCommand::ProcessMode(RefineMode::Trim));

        assert!(rx.try_recv().is_err());
    }

    /// ワーカー停止時は再開メニューを有効化すること
    #[test]
    fn sync_menu_state_enables_retry_when_worker_stopped() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, _) = with_stub_worker(&state);
        state.worker().set_alive(false);

        worker.sync_menu_state(&menu);

        assert!(menu.retry_clipboard_worker_item.is_enabled());
    }

    /// ワーカー稼働中は再開メニューを無効化すること
    #[test]
    fn sync_menu_state_disables_retry_when_worker_alive() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, _) = with_stub_worker(&state);
        state.worker().set_alive(true);

        worker.sync_menu_state(&menu);

        assert!(!menu.retry_clipboard_worker_item.is_enabled());
    }

    /// `restart` で監視世代が進むとすること
    #[test]
    fn restart_bumps_monitor_generation() {
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
    }
}
