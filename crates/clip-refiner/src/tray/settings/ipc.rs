use std::sync::Arc;

use super::model::deep_merge_json_values;

pub(crate) use super::model::SettingsPatch;

use crate::config::{FavoriteMoveDirection, RegexSettings};
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::state::{AppEvent, AppState, RegexPreviewRequest};

use serde::Deserialize;
use tao::event_loop::EventLoopProxy;

// ======================================================================
// IPC メッセージ
// ======================================================================
/// 設定ウィンドウから送られる JSON IPC メッセージ
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum SettingsIpcMessage {
    /// ウィンドウを閉じる
    Close,
    /// 設定ファイルを既定アプリで開く
    OpenConfig,
    /// 設定の一部を適用する
    Apply {
        /// 部分更新パッチ
        patch: Box<SettingsPatch>,
    },
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearRecord,
    /// 登録クリップをクリップボードへコピーする
    ClipCopy {
        /// 登録クリップのインデックス
        index: usize,
    },
    /// 登録クリップを削除する
    ClipDelete {
        /// 登録クリップのインデックス
        index: usize,
    },
    /// クリップボード内容を登録クリップとして追加する
    ClipRegister,
    /// お気に入り変換モードの登録状態を切り替える
    FavoriteToggle {
        /// 対象の変換モード
        mode: RefineMode,
    },
    /// お気に入り変換モードの表示順を変更する
    FavoriteMove {
        /// 対象の変換モード
        mode: RefineMode,
        /// 移動方向 (`up` / `down`)
        direction: IpcFavoriteMoveDirection,
    },
    /// 正規表現置換のプレビューを要求する
    RegexPreview {
        /// クライアント側リクエスト ID
        #[serde(rename = "requestId")]
        request_id: u64,
        /// プレビュー対象の正規表現設定
        settings: RegexSettings,
    },
}

/// IPC 向けお気に入り移動方向
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IpcFavoriteMoveDirection {
    /// 上へ移動
    Up,
    /// 下へ移動
    Down,
}

impl From<IpcFavoriteMoveDirection> for FavoriteMoveDirection {
    fn from(direction: IpcFavoriteMoveDirection) -> Self {
        match direction {
            IpcFavoriteMoveDirection::Up => Self::Up,
            IpcFavoriteMoveDirection::Down => Self::Down,
        }
    }
}

impl From<SettingsIpcMessage> for SettingsIpcAction {
    fn from(message: SettingsIpcMessage) -> Self {
        match message {
            SettingsIpcMessage::Close => Self::Close,
            SettingsIpcMessage::OpenConfig => Self::OpenConfigFile,
            SettingsIpcMessage::Apply { patch } => Self::Apply(patch),
            SettingsIpcMessage::ClearHistory => Self::ClearHistory,
            SettingsIpcMessage::ClearRecord => Self::ClearContinuousRecord,
            SettingsIpcMessage::ClipCopy { index } => Self::ClipCopy(index),
            SettingsIpcMessage::ClipDelete { index } => Self::ClipDelete(index),
            SettingsIpcMessage::ClipRegister => Self::ClipRegister,
            SettingsIpcMessage::FavoriteToggle { mode } => Self::FavoriteToggle(mode),
            SettingsIpcMessage::FavoriteMove { mode, direction } => {
                Self::FavoriteMove(mode, direction.into())
            }
            SettingsIpcMessage::RegexPreview {
                request_id,
                settings,
            } => Self::RegexPreview(request_id, settings),
        }
    }
}

// ======================================================================
// IPC アクション
// ======================================================================
/// 設定ウィンドウから送られる IPC メッセージの種別
#[derive(Debug, Clone)]
pub(crate) enum SettingsIpcAction {
    /// ウィンドウを閉じる
    Close,
    /// 設定ファイルを既定アプリで開く
    OpenConfigFile,
    /// 設定の一部を適用する
    Apply(Box<SettingsPatch>),
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearContinuousRecord,
    /// 登録クリップをクリップボードへコピーする
    ClipCopy(usize),
    /// 登録クリップを削除する
    ClipDelete(usize),
    /// クリップボード内容を登録クリップとして追加する
    ClipRegister,
    /// お気に入り変換モードの登録状態を切り替える
    FavoriteToggle(RefineMode),
    /// お気に入り変換モードの表示順を変更する
    FavoriteMove(RefineMode, FavoriteMoveDirection),
    /// 正規表現置換のプレビューを要求する
    RegexPreview(u64, RegexSettings),
}

/// 適用待ちキューへ部分更新 JSON を原子的にマージして格納する
#[cfg(any(test, feature = "test-helpers", debug_assertions))]
pub(crate) fn enqueue_settings_patch_json(state: &Arc<AppState>, patch_json: &str) -> bool {
    let mut guard = state.pending_settings_patch_mut();
    let merged_json = if let Some(existing) = guard.as_ref() {
        merge_settings_patch_json_strings(existing, patch_json)
            .unwrap_or_else(|| patch_json.to_string())
    } else {
        patch_json.to_string()
    };
    *guard = Some(merged_json);
    true
}

