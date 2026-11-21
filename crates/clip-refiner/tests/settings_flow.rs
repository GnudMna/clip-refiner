//! 設定ウィンドウ IPC とパッチ適用の統合テスト

use clip_refiner::AppConfig;
use clip_refiner::MonitorMode;
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

/// プリセット外の監視周期を数値入力相当のパッチで反映すること
#[test]
fn apply_ipc_accepts_custom_interval_ms() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"intervalMs":750}}"#));

    assert_eq!(ctx.state().with_config(|c| c.interval_ms), 750);
    assert!(ctx.menu().interval.custom_item.is_checked());
    assert!(
        ctx.menu()
            .interval
            .items
            .iter()
            .all(|(item, _)| !item.is_checked())
    );
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

/// Event 監視への切替と同時の周期パッチでは周期をスキップすること
#[test]
fn apply_ipc_skips_interval_when_switching_to_event() {
    let ctx = SettingsTestContext::new();
    ctx.state()
        .with_config_mut(|c| c.monitor_mode = MonitorMode::Polling);
    let default_interval = ctx.state().with_config(|c| c.interval_ms);

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"monitorMode":"Event","intervalMs":500}}"#,));

    assert_eq!(
        ctx.state().with_config(|c| c.monitor_mode),
        MonitorMode::Event
    );
    assert_eq!(ctx.state().with_config(|c| c.interval_ms), default_interval);
}

/// 有効な正規表現パッチが設定へ反映されるようにすること
#[test]
fn apply_ipc_updates_regex_settings() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"regex":{"pattern":"\\d+","replacement":"N","caseInsensitive":true,"multiline":false}}}"#,
    ));

    let regex = ctx.state().with_config(|c| c.regex.clone());
    assert_eq!(regex.pattern, r"\d+");
    assert_eq!(regex.replacement, "N");
    assert!(regex.case_insensitive);
    assert!(!regex.multiline);
}

/// ホットキーパッチが設定へ反映され不正値が正規化されるようにすること
#[test]
fn apply_ipc_updates_hotkeys_and_fixes_invalid() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"hotkeys":{"quickSelector":"Ctrl+Shift+S","pause":"Bad+Key","favoriteModeSlots":[""]}}}"#,
    ));

    let hotkeys = ctx.state().with_config(|c| c.hotkeys.clone());
    assert_eq!(hotkeys.quick_selector, "Ctrl+Shift+S");
    assert_eq!(hotkeys.pause, consts::DEFAULT_HOTKEY_PAUSE);
    assert_eq!(hotkeys.favorite_mode_slots, vec![String::new()]);
}

/// 無効な正規表現パッチは設定を更新しないこと
#[test]
fn apply_ipc_rejects_invalid_regex() {
    let ctx = SettingsTestContext::new();
    let before = ctx.state().with_config(|c| c.regex.clone());

    assert!(ctx.apply_ipc(
        r#"{"type":"apply","patch":{"regex":{"pattern":"[unclosed","replacement":"","caseInsensitive":false,"multiline":false}}}"#,
    ));

    let after = ctx.state().with_config(|c| c.regex.clone());
    assert_eq!(after.pattern, before.pattern);
    assert_eq!(after.replacement, before.replacement);
}

/// 連続記録の有効化が設定とトレイメニューへ反映されるようにすること
#[test]
fn apply_ipc_enables_continuous_record() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"continuousRecordEnabled":true}}"#));

    assert!(ctx.state().with_config(|c| c.continuous_record_enabled));
    assert!(ctx.menu().continuous_record.enabled_item.is_checked());
}

/// 一時停止パッチが設定とトレイメニューへ反映されるようにすること
#[test]
fn apply_ipc_pauses_monitoring() {
    let ctx = SettingsTestContext::new();

    assert!(ctx.apply_ipc(r#"{"type":"apply","patch":{"isPaused":true}}"#));

    assert!(ctx.state().with_config(|c| c.is_paused));
    assert!(ctx.menu().pause_item.is_checked());
}

/// IPC `apply` 後に `config.toml` へ保存され再読込できること
#[test]
fn apply_ipc_persists_settings_to_config_toml() {
    SettingsTestContext::with_persistence(|ctx| {
        assert!(ctx.apply_ipc(
            r#"{"type":"apply","patch":{"intervalMs":750,"historyEnabled":true,"hotkeys":{"quickSelector":"Ctrl+Shift+S","favoriteModeSlots":[""]}}}"#,
        ));

        assert!(ctx.is_config_persistence_enabled());
        assert_eq!(ctx.state().with_config(|c| c.interval_ms), 750);
        assert!(ctx.state().with_config(|c| c.history_enabled));

        let loaded = AppConfig::load();
        assert!(loaded.fallback.is_none());
        assert_eq!(loaded.config.interval_ms, 750);
        assert!(loaded.config.history_enabled);
        assert_eq!(loaded.config.hotkeys.quick_selector, "Ctrl+Shift+S");
        assert_eq!(
            loaded.config.hotkeys.favorite_mode_slots,
            vec![String::new()]
        );
    });
}
