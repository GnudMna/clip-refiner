use std::sync::Arc;

use crate::tray::settings::SettingsWindow;
use crate::tray::settings::snapshot::SettingsSnapshot;
use crate::tray::state::AppState;

// ======================================================================
// 表示更新
// ======================================================================
/// 設定ウィンドウ UI の更新範囲
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsRefreshScope {
    /// 全フィールドを再同期する
    Full,
    /// 登録クリップ一覧のみ更新する
    Clips,
    /// お気に入りとショートカット一覧のみ更新する
    FavoritesAndShortcuts,
}

/// 設定ウィンドウの表示内容を最新の設定へ更新する
pub(crate) fn refresh_settings_window(state: &Arc<AppState>, settings: &SettingsWindow) {
    refresh_settings_window_scoped(state, settings, SettingsRefreshScope::Full, false, false);
}

/// 外部で変更された設定ファイルを設定ウィンドウへ反映する
pub(crate) fn refresh_settings_window_after_external_reload(
    state: &Arc<AppState>,
    settings: &SettingsWindow,
) {
    refresh_settings_window_scoped(state, settings, SettingsRefreshScope::Full, false, true);
}

/// 登録クリップ一覧のみ設定ウィンドウへ反映する
pub(crate) fn refresh_settings_clips(state: &Arc<AppState>, settings: &SettingsWindow) {
    refresh_settings_window_scoped(state, settings, SettingsRefreshScope::Clips, false, false);
}

/// お気に入りとショートカット一覧のみ設定ウィンドウへ反映する
pub(crate) fn refresh_settings_favorites_and_shortcuts(
    state: &Arc<AppState>,
    settings: &SettingsWindow,
) {
    refresh_settings_window_scoped(
        state,
        settings,
        SettingsRefreshScope::FavoritesAndShortcuts,
        false,
        false,
    );
}

/// 設定ウィンドウの表示内容を指定範囲で更新する
pub(crate) fn refresh_settings_window_scoped(
    state: &Arc<AppState>,
    settings: &SettingsWindow,
    scope: SettingsRefreshScope,
    force: bool,
    external_sync: bool,
) {
    if !settings.is_visible() {
        return;
    }

    match scope {
        SettingsRefreshScope::Full => {
            let snapshot = state.with_config(SettingsSnapshot::from_config);
            settings.load_snapshot_with_options(&snapshot.to_json(), force, external_sync);
        }
        SettingsRefreshScope::Clips => {
            let clips_json = state.with_config(SettingsSnapshot::clips_json);
            settings.update_clips(&clips_json);
        }
        SettingsRefreshScope::FavoritesAndShortcuts => {
            let json = state.with_config(SettingsSnapshot::favorites_and_shortcuts_json);
            settings.update_favorites_and_shortcuts(&json);
        }
    }
}
