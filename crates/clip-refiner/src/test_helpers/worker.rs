//! クリップボードワーカー復旧フローの統合テスト用ヘルパー

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, test_app_state};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

/// OS クリップボードを開くワーカー起動テストの直列化ロック
///
/// Windows では複数 `OwnedClipboard` の同時生成・破棄がヒープ破壊を起こしうる
static CLIPBOARD_WORKER_SPAWN_LOCK: Mutex<()> = Mutex::new(());

// ======================================================================
// テスト用ヘルパー
// ======================================================================
/// 実ワーカー起動を伴うテストを直列実行する
///
/// # Arguments
/// * `f` - ロック保持中に実行する処理
///
/// # Returns
/// * `T` - `f` の戻り値
pub fn with_clipboard_worker_spawn_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = CLIPBOARD_WORKER_SPAWN_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f()
}

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

/// ワーカーが稼働中かどうか返す
///
/// # Arguments
/// * `state` - ワーカー稼働状態を参照する `AppState`
///
/// # Returns
/// * `bool` - 稼働中なら `true`
pub fn is_worker_alive(state: &AppState) -> bool {
    state.worker().is_alive()
}

/// ワーカー稼働フラグを設定する
///
/// # Arguments
/// * `state` - 更新対象の `AppState`
/// * `alive` - 稼働中なら `true`
pub fn set_worker_alive(state: &AppState, alive: bool) {
    state.worker().set_alive(alive);
}

/// ワーカー復旧待ちフラグを設定する
///
/// # Arguments
/// * `state` - 更新対象の `AppState`
/// * `pending` - 復旧待ちなら `true`
pub fn set_worker_recovery_pending(state: &AppState, pending: bool) {
    state.worker().set_recovery_pending(pending);
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
        if is_worker_alive(state) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}
