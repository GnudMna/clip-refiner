use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::tray::state::AppState;

// ======================================================================
// 監視スレッド管理
// ======================================================================
/// クリップボード監視の世代カウンタを進め、ワーカーに監視状態のリセットを通知する
///
/// 監視処理自体はクリップボードワーカースレッド内で実行される
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn bump_monitor_generation(state: &Arc<AppState>) {
    state.monitor_generation.fetch_add(1, Ordering::SeqCst);
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
}
