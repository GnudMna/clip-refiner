//! 設定ウィンドウ IPC とパッチ適用の統合テスト

use clip_refiner::AppConfig;
use clip_refiner::RefineMode;
use clip_refiner::consts;
use clip_refiner::test_helpers::SettingsTestContext;

// ======================================================================
// テスト
// ======================================================================
// ======================================================================
// IPC キュー
// ======================================================================

/// debounce 相当の連続 `apply` IPC がキューでマージされるようにすること
#[test]
fn sequential_apply_ipc_messages_merge_in_queue() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.enqueue_apply_ipc(r#"{"type":"apply","patch":{"historyEnabled":true}}"#));
    assert!(ctx.enqueue_apply_ipc(r#"{"type":"apply","patch":{"continuousRecordEnabled":true}}"#));

    let json = ctx
        .take_pending_patch_json()
        .expect("マージ済みパッチがキューに残ること");
    let patch: serde_json::Value =
        serde_json::from_str(&json).expect("マージ済みパッチ JSON のパース");
    assert_eq!(patch["historyEnabled"], true);
    assert_eq!(patch["continuousRecordEnabled"], true);
}

/// ネスト項目の部分更新が JSON マージで兄弟フィールドを保持できるようにすること
#[test]
fn partial_notification_ipc_merge_preserves_sibling_fields() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.enqueue_apply_ipc(
        r#"{"type":"apply","patch":{"notificationSettings":{"enabled":true,"notifyProcess":false,"notifyResult":false,"notifyPause":true,"notifyOcr":true}}}"#,
    ));
    assert!(ctx.enqueue_patch_json(r#"{"notificationSettings":{"notifyResult":true}}"#,));

    let json = ctx
        .take_pending_patch_json()
        .expect("マージ済みパッチがキューに残ること");
    let patch: serde_json::Value =
        serde_json::from_str(&json).expect("マージ済みパッチ JSON のパース");
    let notification = &patch["notificationSettings"];
    assert_eq!(notification["enabled"], true);
    assert_eq!(notification["notifyProcess"], false);
    assert_eq!(notification["notifyResult"], true);
    assert_eq!(notification["notifyPause"], true);
    assert_eq!(notification["notifyOcr"], true);
}

// ======================================================================
// パッチ適用
// ======================================================================
/// IPC `apply` から設定とトレイメニューへ反映されるようにすること
#[test]
fn apply_ipc_updates_config_and_tray_menu() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"intervalMs":2000,"historyEnabled":true}}"#));

    assert_eq!(ctx.state().with_config(|c| c.interval_ms), 2_000);
    assert!(ctx.state().with_config(|c| c.history_enabled));
    assert!(ctx.menu().history.enabled_item.is_checked());
}

/// 履歴件数パッチが `normalize` で上限へクランプされるようにすること
#[test]
fn apply_ipc_clamps_history_limit() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"historyLimit":999}}"#,));

    assert_eq!(
        ctx.state().with_config(|c| c.history_limit),
        consts::MAX_HISTORY_LIMIT
    );
}

/// パイプライン非空時のデフォルトモード変更が `mode` のみ更新しパイプラインを保持すること
#[test]
fn apply_ipc_updates_default_mode_when_pipeline_active() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"pipeline":["Trim","TrimLines"]}}"#));
    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"mode":"UrlDecode"}}"#));

    assert_eq!(
        ctx.state().with_config(|c| c.pipeline.clone()),
        vec![RefineMode::Trim, RefineMode::TrimLines]
    );
    assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::UrlDecode);
    assert_eq!(
        ctx.state().with_config(AppConfig::effective_pipeline),
        vec![RefineMode::Trim, RefineMode::TrimLines]
    );
}

/// パイプライン変更が設定へ反映されるようにすること
#[test]
fn apply_ipc_updates_pipeline() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"pipeline":["Trim","TrimLines"]}}"#,));

    assert_eq!(
        ctx.state().with_config(|c| c.pipeline.clone()),
        vec![RefineMode::Trim, RefineMode::TrimLines]
    );
    assert_eq!(ctx.state().with_config(|c| c.mode), RefineMode::Trim);
}

/// キューに積んだパッチを取り出して適用できるようにすること
#[test]
fn dequeue_and_apply_merged_patch() {
    let ctx = SettingsTestContext::new();

    ctx.enqueue_apply_ipc(r#"{"type":"apply","patch":{"isPaused":true}}"#);

    assert!(ctx.apply_pending_patch());
    assert!(ctx.state().with_config(|c| c.is_paused));
    assert!(ctx.menu().pause_item.is_checked());
}
