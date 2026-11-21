//! 設定ウィンドウからの JSON IPC 解釈・キュー投入・イベント送信

mod dispatch;
mod message;
mod queue;

pub(crate) use super::model::SettingsPatch;
pub(crate) use dispatch::dispatch_settings_ipc;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use message::SettingsIpcAction;
pub(crate) use message::parse_settings_ipc_message;
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) use queue::{enqueue_settings_patch, enqueue_settings_patch_json};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::queue::merge_settings_patch_json_strings;
    use super::{
        SettingsIpcAction, SettingsPatch, enqueue_settings_patch, enqueue_settings_patch_json,
        parse_settings_ipc_message,
    };
    use crate::config::{FavoriteMoveDirection, NotificationSettings};
    use crate::refiner::RefineMode;

    /// JSON エンベロープ形式の `apply` を解釈できるようにすること
    #[test]
    fn parse_ipc_apply_patch() {
        let action = parse_settings_ipc_message(
            r#"{"type":"apply","patch":{"notificationSettings":{"enabled":true,"notifyProcess":false,"notifyResult":true,"notifyPause":true,"notifyOcr":false}}}"#,
        );
        let Some(SettingsIpcAction::Apply(patch)) = action else {
            panic!("Apply アクションとして解釈されること");
        };
        let notification = patch
            .notification_settings
            .expect("notification_settings が含まれること");
        assert!(notification.enabled);
        assert!(!notification.notify_process);
        assert!(notification.notify_result);
        assert!(notification.notify_pause);
        assert!(!notification.notify_ocr);
    }

    /// `snake_case` 別名でも IPC パッチを解釈できるようにすること
    #[test]
    fn parse_ipc_apply_patch_nested_snake_case_alias() {
        let action = parse_settings_ipc_message(
            r#"{"type":"apply","patch":{"notificationSettings":{"enabled":true,"notify_process":false,"notify_result":true,"notify_pause":true,"notify_ocr":false}}}"#,
        );
        let Some(SettingsIpcAction::Apply(patch)) = action else {
            panic!("Apply アクションとして解釈されること");
        };
        let notification = patch
            .notification_settings
            .expect("notification_settings が含まれること");
        assert!(notification.enabled);
        assert!(!notification.notify_process);
        assert!(notification.notify_result);
        assert!(notification.notify_pause);
        assert!(!notification.notify_ocr);
    }

    /// ネスト項目の camelCase でも解釈できるようにすること
    #[test]
    fn parse_ipc_apply_patch_nested_camel_case_alias() {
        let action = parse_settings_ipc_message(
            r#"{"type":"apply","patch":{"regex":{"pattern":"a","replacement":"b","caseInsensitive":true}}}"#,
        );
        let Some(SettingsIpcAction::Apply(patch)) = action else {
            panic!("Apply アクションとして解釈されること");
        };
        let regex = patch.regex.expect("regex が含まれること");
        let step = &regex.active_steps()[0];
        assert_eq!(step.pattern, "a");
        assert_eq!(step.replacement, "b");
        assert!(step.case_insensitive);
    }

    /// `SettingsPatch::merge_from` で複数フィールドを統合できるようにすること
    #[test]
    fn merge_settings_patch_combines_fields() {
        let mut base = SettingsPatch {
            history_enabled: Some(true),
            ..SettingsPatch::default()
        };
        base.merge_from(&SettingsPatch {
            continuous_record_enabled: Some(true),
            ..SettingsPatch::default()
        });
        assert_eq!(base.history_enabled, Some(true));
        assert_eq!(base.continuous_record_enabled, Some(true));
    }

    /// `merge_from` でネスト項目を深くマージできるようにすること
    #[test]
    fn merge_settings_patch_deep_merges_nested_fields() {
        let mut base_settings = NotificationSettings::default();
        base_settings.enabled = true;
        base_settings.notify_process = false;
        base_settings.notify_result = false;
        base_settings.notify_pause = true;
        base_settings.notify_ocr = true;
        let mut merged_settings = NotificationSettings::default();
        merged_settings.enabled = false;
        merged_settings.notify_process = true;
        merged_settings.notify_result = true;
        merged_settings.notify_pause = false;
        merged_settings.notify_ocr = false;
        let mut base = SettingsPatch {
            notification_settings: Some(base_settings),
            ..SettingsPatch::default()
        };
        base.merge_from(&SettingsPatch {
            notification_settings: Some(merged_settings),
            ..SettingsPatch::default()
        });
        let notification = base
            .notification_settings
            .expect("notification_settings が保持されること");
        assert!(!notification.enabled);
        assert!(notification.notify_process);
        assert!(notification.notify_result);
        assert!(!notification.notify_pause);
        assert!(!notification.notify_ocr);
    }

    /// JSON 文字列マージでネスト項目の一部だけを更新できるようにすること
    #[test]
    fn merge_settings_patch_json_strings_preserves_sibling_nested_fields() {
        let existing = r#"{"notificationSettings":{"enabled":true,"notifyProcess":false,"notifyResult":false,"notifyPause":true,"notifyOcr":true}}"#;
        let incoming = r#"{"notificationSettings":{"notifyResult":true}}"#;
        let merged = merge_settings_patch_json_strings(existing, incoming)
            .expect("マージ後 JSON が生成されること");
        let value: serde_json::Value =
            serde_json::from_str(&merged).expect("マージ後 JSON のパース");
        assert_eq!(value["notificationSettings"]["enabled"], true);
        assert_eq!(value["notificationSettings"]["notifyProcess"], false);
        assert_eq!(value["notificationSettings"]["notifyResult"], true);
        assert_eq!(value["notificationSettings"]["notifyPause"], true);
        assert_eq!(value["notificationSettings"]["notifyOcr"], true);
    }

    /// `favorite-move` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_favorite_move() {
        let action = parse_settings_ipc_message(
            r#"{"type":"favorite-move","mode":"Trim","direction":"up"}"#,
        );
        assert!(matches!(
            action,
            Some(SettingsIpcAction::FavoriteMove(
                RefineMode::Trim,
                FavoriteMoveDirection::Up
            ))
        ));
    }

    /// `open-config` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_open_config() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"open-config"}"#),
            Some(SettingsIpcAction::OpenConfigFile)
        ));
    }

    /// `enqueue_settings_patch_json` で部分 JSON を深くマージできるようにすること
    #[test]
    fn enqueue_settings_patch_json_deep_merges_nested_fields() {
        let state = Arc::new(crate::tray::state::test_app_state());

        let mut notification_settings = NotificationSettings::default();
        notification_settings.enabled = true;
        notification_settings.notify_process = false;
        notification_settings.notify_result = false;
        notification_settings.notify_pause = true;
        enqueue_settings_patch(
            &state,
            &SettingsPatch {
                notification_settings: Some(notification_settings),
                ..SettingsPatch::default()
            },
        );
        assert!(enqueue_settings_patch_json(
            &state,
            r#"{"notificationSettings":{"notifyResult":true}}"#,
        ));

        let json = state
            .settings_pending()
            .take_patch()
            .expect("マージ済みパッチがキューに残ること");
        let value: serde_json::Value = serde_json::from_str(&json).expect("マージ後 JSON のパース");
        assert_eq!(value["notificationSettings"]["enabled"], true);
        assert_eq!(value["notificationSettings"]["notifyResult"], true);
        assert_eq!(value["notificationSettings"]["notifyPause"], true);
    }

    /// `clip-copy` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_clip_copy() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"clip-copy","index":2}"#),
            Some(SettingsIpcAction::ClipCopy(2))
        ));
    }

    /// `enqueue_settings_patch` で連続投入したパッチがマージされるようにすること
    #[test]
    fn enqueue_settings_patch_merges_sequential_applies() {
        let state = Arc::new(crate::tray::state::test_app_state());

        assert!(enqueue_settings_patch(
            &state,
            &SettingsPatch {
                history_enabled: Some(true),
                ..SettingsPatch::default()
            }
        ));
        assert!(enqueue_settings_patch(
            &state,
            &SettingsPatch {
                continuous_record_enabled: Some(true),
                ..SettingsPatch::default()
            }
        ));

        let json = state
            .settings_pending()
            .take_patch()
            .expect("マージ済みパッチがキューに残ること");
        let patch: SettingsPatch = serde_json::from_str(&json).expect("パッチ JSON のパース");
        assert_eq!(patch.history_enabled, Some(true));
        assert_eq!(patch.continuous_record_enabled, Some(true));
    }

    /// マージ失敗時は既存キューを保持すること
    #[test]
    fn enqueue_settings_patch_json_preserves_queue_on_merge_failure() {
        let state = Arc::new(crate::tray::state::test_app_state());

        assert!(enqueue_settings_patch(
            &state,
            &SettingsPatch {
                history_enabled: Some(true),
                ..SettingsPatch::default()
            }
        ));
        assert!(!enqueue_settings_patch_json(&state, "not-json"));

        let json = state
            .settings_pending()
            .take_patch()
            .expect("既存キューが残ること");
        let patch: SettingsPatch = serde_json::from_str(&json).expect("パッチ JSON のパース");
        assert_eq!(patch.history_enabled, Some(true));
        assert_eq!(patch.continuous_record_enabled, None);
    }

    /// 不正な単体 JSON はキューへ入れないようにすること
    #[test]
    fn enqueue_settings_patch_json_rejects_invalid_standalone_payload() {
        let state = Arc::new(crate::tray::state::test_app_state());
        assert!(!enqueue_settings_patch_json(&state, "not-json"));
        assert!(!enqueue_settings_patch_json(&state, r#""string""#));
        assert!(state.settings_pending().take_patch().is_none());
    }

    /// `close` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_close() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"close"}"#),
            Some(SettingsIpcAction::Close)
        ));
    }

    /// `clear-history` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_clear_history() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"clear-history"}"#),
            Some(SettingsIpcAction::ClearHistory)
        ));
    }

    /// `clear-record` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_clear_record() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"clear-record"}"#),
            Some(SettingsIpcAction::ClearContinuousRecord)
        ));
    }

    /// `clip-delete` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_clip_delete() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"clip-delete","index":1}"#),
            Some(SettingsIpcAction::ClipDelete(1))
        ));
    }

    /// `clip-register` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_clip_register() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"clip-register"}"#),
            Some(SettingsIpcAction::ClipRegister)
        ));
    }

    /// `favorite-toggle` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_favorite_toggle() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"favorite-toggle","mode":"Trim"}"#),
            Some(SettingsIpcAction::FavoriteToggle(RefineMode::Trim))
        ));
    }

    /// `regex-preview` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_regex_preview() {
        let action = parse_settings_ipc_message(
            r#"{"type":"regex-preview","requestId":3,"settings":{"pattern":"[0-9]+","replacement":"X","caseInsensitive":false,"multiline":true},"sample":"abc 42"}"#,
        );
        let Some(SettingsIpcAction::RegexPreview(request_id, settings, sample)) = action else {
            panic!("RegexPreview アクションとして解釈されること");
        };
        assert_eq!(request_id, 3);
        let step = &settings.active_steps()[0];
        assert_eq!(step.pattern, "[0-9]+");
        assert_eq!(step.replacement, "X");
        assert!(!step.case_insensitive);
        assert!(step.multiline);
        assert_eq!(sample, "abc 42");
    }

    /// `regex-preview` で sample 省略時は空文字になるようにすること
    #[test]
    fn parse_ipc_regex_preview_without_sample() {
        let action = parse_settings_ipc_message(
            r#"{"type":"regex-preview","requestId":4,"settings":{"pattern":"a","replacement":"","caseInsensitive":false,"multiline":false}}"#,
        );
        let Some(SettingsIpcAction::RegexPreview(_, _, sample)) = action else {
            panic!("RegexPreview アクションとして解釈されること");
        };
        assert!(sample.is_empty());
    }

    /// `check-update` / `apply-update` メッセージを解釈できるようにすること
    #[test]
    fn parse_ipc_update_messages() {
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"check-update"}"#),
            Some(SettingsIpcAction::CheckUpdate)
        ));
        assert!(matches!(
            parse_settings_ipc_message(r#"{"type":"apply-update"}"#),
            Some(SettingsIpcAction::ApplyUpdate)
        ));
    }

    /// JS から送る `apply` JSON をキュー投入から取り出しまで往復できるようにすること
    #[test]
    fn ipc_apply_round_trip_through_queue() {
        let state = Arc::new(crate::tray::state::test_app_state());
        let json = r#"{"type":"apply","patch":{"intervalMs":2000,"isPaused":true}}"#;
        let action = parse_settings_ipc_message(json).expect("IPC JSON の解釈");
        let SettingsIpcAction::Apply(patch) = action else {
            panic!("Apply アクションとして解釈されること");
        };

        assert!(enqueue_settings_patch(&state, &patch));
        let queued = state
            .settings_pending()
            .take_patch()
            .expect("キューにパッチが格納されること");
        let restored: SettingsPatch =
            serde_json::from_str(&queued).expect("キュー JSON からパッチを復元");
        assert_eq!(restored.interval_ms, Some(2000));
        assert_eq!(restored.is_paused, Some(true));
    }

    /// 旧プレフィックス形式は解釈しないようにすること
    #[test]
    fn parse_ipc_legacy_prefix_returns_none() {
        assert!(parse_settings_ipc_message("close").is_none());
        assert!(parse_settings_ipc_message("apply:{}").is_none());
    }

    /// 不正な JSON は `None` を返すようにすること
    #[test]
    fn parse_ipc_unknown_returns_none() {
        assert!(parse_settings_ipc_message("invalid").is_none());
        assert!(parse_settings_ipc_message(r#"{"type":"unknown"}"#).is_none());
    }
}