/// 適用待ちキューへ部分更新を原子的にマージして格納する
pub(crate) fn enqueue_settings_patch(state: &Arc<AppState>, patch: &SettingsPatch) -> bool {
    let Ok(incoming_json) = serde_json::to_string(&patch) else {
        return false;
    };

    let mut guard = state.pending_settings_patch_mut();
    let merged_json = if let Some(existing) = guard.as_ref() {
        merge_settings_patch_json_strings(existing, &incoming_json).unwrap_or(incoming_json)
    } else {
        incoming_json
    };
    *guard = Some(merged_json);
    true
}

/// 2 つの部分更新 JSON 文字列を深くマージする
fn merge_settings_patch_json_strings(existing: &str, incoming: &str) -> Option<String> {
    let mut base = serde_json::from_str::<serde_json::Value>(existing).ok()?;
    let incoming_value = serde_json::from_str::<serde_json::Value>(incoming).ok()?;
    deep_merge_json_values(&mut base, &incoming_value);
    serde_json::to_string(&base).ok()
}

/// IPC メッセージ文字列 (JSON) を解釈する
pub(crate) fn parse_settings_ipc_message(msg: &str) -> Option<SettingsIpcAction> {
    serde_json::from_str::<SettingsIpcMessage>(msg)
        .ok()
        .map(SettingsIpcAction::from)
}

/// IPC アクションを `AppEvent` へ変換して送信する
///
/// 設定ウィンドウ内で完結する操作 (`Apply` / データ削除 / 設定ファイル起動) は
/// 専用の `AppEvent` としてイベントループへ送り、`user_event` 側の共通ハンドラへ委譲する
pub(crate) fn dispatch_settings_ipc(
    proxy: &EventLoopProxy<AppEvent>,
    state: &Arc<AppState>,
    action: SettingsIpcAction,
) {
    match action {
        SettingsIpcAction::Close => {
            dispatch::send_app_event(proxy, AppEvent::HideSettingsWindow);
        }
        SettingsIpcAction::OpenConfigFile => {
            dispatch::send_app_event(proxy, AppEvent::SettingsOpenConfigFile);
        }
        SettingsIpcAction::Apply(patch) => {
            if enqueue_settings_patch(state, &patch) {
                dispatch::send_app_event(proxy, AppEvent::SettingsApplyPatch);
            } else {
                crate::log_warn!("設定パッチのキューイングに失敗");
            }
        }
        SettingsIpcAction::ClearHistory => {
            dispatch::send_app_event(proxy, AppEvent::SettingsClearHistory);
        }
        SettingsIpcAction::ClearContinuousRecord => {
            dispatch::send_app_event(proxy, AppEvent::SettingsClearContinuousRecord);
        }
        SettingsIpcAction::ClipCopy(index) => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipCopy(index));
        }
        SettingsIpcAction::ClipDelete(index) => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipDelete(index));
        }
        SettingsIpcAction::ClipRegister => {
            dispatch::send_app_event(proxy, AppEvent::RequestClipRegister);
        }
        SettingsIpcAction::FavoriteToggle(mode) => {
            dispatch::send_app_event(proxy, AppEvent::RequestFavoriteToggle(mode));
        }
        SettingsIpcAction::FavoriteMove(mode, direction) => {
            dispatch::send_app_event(proxy, AppEvent::RequestFavoriteMove(mode, direction));
        }
        SettingsIpcAction::RegexPreview(request_id, settings) => {
            state.set_regex_preview_request(RegexPreviewRequest {
                request_id,
                settings,
            });
            dispatch::send_app_event(proxy, AppEvent::SettingsRegexPreview);
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NotificationSettings;

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
        assert_eq!(regex.pattern, "a");
        assert_eq!(regex.replacement, "b");
        assert!(regex.case_insensitive);
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
            .take_pending_settings_patch()
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
            .take_pending_settings_patch()
            .expect("マージ済みパッチがキューに残ること");
        let patch: SettingsPatch = serde_json::from_str(&json).expect("パッチ JSON のパース");
        assert_eq!(patch.history_enabled, Some(true));
        assert_eq!(patch.continuous_record_enabled, Some(true));
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
            r#"{"type":"regex-preview","requestId":3,"settings":{"pattern":"[0-9]+","replacement":"X","caseInsensitive":false,"multiline":true}}"#,
        );
        let Some(SettingsIpcAction::RegexPreview(request_id, settings)) = action else {
            panic!("RegexPreview アクションとして解釈されること");
        };
        assert_eq!(request_id, 3);
        assert_eq!(settings.pattern, "[0-9]+");
        assert_eq!(settings.replacement, "X");
        assert!(!settings.case_insensitive);
        assert!(settings.multiline);
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
            .take_pending_settings_patch()
            .expect("キューにパッチが格納されること");
        let restored: SettingsPatch =
            serde_json::from_str(&queued).expect("キュー JSON からパッチを復元");
        assert_eq!(restored.interval_ms, Some(2000));
        assert_eq!(restored.is_paused, Some(true));
    }

    /// 旧プレフィックス形式は解釈しないであるようにすること
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
