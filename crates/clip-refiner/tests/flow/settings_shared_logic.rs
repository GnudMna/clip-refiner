//! 設定 UI 共有純粋ロジックのテスト

use clip_refiner::test_helpers::{SettingsTestContext, run_js_test_script};

// ======================================================================
// テスト
// ======================================================================

/// `shared_logic.ts` の純粋ロジックが Node.js 上で期待どおり動作すること
#[test]
fn settings_shared_logic_js_tests() {
    run_js_test_script("settings_shared_logic.ts");
}

/// 通知パッチの Rust 側マージがマスター OFF 時も個別トグルを保持すること
#[test]
fn notification_patch_merge_preserves_sub_toggles_when_master_off() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.enqueue_apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":false,"notifyProcess":true,"notifyAutoRefine":false,"notifyOcr":true}}}"#,
    ));
    assert!(ctx.enqueue_patch_json(
        r#"{"notificationSettings":{"notifyAutoRefine":true,"notifyResult":true}}"#,
    ));

    let json = ctx
        .take_pending_patch_json()
        .expect("マージ済みパッチがキューに残ること");
    let patch: serde_json::Value =
        serde_json::from_str(&json).expect("マージ済みパッチ JSON のパース");
    let notification = &patch["notificationSettings"];
    assert_eq!(notification["enabled"], false);
    assert_eq!(notification["notifyProcess"], true);
    assert_eq!(notification["notifyAutoRefine"], true);
    assert_eq!(notification["notifyOcr"], true);
    assert_eq!(notification["notifyResult"], true);
}
