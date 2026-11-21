//! クリップボードワーカー復旧フローの統合テスト

use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

use clip_refiner::RefineMode;
use clip_refiner::test_helpers::{
    ClipboardCommand, ClipboardWorkerHandle, stub_worker, test_app_state_arc, test_tray_menu,
    wait_until_worker_alive,
};

static SPAWN_TEST_LOCK: Mutex<()> = Mutex::new(());

fn with_spawn_test_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = SPAWN_TEST_LOCK.lock().expect("spawn test lock");
    f()
}

// ======================================================================
// テスト
// ======================================================================
/// ワーカー起動後に `worker_alive` が立つようにすること
#[test]
fn worker_spawn_becomes_alive() {
    with_spawn_test_lock(|| {
        let state = test_app_state_arc();
        let _worker = ClipboardWorkerHandle::spawn(&state);

        assert!(wait_until_worker_alive(&state, Duration::from_secs(3)));
    });
}

/// 停止状態を模倣したあと `restart` でワーカーが復旧すること
#[test]
fn worker_restart_restores_alive_after_simulated_stop() {
    with_spawn_test_lock(|| {
        let state = test_app_state_arc();
        let worker = ClipboardWorkerHandle::spawn(&state);
        assert!(wait_until_worker_alive(&state, Duration::from_secs(3)));

        state.set_worker_alive(false);
        state.set_worker_recovery_pending(true);

        worker.restart();

        assert!(wait_until_worker_alive(&state, Duration::from_secs(3)));
        assert!(state.is_worker_alive());
    });
}

/// 復旧待ちフラグが立っている状態で再起動すると監視世代が進むようにすること
#[test]
fn worker_restart_bumps_monitor_generation_when_recovery_pending() {
    with_spawn_test_lock(|| {
        let state = test_app_state_arc();
        let worker = ClipboardWorkerHandle::spawn(&state);
        assert!(wait_until_worker_alive(&state, Duration::from_secs(3)));

        let generation_before = state.monitor_generation.load(Ordering::SeqCst);
        state.set_worker_alive(false);
        state.set_worker_recovery_pending(true);

        worker.restart();
        assert!(wait_until_worker_alive(&state, Duration::from_secs(3)));

        let generation_after = state.monitor_generation.load(Ordering::SeqCst);
        assert!(generation_after > generation_before);
    });
}

/// ワーカー停止中はスタブ経由のコマンド送信が届かないようにすること
#[test]
fn worker_send_blocked_while_stopped() {
    let state = test_app_state_arc();
    let (stub, rx) = stub_worker(&state);
    state.set_worker_alive(false);

    stub.send(ClipboardCommand::ProcessMode(RefineMode::Trim));

    assert!(rx.try_recv().is_err());
}

/// ワーカー停止時は再開メニューが有効化されるようにすること
#[test]
fn worker_stop_enables_retry_menu_item() {
    let state = test_app_state_arc();
    let menu = test_tray_menu(&state);
    let (worker, _) = stub_worker(&state);
    state.set_worker_alive(false);

    worker.sync_menu_state(&menu);

    assert!(menu.retry_clipboard_worker_item.is_enabled());
}
