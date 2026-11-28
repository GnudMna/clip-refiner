use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::tray::state::AppState;

// ======================================================================
// 監視スレッド管理
// ======================================================================
/// クリップボード監視の世代カウンタを進め、ワーカーに監視状態のリセットを通知する
///
/// 監視処理自体はクリップボードワーカースレッド内で実行される。ワーカーは世代の変化を
/// 待ち時間の合間にしか確認しないため、コマンド待ちから起こして直ちに反映させる
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn bump_monitor_generation(state: &Arc<AppState>) {
    state.monitor_generation.fetch_add(1, Ordering::SeqCst);
    state.worker().wake();
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    /// `bump_monitor_generation` が世代カウンタをインクリメントすること
    #[test]
    fn bump_monitor_generation_increments() {
        let state = Arc::new(crate::tray::state::test_app_state());

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 1);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 2);
    }

    /// 世代を進めるたびにワーカーを起こすこと
    #[test]
    fn bump_monitor_generation_wakes_worker() {
        use std::sync::atomic::AtomicUsize;

        let state = Arc::new(crate::tray::state::test_app_state());
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&wakes);
        state.worker().set_waker(Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        bump_monitor_generation(&state);
        bump_monitor_generation(&state);
        assert_eq!(wakes.load(Ordering::SeqCst), 2);
    }
}
