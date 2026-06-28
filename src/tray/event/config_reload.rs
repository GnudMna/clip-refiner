use std::sync::Arc;

use super::super::clipboard_monitor::bump_monitor_generation;
use super::super::hotkey::HotkeyHandler;
use super::super::menu::TrayMenu;
use super::super::state::AppState;
use super::super::text_selector::TextSelectorWindow;

use crate::config::{AppConfig, ConfigReloadError};
use crate::platform;

// ======================================================================
// 設定再読み込み
// ======================================================================
/// 設定再読み込みの結果
pub struct ConfigReloadOutcome {
    /// ユーザー向けメッセージ
    pub message: String,
}

/// ディスク上の設定を読み込み、アプリ状態と UI へ反映する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `hotkey_handler` - グローバルホットキーハンドラ
/// * `text_selector` - 登録文字列セレクター (表示中なら内容を更新)
///
/// # Returns
/// * `Result<ConfigReloadOutcome, String>` - 成功時は結果メッセージ、失敗時はエラー文言
pub fn apply_config_reload(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    hotkey_handler: &mut HotkeyHandler,
    text_selector: &TextSelectorWindow,
) -> Result<ConfigReloadOutcome, String> {
    let (loaded, migrated) = AppConfig::reload_from_disk().map_err(|e| reload_error_message(&e))?;

    let previous = state.with_config(std::clone::Clone::clone);
    let hotkeys_changed = previous.hotkeys != loaded.hotkeys;
    let favorite_hotkeys_changed = previous.hotkeys.favorite_mode_slots
        != loaded.hotkeys.favorite_mode_slots
        || previous.favorite_modes != loaded.favorite_modes;
    let monitor_changed = previous.monitor_mode != loaded.monitor_mode
        || previous.interval_ms != loaded.interval_ms
        || previous.is_paused != loaded.is_paused
        || previous.effective_pipeline() != loaded.effective_pipeline();
    let history_disabled = previous.history_enabled && !loaded.history_enabled;

    state.with_config_mut(|config| *config = loaded);
    state.record_config_disk_sync();

    menu.sync_from_config(state)
        .map_err(|e| format!("メニュー同期に失敗: {e}"))?;

    if hotkeys_changed || favorite_hotkeys_changed {
        let hotkeys = state.with_config(|c| c.hotkeys.clone());
        let favorite_modes = state.with_config(|c| c.favorite_modes.clone());
        hotkey_handler
            .reload(&hotkeys, &favorite_modes)
            .map_err(|e| format!("ホットキーの再登録に失敗: {e}"))?;
    }

    if history_disabled {
        state.clear_history();
        menu.refresh_history(state)
            .map_err(|e| format!("履歴メニュー更新に失敗: {e}"))?;
    }

    if monitor_changed {
        bump_monitor_generation(state);
    }

    if text_selector.is_visible() {
        let texts_json = state.with_config(AppConfig::texts_to_json_list);
        text_selector.refresh_items(&texts_json);
    }

    let message = build_reload_message(
        hotkeys_changed || favorite_hotkeys_changed,
        monitor_changed,
        migrated,
    );
    crate::log_info!("設定を再読み込みしました: {}", message);
    Ok(ConfigReloadOutcome { message })
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 再読み込み成功時の通知メッセージを組み立てる
fn build_reload_message(hotkeys_changed: bool, monitor_changed: bool, migrated: bool) -> String {
    let mut parts = vec!["設定を反映しました".to_string()];
    if hotkeys_changed {
        parts.push("ホットキーを更新".to_string());
    }
    if monitor_changed {
        parts.push("監視設定を更新".to_string());
    }
    if migrated {
        parts.push("設定スキーマを移行".to_string());
    }
    parts.join(" / ")
}

/// 再読み込みエラーをユーザー向けメッセージへ変換する
fn reload_error_message(error: &ConfigReloadError) -> String {
    match error {
        ConfigReloadError::Parse(detail) => {
            format!("{} ({detail})", error.user_message())
        }
        _ => error.user_message().to_string(),
    }
}

/// 設定再読み込みを実行し、結果をデスクトップ通知で表示する
pub fn reload_config_with_notification(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    hotkey_handler: &mut HotkeyHandler,
    text_selector: &TextSelectorWindow,
) {
    match apply_config_reload(state, menu, hotkey_handler, text_selector) {
        Ok(outcome) => platform::show_notification("設定を再読み込み", &outcome.message),
        Err(message) => {
            crate::log_warn!("設定の再読み込みに失敗: {}", message);
            platform::show_notification("設定の再読み込み", &message);
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
        let message = build_reload_message(true, true, true);
        assert!(message.contains("設定を反映しました"));
        assert!(message.contains("ホットキーを更新"));
        assert!(message.contains("監視設定を更新"));
        assert!(message.contains("設定スキーマを移行"));
    }

    /// 変更がない場合は基本メッセージのみであること
    #[test]
    fn build_reload_message_without_changes() {
        assert_eq!(
            build_reload_message(false, false, false),
            "設定を反映しました"
        );
    }
}
