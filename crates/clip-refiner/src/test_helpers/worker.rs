//! クリップボードワーカー復旧フローの統合テスト用ヘルパー

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, test_app_state};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// テスト用ヘルパー
// ======================================================================
/// ユニットテスト用 `AppState` を `Arc` で返す
///
/// # Returns
/// * `Arc<AppState>` - テスト用アプリケーション状態
pub fn test_app_state_arc() -> Arc<AppState> {
    Arc::new(test_app_state())
}

/// コマンド受信のみ行うスタブワーカーを生成する
///
/// # Arguments
/// * `state` - 共有アプリケーション状態
///
/// # Returns
/// * `(Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>)` - スタブワーカーとコマンド受信側
pub fn stub_worker(
    state: &Arc<AppState>,
) -> (Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>) {
    let (tx, rx) = mpsc::channel();
    let worker = ClipboardWorkerHandle::for_test(Arc::clone(state), tx);
    (worker, rx)
}

/// テスト用トレイメニューを構築する
///
/// # Arguments
/// * `state` - 共有アプリケーション状態
///
/// # Returns
/// * `TrayMenu` - 構築されたテスト用トレイメニュー
pub fn test_tray_menu(state: &Arc<AppState>) -> TrayMenu {
    TrayMenu::build(state).expect("テスト用トレイメニューの構築に失敗")
}

/// ワーカーが稼働状態になるまで待機する
///
/// # Arguments
/// * `state` - ワーカー稼働状態を参照する `AppState`
/// * `timeout` - 待機の最大時間
///
/// # Returns
/// * `bool` - タイムアウト内に稼働状態になった場合は `true`
pub fn wait_until_worker_alive(state: &AppState, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if state.is_worker_alive() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
