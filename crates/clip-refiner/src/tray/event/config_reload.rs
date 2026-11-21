use std::sync::Arc;

use thiserror::Error;

use super::super::clipboard_monitor::bump_monitor_generation;
use super::super::hotkey::{HotkeyHandler, format_registration_failures};
use super::super::menu::TrayMenu;
use super::super::state::AppState;
use crate::config::{
    AppConfig, ConfigReloadError, format_unknown_config_keys_message,
    format_unsupported_config_version_message,
};
use crate::platform;
use crate::tray::selector::{ClipSelectorWindow, clip_selector_json};
use crate::user_error::{UserNotification, titles};

// ======================================================================
// 設定再読み込み
// ======================================================================
/// 設定再読み込みの結果
pub struct ConfigReloadOutcome {
    /// ユーザー向けメッセージ
    pub message: String,
}

/// 設定再読み込みの適用失敗
///
/// ユーザー向け文言は `Display` (`#[error(...)]`) に集約し、通知へは `to_string()` で渡す
#[derive(Debug, Error)]
pub(crate) enum ApplyConfigReloadError {
    /// ディスク上の設定読み込み失敗
    #[error("{}", .0.notification_message())]
    Disk(#[from] ConfigReloadError),
    /// トレイメニュー同期失敗
    #[error("メニュー同期に失敗: {0}")]
    MenuSync(String),
    /// ホットキー再登録失敗
    #[error("ホットキーの再登録に失敗: {0}")]
    HotkeyReload(String),
    /// 履歴メニュー更新失敗
    #[error("履歴メニュー更新に失敗: {0}")]
    HistoryMenu(String),
    /// 連続記録メニュー更新失敗
    #[error("連続記録メニュー更新に失敗: {0}")]
    ContinuousRecordMenu(String),
}

impl From<&ApplyConfigReloadError> for UserNotification {
    fn from(err: &ApplyConfigReloadError) -> Self {
        Self::new(titles::SETTINGS, err.to_string())
    }
}

/// ディスク上の設定を読み込み、アプリ状態と UI へ反映する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `hotkey_handler` - グローバルホットキーハンドラ
/// * `clip_selector` - 登録クリップセレクター (表示中なら内容を更新)
///
/// # Returns
/// * `Result<ConfigReloadOutcome, ApplyConfigReloadError>` - 成功時は結果メッセージ、失敗時は型付きエラー
pub fn apply_config_reload(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    hotkey_handler: &mut HotkeyHandler,
    clip_selector: &ClipSelectorWindow,
) -> Result<ConfigReloadOutcome, ApplyConfigReloadError> {
    let loaded = AppConfig::reload_from_disk()?;
    let unknown_keys = loaded.unknown_keys.clone();
    let migrated = loaded.migrated;
    let newer_version_clamped = loaded.newer_version_clamped;
    let unsupported_version_kept = loaded.unsupported_version_kept;
    let loaded = loaded.config;

    let previous = state.with_config(std::clone::Clone::clone);
    let hotkeys_changed = previous.hotkeys != loaded.hotkeys;
    let favorite_hotkeys_changed = previous.hotkeys.favorite_mode_slots
        != loaded.hotkeys.favorite_mode_slots
        || previous.favorite_modes != loaded.favorite_modes;
    let monitor_changed = previous.monitor_mode != loaded.monitor_mode
        || previous.interval_ms != loaded.interval_ms
        || previous.is_paused != loaded.is_paused
        || previous.auto_refine_enabled != loaded.auto_refine_enabled
        || previous.effective_pipeline() != loaded.effective_pipeline()
        || previous.continuous_record_enabled != loaded.continuous_record_enabled;
    let history_disabled = previous.history_enabled && !loaded.history_enabled;
    let continuous_record_disabled =
        previous.continuous_record_enabled && !loaded.continuous_record_enabled;

    state.with_config_mut(|config| *config = loaded);
    state.config_sync().record_disk_sync();

    menu.sync_from_config(state)
        .map_err(|e| ApplyConfigReloadError::MenuSync(e.to_string()))?;

    let mut change_labels = Vec::new();
    let mut hotkey_registration_message = None;

    if hotkeys_changed || favorite_hotkeys_changed {
        let hotkeys = state.with_config(|c| c.hotkeys.clone());
        let favorite_modes = state.with_config(|c| c.favorite_modes.clone());
        let registration_failures = hotkey_handler
            .reload(&hotkeys, &favorite_modes)
            .map_err(|e| ApplyConfigReloadError::HotkeyReload(e.to_string()))?;
        change_labels.push("ホットキーを更新");
        if !registration_failures.is_empty() {
            change_labels.push("一部ホットキーの登録に失敗");
            let registration_message = format_registration_failures(&registration_failures);
            crate::log_warn!("{registration_message}");
            hotkey_registration_message = Some(registration_message);
        }
    }

    if history_disabled {
        state.clipboard_session().clear_history();
        menu.refresh_history(state)
            .map_err(|e| ApplyConfigReloadError::HistoryMenu(e.to_string()))?;
    }

    if continuous_record_disabled {
        state.clipboard_session().clear_continuous_record();
        menu.refresh_continuous_record(state)
            .map_err(|e| ApplyConfigReloadError::ContinuousRecordMenu(e.to_string()))?;
    }

    if monitor_changed {
        bump_monitor_generation(state);
    }

    if clip_selector.is_visible() {
        let clips_json = state.with_config(clip_selector_json);
        clip_selector.refresh_items(&clips_json);
    }

    if monitor_changed {
        change_labels.push("監視設定を更新");
    }
    if migrated {
        change_labels.push("設定スキーマを移行");
    }
    if newer_version_clamped {
        change_labels.push("将来 version を現行へ揃え");
    }
    if unsupported_version_kept {
        change_labels.push("未対応 version を検出し既存項目を維持");
    }
    if !unknown_keys.is_empty() {
        change_labels.push("未知の設定キーを検出");
    }

    let mut message = build_reload_message(&change_labels);
    if unsupported_version_kept {
        message.push('\n');
        message.push_str(format_unsupported_config_version_message());
    }
    if !unknown_keys.is_empty() {
        message.push('\n');
        message.push_str(&format_unknown_config_keys_message(&unknown_keys));
    }
    if let Some(registration_message) = hotkey_registration_message {
        message.push('\n');
        message.push_str(&registration_message);
    }
    crate::log_info!("設定を再読み込みしました: {}", message);
    Ok(ConfigReloadOutcome { message })
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 再読み込み成功時の通知メッセージを組み立てる
fn build_reload_message(extra_parts: &[&str]) -> String {
    let mut parts = vec!["設定を反映しました".to_string()];
    parts.extend(extra_parts.iter().map(|label| (*label).to_string()));
    parts.join(" / ")
}

/// 設定再読み込みを実行し、結果をデスクトップ通知で表示する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `hotkey_handler` - グローバルホットキーハンドラ
/// * `clip_selector` - 登録クリップセレクター (表示中なら内容を更新)
pub fn reload_config_with_notification(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    hotkey_handler: &mut HotkeyHandler,
    clip_selector: &ClipSelectorWindow,
) {
    match apply_config_reload(state, menu, hotkey_handler, clip_selector) {
        Ok(outcome) => platform::show_notification("設定を再読み込み", &outcome.message),
        Err(err) => {
            // 再読み込み失敗は警告レベル。タイトルは成功時と同系統を維持する
            crate::log_warn!("設定の再読み込みに失敗: {err}");
            platform::show_notification("設定の再読み込み", &err.to_string());
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 再読み込み成功メッセージが変更内容を反映すること
    #[test]
    fn build_reload_message_lists_changes() {
        let message = build_reload_message(&[
            "ホットキーを更新",
            "監視設定を更新",
            "設定スキーマを移行",
            "将来 version を現行へ揃え",
            "未対応 version を検出し既存項目を維持",
            "未知の設定キーを検出",
        ]);
        assert!(message.contains("設定を反映しました"));
        assert!(message.contains("ホットキーを更新"));
        assert!(message.contains("監視設定を更新"));
        assert!(message.contains("設定スキーマを移行"));
        assert!(message.contains("将来 version を現行へ揃え"));
        assert!(message.contains("未対応 version を検出し既存項目を維持"));
        assert!(message.contains("未知の設定キーを検出"));
    }

    /// 変更がない場合は基本メッセージのみとなるようにすること
    #[test]
    fn build_reload_message_without_changes() {
        assert_eq!(build_reload_message(&[]), "設定を反映しました");
    }

    /// ディスク読み込み失敗が通知向け文言を保持すること
    #[test]
    fn apply_config_reload_error_disk_uses_notification_message() {
        let err = ApplyConfigReloadError::from(ConfigReloadError::Parse("bad key".into()));
        assert!(err.to_string().contains("形式が不正"));
        assert!(err.to_string().contains("bad key"));

        let notification = UserNotification::from(&err);
        assert_eq!(notification.title(), titles::SETTINGS);
        assert_eq!(notification.body(), err.to_string());
    }

    /// UI 反映失敗が接頭辞付きの `Display` になるようにすること
    #[test]
    fn apply_config_reload_error_menu_sync_prefix() {
        let err = ApplyConfigReloadError::MenuSync("broken".into());
        assert_eq!(err.to_string(), "メニュー同期に失敗: broken");
    }
}
