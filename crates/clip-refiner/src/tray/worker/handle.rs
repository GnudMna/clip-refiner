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
    pub fn send(&self, command: ClipboardCommand) {
        if !self.state.is_worker_alive() {
            crate::log_warn!("クリップボードワーカーが停止中のため操作を送信できません");
            crate::platform::show_notification(
                "クリップボードエラー",
                "クリップボード監視が停止しています。トレイメニューの「クリップボード監視を再開」を実行してください",
            );
            return;
        }

        let tx = self.tx.lock_ignore_poison();
        if let Err(err) = tx.send(command) {
            crate::log_error!("クリップボードワーカーへのコマンド送信に失敗: {:?}", err);
            self.mark_worker_stopped();
        }
    }

    /// 停止したワーカーを再起動する
    pub fn restart(self: &Arc<Self>) {
        {
            let tx = self.tx.lock_ignore_poison();
            let _ = tx.send(ClipboardCommand::Shutdown);
        }
        thread::sleep(Duration::from_millis(50));

        let (tx, rx) = mpsc::channel();
        *self.tx.lock_ignore_poison() = tx;
        self.state.set_worker_alive(false);
        Self::start_thread(Arc::clone(self), rx);
        bump_monitor_generation(&self.state);
        crate::log_info!("クリップボードワーカーの再起動を要求しました");
    }

    /// トレイメニューの再開項目の有効状態を同期する
    pub fn sync_menu_state(&self, menu: &super::super::menu::TrayMenu) {
        menu.set_clipboard_worker_retry_enabled(!self.state.is_worker_alive());
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
        if self.state.is_worker_alive() {
            self.state.set_worker_alive(false);
            self.state.set_worker_recovery_pending(true);
            dispatch::send_app_event(&self.state.proxy, AppEvent::ClipboardWorkerStopped);
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
impl ClipboardWorkerHandle {
    /// テスト用にチャネル送信のみ行うハンドルを生成する
    pub fn for_test(state: Arc<AppState>, tx: Sender<ClipboardCommand>) -> Arc<Self> {
        state.set_worker_alive(true);
        Arc::new(Self {
            state,
            tx: std::sync::Mutex::new(tx),
        })
    }
}
