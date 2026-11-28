use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::lock_ext::LockExt;

// ======================================================================
// ワーカー稼働状態
// ======================================================================
/// 稼働中のワーカーをコマンド待ちから起こす処理
pub(crate) type WorkerWaker = Box<dyn Fn() + Send + Sync>;

/// クリップボードワーカーの稼働・復旧通知フラグと起床手段
pub(crate) struct WorkerState {
    alive: AtomicBool,
    recovery_pending: AtomicBool,
    /// ワーカースレッドの起動時に登録する起床処理 (未起動なら `None`)
    waker: Mutex<Option<WorkerWaker>>,
}

impl WorkerState {
    /// 停止状態のワーカー状態を生成する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    pub fn new() -> Self {
        Self {
            alive: AtomicBool::new(false),
            recovery_pending: AtomicBool::new(false),
            waker: Mutex::new(None),
        }
    }

    /// テスト用に稼働中として初期化する
    ///
    /// # Returns
    /// * `Self` - 生成したインスタンス
    #[cfg(any(test, feature = "test-helpers", debug_assertions))]
    pub fn new_for_test() -> Self {
        Self {
            alive: AtomicBool::new(true),
            recovery_pending: AtomicBool::new(false),
            waker: Mutex::new(None),
        }
    }

    /// クリップボードワーカーが稼働中かどうか
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - 稼働中の場合は `true`
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// クリップボードワーカーの稼働状態を更新する
    ///
    /// # Arguments
    /// * `alive` - 稼働中にする場合は `true`
    pub fn set_alive(&self, alive: bool) {
        self.alive.store(alive, Ordering::SeqCst);
    }

    /// ワーカー復旧成功時の通知が必要であることを記録する
    ///
    /// # Arguments
    /// * `pending` - 通知が必要な場合は `true`
    pub fn set_recovery_pending(&self, pending: bool) {
        self.recovery_pending.store(pending, Ordering::SeqCst);
    }

    /// ワーカー復旧成功時の通知が必要かどうかを取得し、フラグをクリアする
    ///
    /// # Arguments
    /// * `self` - 自身
    ///
    /// # Returns
    /// * `bool` - 通知が必要だった場合は `true`
    pub fn take_recovery_pending(&self) -> bool {
        self.recovery_pending.swap(false, Ordering::SeqCst)
    }

    /// ワーカーの起床処理を登録する (再起動時は新しいスレッド向けに差し替える)
    ///
    /// # Arguments
    /// * `waker` - 稼働中のワーカーへ起床を送る処理
    pub fn set_waker(&self, waker: WorkerWaker) {
        *self.waker.lock_ignore_poison() = Some(waker);
    }

    /// 稼働中のワーカーをコマンド待ちから起こす (未登録なら何もしない)
    ///
    /// 監視世代や監視周期の変更を、ワーカーの待ち時間の満了を待たずに反映させる
    pub fn wake(&self) {
        if let Some(waker) = self.waker.lock_ignore_poison().as_ref() {
            waker();
        }
    }
}

impl Default for WorkerState {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ワーカー稼働状態と復旧通知フラグを更新できるようにすること
    #[test]
    fn worker_alive_and_recovery_pending() {
        let worker = WorkerState::new_for_test();
        assert!(worker.is_alive());
        worker.set_alive(false);
        assert!(!worker.is_alive());
        worker.set_recovery_pending(true);
        assert!(worker.take_recovery_pending());
        assert!(!worker.take_recovery_pending());
    }

    /// 登録した起床処理を `wake` で呼び、未登録なら何もしないこと
    #[test]
    fn wake_calls_registered_waker() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;

        let worker = WorkerState::new();
        // 未登録では何も起きない
        worker.wake();

        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        worker.set_waker(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        worker.wake();
        worker.wake();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
