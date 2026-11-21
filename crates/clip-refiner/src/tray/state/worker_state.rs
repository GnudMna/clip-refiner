use std::sync::atomic::{AtomicBool, Ordering};

// ======================================================================
// ワーカー稼働状態
// ======================================================================
/// クリップボードワーカーの稼働・復旧通知フラグ
pub(crate) struct WorkerState {
    alive: AtomicBool,
    recovery_pending: AtomicBool,
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
}
