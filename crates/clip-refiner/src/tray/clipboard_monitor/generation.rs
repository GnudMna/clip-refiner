use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::tray::state::AppState;

// ======================================================================
// 監視スレッド管理
// ======================================================================
/// クリップボード監視の世代カウンタを進め、ワーカーに監視状態のリセットを通知する
///
/// 監視処理自体はクリップボードワーカースレッド内で実行される
/// 一時停止中 (`is_paused == true`) の場合は何もしない
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn bump_monitor_generation(state: &Arc<AppState>) {
    if state.with_config(|c| c.is_paused) {
        return;
    }

    bump_monitor_generation_unchecked(state);
}

/// 一時停止中でも監視世代を進める
///
/// 連続記録の有効化など、一時停止と独立して同期が必要な場合に使う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
pub fn bump_monitor_generation_unchecked(state: &Arc<AppState>) {
    state.monitor_generation.fetch_add(1, Ordering::SeqCst);
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    /// 一時停止中は `bump_monitor_generation` が世代を進めないであるようにすること
    #[test]
    fn bump_monitor_generation_skips_when_paused() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.is_paused = true);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 0);
    }

    /// 監視中は `bump_monitor_generation` が世代カウンタをインクリメントすること
    #[test]
    fn bump_monitor_generation_increments_when_active() {
        let state = Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.is_paused = false);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 1);

        bump_monitor_generation(&state);
        assert_eq!(state.monitor_generation.load(Ordering::SeqCst), 2);
    }
}
