use std::collections::HashSet;
use std::sync::Arc;

use super::SettingsWindow;
use super::ipc::SettingsPatch;
use super::snapshot::SettingsSnapshot;

use crate::config::MonitorMode;
use crate::refiner::{REGEX_PREVIEW_SAMPLE, preview_regex_replace};
use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::dispatch;
use crate::tray::event::{update_monitor_mode, update_refine};
use crate::tray::menu::TrayMenu;
use crate::tray::notify;
use crate::tray::quick_selector::QuickSelectorWindow;
use crate::tray::state::AppState;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use serde::Serialize;

// ======================================================================
// 適用フィードバック
// ======================================================================
/// 設定パッチ適用結果 (設定ウィンドウ UI へ返す)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsApplyFeedback {
    /// 適用に成功したかどうか
    pub ok: bool,
    /// 対象フィールド (`launchAtLogin` など)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// ユーザー向けメッセージ
    pub message: String,
    /// デスクトップ通知を別経路で既に表示済みかどうか
    #[serde(skip)]
    pub(crate) desktop_already_notified: bool,
}

impl SettingsApplyFeedback {
    /// 成功メッセージを生成する
    pub(crate) fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            field: None,
            message: message.into(),
            desktop_already_notified: false,
        }
    }

    /// エラーメッセージを生成する
    pub(crate) fn error(field: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            field: field.map(str::to_string),
            message: message.into(),
            desktop_already_notified: false,
        }
    }

    /// デスクトップ通知を別経路で既に表示済みであることを記録する
    pub(crate) fn with_desktop_already_notified(mut self) -> Self {
        self.desktop_already_notified = true;
        self
    }
}

/// 非表示ウィンドウ向けにデスクトップ通知で適用エラーを伝えるべきかどうか
pub(crate) fn should_notify_apply_error_via_desktop(
    window_visible: bool,
    feedback: &SettingsApplyFeedback,
) -> bool {
    !window_visible && !feedback.ok && !feedback.desktop_already_notified
}

/// 部分更新パッチの適用結果
pub(crate) struct SettingsPatchApplyResult {
    /// UI 再同期が必要な場合のスコープと `force` フラグ
    pub refresh: Option<(SettingsRefreshScope, bool)>,
    /// ユーザーへ通知するフィードバック
    pub feedback: Option<SettingsApplyFeedback>,
}

// ======================================================================
// ウィンドウ内コマンド
// ======================================================================
/// 設定ウィンドウ内で完結する操作
///
/// クリップ操作やお気に入り変更などは `ipc::dispatch_settings_ipc` 経由で
/// 共通の `AppEvent` ハンドラへ委譲する
pub(crate) enum SettingsWindowCommand {
    /// 部分更新パッチを適用する
    Apply(SettingsPatch),
    /// 履歴を全削除する
    ClearHistory,
    /// 連続記録を全削除する
    ClearContinuousRecord,
    /// 設定ファイルを既定アプリで開く
    OpenConfigFile,
}

/// 設定ウィンドウ内コマンドを処理する
pub(crate) fn handle_settings_window_command(
    command: SettingsWindowCommand,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    settings: &SettingsWindow,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) {
    match command {
        SettingsWindowCommand::Apply(patch) => {
            apply_settings_patch(
                &patch,
                state,
                menu,
                settings,
                quick_selector,
                clipboard_worker,
            );
        }
        SettingsWindowCommand::ClearHistory => {
            state.clear_history();
            state.save_config();
            if let Err(err) = menu.refresh_history(state) {
                dispatch::log_menu_operation_error("履歴メニューの更新", err);
            }
            refresh_settings_window(state, settings);
            settings.notify_apply_feedback(&SettingsApplyFeedback::ok("履歴をクリアしました"));
        }
        SettingsWindowCommand::ClearContinuousRecord => {
            state.clear_continuous_record();
            state.save_config();
            if let Err(err) = menu.refresh_continuous_record(state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
            refresh_settings_window(state, settings);
            settings.notify_apply_feedback(&SettingsApplyFeedback::ok("連続記録をクリアしました"));
        }
        SettingsWindowCommand::OpenConfigFile => {
            state.save_config();
            if let Err(e) = crate::config::open_config_file() {
                crate::log_error!("設定ファイルの起動に失敗: {:?}", e);
                crate::platform::show_notification("エラー", "設定ファイルを開けませんでした");
                settings.notify_apply_feedback(
                    &SettingsApplyFeedback::error(None, "設定ファイルを開けませんでした")
                        .with_desktop_already_notified(),
                );
            }
        }
    }
}

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

// ======================================================================
// パッチ適用
// ======================================================================
/// 部分更新パッチの適用結果
#[allow(clippy::struct_excessive_bools)]
struct PatchApplyOutcome {
    monitor_mode_changed: bool,
    bump_monitor: bool,
    mode_changed: bool,
    pipeline_changed: bool,
}

/// 部分更新パッチを設定へ反映する
fn apply_settings_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    settings: &SettingsWindow,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) {
    let result =
        apply_settings_patch_without_ui(patch, state, menu, quick_selector, clipboard_worker);
    if let Some(feedback) = result.feedback {
        settings.notify_apply_feedback(&feedback);
    }
    if let Some((scope, force)) = result.refresh {
        refresh_settings_window_scoped(state, settings, scope, force, false);
    }
}

/// UI を介さず部分更新パッチを設定へ反映する
pub(crate) fn apply_settings_patch_without_ui(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> SettingsPatchApplyResult {
    let mut outcome = PatchApplyOutcome {
        monitor_mode_changed: false,
        bump_monitor: false,
        mode_changed: false,
        pipeline_changed: false,
    };
    let mut config_dirty = false;

    apply_monitor_patch(patch, state, menu, &mut outcome, &mut config_dirty);
    apply_notification_patch(patch, state, menu, &mut config_dirty);
    apply_history_patch(patch, state, menu, &mut outcome, &mut config_dirty);
    apply_continuous_record_patch(
        patch,
        state,
        menu,
        clipboard_worker,
        &mut outcome,
        &mut config_dirty,
    );
    let regex_feedback = apply_regex_patch(patch, state, &mut config_dirty);
    apply_refine_patch(
        patch,
        state,
        menu,
        quick_selector,
        clipboard_worker,
        &mut outcome,
        &mut config_dirty,
    );
    let system_outcome = apply_system_patch(patch, state, menu, &mut config_dirty);

    if config_dirty {
        state.save_config();
    }

    if outcome.bump_monitor && !outcome.mode_changed {
        bump_monitor_generation(state);
    }

    let feedback = regex_feedback.or(system_outcome.feedback);

    SettingsPatchApplyResult {
        refresh: refresh_plan_after_patch(patch, system_outcome.force_refresh),
        feedback,
    }
}

/// パッチ適用後に UI へ再同期が必要か判定する
///
/// UI 発の変更でサーバー側正規化 (`normalize`) が走る項目のみ再同期する
fn refresh_plan_after_patch(
    patch: &SettingsPatch,
    force: bool,
) -> Option<(SettingsRefreshScope, bool)> {
    if force {
        return Some((SettingsRefreshScope::Full, true));
    }
    if patch.pipeline.is_some()
        || patch.history_limit.is_some()
        || patch.continuous_record_limit.is_some()
    {
        return Some((SettingsRefreshScope::Full, false));
    }
    None
}

/// 監視方式と監視周期のパッチを適用する
///
/// イベント監視への切替と同時に `interval_ms` が含まれる場合は周期をスキップする
/// (イベント監視時は周期を使用しない)
fn apply_monitor_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(monitor_mode) = patch.monitor_mode
        && state.with_config(|c| c.monitor_mode) != monitor_mode
    {
        update_monitor_mode(state, menu, monitor_mode);
        outcome.monitor_mode_changed = true;
    }

    let switching_to_event =
        outcome.monitor_mode_changed && patch.monitor_mode == Some(MonitorMode::Event);

    if let Some(interval_ms) = patch.interval_ms
        && !switching_to_event
    {
        state.with_config_mut(|c| {
            c.interval_ms = interval_ms;
            c.normalize();
        });
        let interval_ms = state.with_config(|c| c.interval_ms);
        for (item, ms) in &menu.interval.items {
            item.set_checked(*ms == interval_ms);
        }
        *config_dirty = true;
    }
}

/// 通知設定のパッチを適用する
fn apply_notification_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    config_dirty: &mut bool,
) {
    let Some(notification_settings) = patch.notification_settings.as_ref() else {
        return;
    };

    state.with_config_mut(|c| c.notification_settings = notification_settings.clone());
    menu.notification
        .enabled_item
        .set_checked(state.with_config(|c| c.notification_settings.enabled));
    menu.notification
        .notify_mode_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_mode));
    menu.notification
        .notify_result_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_result));
    menu.notification
        .notify_pause_item
        .set_checked(state.with_config(|c| c.notification_settings.notify_pause));
    *config_dirty = true;
}

/// 履歴設定のパッチを適用する
fn apply_history_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(history_enabled) = patch.history_enabled {
        state.with_config_mut(|c| c.history_enabled = history_enabled);
        menu.history.enabled_item.set_checked(history_enabled);
        *config_dirty = true;
        outcome.bump_monitor = true;
        if let Err(err) = menu.refresh_history(state) {
            dispatch::log_menu_operation_error("履歴メニューの更新", err);
        }
    }

    if let Some(history_limit) = patch.history_limit {
        state.with_config_mut(|c| {
            c.history_limit = history_limit;
            c.normalize();
        });
        *config_dirty = true;
    }
}

/// 連続記録設定のパッチを適用する
fn apply_continuous_record_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clipboard_worker: &ClipboardWorkerHandle,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(continuous_record_enabled) = patch.continuous_record_enabled {
        let current = state.with_config(|c| c.continuous_record_enabled);
        if current != continuous_record_enabled {
            menu.continuous_record
                .enabled_item
                .set_checked(continuous_record_enabled);
            state.with_config_mut(|c| c.continuous_record_enabled = continuous_record_enabled);
            *config_dirty = true;
            outcome.bump_monitor = true;
            if continuous_record_enabled {
                dispatch::send_clipboard_command(
                    clipboard_worker,
                    ClipboardCommand::CatchUpContinuousRecord,
                );
            }
            if let Err(err) = menu.refresh_continuous_record(state) {
                dispatch::log_menu_operation_error("連続記録メニューの更新", err);
            }
        }
    }

    if let Some(continuous_record_limit) = patch.continuous_record_limit {
        state.with_config_mut(|c| {
            c.continuous_record_limit = continuous_record_limit;
            c.normalize();
        });
        *config_dirty = true;
    }
}

/// 正規表現設定のパッチを適用する
///
/// パターンがコンパイルできない場合は設定を更新せずエラーフィードバックを返す
fn apply_regex_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    config_dirty: &mut bool,
) -> Option<SettingsApplyFeedback> {
    let regex = patch.regex.as_ref()?;

    let preview = preview_regex_replace(REGEX_PREVIEW_SAMPLE, regex);
    if !preview.valid {
        return Some(SettingsApplyFeedback::error(Some("regex"), preview.message));
    }

    state.with_config_mut(|c| c.regex = regex.clone());
    *config_dirty = true;
    None
}

/// 加工モードとパイプラインのパッチを適用する
fn apply_refine_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    clipboard_worker: &ClipboardWorkerHandle,
    outcome: &mut PatchApplyOutcome,
    config_dirty: &mut bool,
) {
    if let Some(pipeline) = patch.pipeline.clone() {
        state.with_config_mut(|c| {
            c.pipeline = pipeline;
            if !c.pipeline.is_empty() {
                c.mode = c.pipeline[0];
            }
            c.normalize();
        });
        outcome.pipeline_changed = true;
        sync_refine_menu_checks(state, menu);
        *config_dirty = true;
        outcome.bump_monitor = true;
    }

    if let Some(mode) = patch.mode
        && !outcome.pipeline_changed
        && state.with_config(|c| c.mode) != mode
    {
        if state.with_config(|c| c.pipeline.is_empty()) {
            update_refine(state, menu, clipboard_worker, mode, quick_selector);
            outcome.mode_changed = true;
        } else {
            state.with_config_mut(|c| c.mode = mode);
            menu.refresh_category_labels(mode);
            *config_dirty = true;
        }
    }
}

/// OS 連携パッチの適用結果
struct SystemPatchOutcome {
    /// UI の強制再同期が必要かどうか
    force_refresh: bool,
    /// ユーザーへ通知するフィードバック
    feedback: Option<SettingsApplyFeedback>,
}

/// OS 連携と一時停止のパッチを適用する
fn apply_system_patch(
    patch: &SettingsPatch,
    state: &Arc<AppState>,
    menu: &TrayMenu,
    config_dirty: &mut bool,
) -> SystemPatchOutcome {
    let mut force_refresh = false;
    let mut feedback = None;

    if let Some(launch_at_login) = patch.launch_at_login {
        let current = crate::platform::autostart::is_enabled();
        if current != launch_at_login
            && let Err(e) = crate::platform::autostart::set_enabled(launch_at_login)
        {
            crate::log_error!("ログイン時自動起動の設定に失敗: {:?}", e);
            crate::platform::show_notification("エラー", "ログイン時自動起動の設定に失敗しました");
            force_refresh = true;
            feedback = Some(
                SettingsApplyFeedback::error(
                    Some("launchAtLogin"),
                    "ログイン時自動起動の設定に失敗しました",
                )
                .with_desktop_already_notified(),
            );
        }
    }

    if let Some(is_paused) = patch.is_paused {
        let current = state.with_config(|c| c.is_paused);
        if current != is_paused {
            state.with_config_mut(|c| c.is_paused = is_paused);
            menu.pause_item.set_checked(is_paused);
            notify::show_pause_notification(state, is_paused, "設定ウィンドウ");
            *config_dirty = true;
            bump_monitor_generation(state);
        }
    }

    SystemPatchOutcome {
        force_refresh,
        feedback,
    }
}

/// 変換モードメニューのチェック状態を設定内容へ同期する
fn sync_refine_menu_checks(state: &Arc<AppState>, menu: &TrayMenu) {
    let (mode, active_modes) = state.with_config(|config| {
        (
            config.mode,
            config
                .effective_pipeline()
                .into_iter()
                .collect::<HashSet<_>>(),
        )
    });
    menu.refine
        .all_mode_items()
        .for_each(|(item, mode)| item.set_checked(active_modes.contains(mode)));
    menu.refresh_category_labels(mode);
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;

    use super::*;

    use crate::config::{MonitorMode, NotificationSettings};
    use crate::consts;
    use crate::refiner::RefineMode;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::test_app_state;
    use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

    fn with_test_worker(
        state: &Arc<AppState>,
    ) -> (Arc<ClipboardWorkerHandle>, mpsc::Receiver<ClipboardCommand>) {
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(Arc::clone(state), tx);
        (worker, rx)
    }

    /// 通知設定パッチが `AppConfig` とトレイメニューへ反映されること
    #[test]
    fn notification_patch_updates_config_and_menu() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            notification_settings: Some(NotificationSettings {
                enabled: true,
                notify_mode: false,
                notify_result: true,
                notify_pause: false,
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        apply_notification_patch(&patch, &state, &menu, &mut config_dirty);

        let ns = state.with_config(|c| c.notification_settings.clone());
        assert!(ns.enabled);
        assert!(!ns.notify_mode);
        assert!(ns.notify_result);
        assert!(!ns.notify_pause);
        assert!(menu.notification.enabled_item.is_checked());
        assert!(!menu.notification.notify_mode_item.is_checked());
        assert!(menu.notification.notify_result_item.is_checked());
        assert!(!menu.notification.notify_pause_item.is_checked());
        assert!(config_dirty);
    }

    /// 正規表現パッチが `AppConfig` へ反映されること
    #[test]
    fn regex_patch_updates_config() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings {
                pattern: r"\d+".to_string(),
                replacement: "bar".to_string(),
                case_insensitive: true,
                multiline: true,
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(apply_regex_patch(&patch, &state, &mut config_dirty).is_none());

        let regex = state.with_config(|c| c.regex.clone());
        assert_eq!(regex.pattern, r"\d+");
        assert_eq!(regex.replacement, "bar");
        assert!(regex.case_insensitive);
        assert!(regex.multiline);
        assert!(config_dirty);
    }

    /// 無効な正規表現パッチは拒否されること
    #[test]
    fn regex_patch_rejects_invalid_pattern() {
        let state = Arc::new(test_app_state());
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings {
                pattern: "[unclosed".to_string(),
                replacement: String::new(),
                ..crate::config::RegexSettings::default()
            }),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        let feedback = apply_regex_patch(&patch, &state, &mut config_dirty)
            .expect("無効パターンでエラーフィードバックが返ること");
        assert!(!feedback.ok);
        assert_eq!(feedback.field.as_deref(), Some("regex"));
        assert!(!config_dirty);
    }

    /// 一時停止パッチが `AppConfig` とトレイメニューへ反映されること
    #[test]
    fn pause_patch_updates_config_and_menu() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            is_paused: Some(true),
            ..SettingsPatch::default()
        };

        let mut config_dirty = false;
        assert!(!apply_system_patch(&patch, &state, &menu, &mut config_dirty).force_refresh);
        assert!(state.with_config(|c| c.is_paused));
        assert!(menu.pause_item.is_checked());
        assert!(config_dirty);
    }

    /// 履歴件数パッチが `normalize` でクランプされること
    #[test]
    fn history_limit_patch_clamps_to_max() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            history_limit: Some(999),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_history_patch(&patch, &state, &menu, &mut outcome, &mut config_dirty);

        assert_eq!(
            state.with_config(|c| c.history_limit),
            consts::MAX_HISTORY_LIMIT
        );
        assert!(config_dirty);
    }

    /// 監視周期パッチが `normalize` で下限へクランプされること
    #[test]
    fn interval_ms_patch_clamps_below_min() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            interval_ms: Some(10),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_monitor_patch(&patch, &state, &menu, &mut outcome, &mut config_dirty);

        assert_eq!(
            state.with_config(|c| c.interval_ms),
            consts::MIN_INTERVAL_MS
        );
        assert_eq!(
            state.monitor_snapshot().interval_ms,
            consts::MIN_INTERVAL_MS
        );
        assert!(config_dirty);
    }

    /// 監視周期パッチが `normalize` で上限へクランプされること
    #[test]
    fn interval_ms_patch_clamps_above_max() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            interval_ms: Some(999_999),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_monitor_patch(&patch, &state, &menu, &mut outcome, &mut config_dirty);

        assert_eq!(
            state.with_config(|c| c.interval_ms),
            consts::MAX_INTERVAL_MS
        );
        assert_eq!(
            state.monitor_snapshot().interval_ms,
            consts::MAX_INTERVAL_MS
        );
        assert!(config_dirty);
    }

    /// イベント監視への切替と同時の周期パッチでは周期をスキップすること
    #[test]
    fn monitor_patch_skips_interval_when_switching_to_event() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let default_interval = state.with_config(|c| c.interval_ms);
        let patch = SettingsPatch {
            monitor_mode: Some(MonitorMode::Event),
            interval_ms: Some(500),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_monitor_patch(&patch, &state, &menu, &mut outcome, &mut config_dirty);

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Event);
        assert_eq!(state.with_config(|c| c.interval_ms), default_interval);
    }

    /// ポーリングへの切替と同時の周期パッチでは周期を反映すること
    #[test]
    fn monitor_patch_applies_interval_when_switching_to_polling() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| c.monitor_mode = MonitorMode::Event);
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let patch = SettingsPatch {
            monitor_mode: Some(MonitorMode::Polling),
            interval_ms: Some(500),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_monitor_patch(&patch, &state, &menu, &mut outcome, &mut config_dirty);

        assert_eq!(state.with_config(|c| c.monitor_mode), MonitorMode::Polling);
        assert_eq!(state.with_config(|c| c.interval_ms), 500);
        assert!(config_dirty);
        assert!(
            menu.interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 500 && item.is_checked())
        );
    }

    /// パイプライン非空時のデフォルトモード変更が `mode` のみ更新しパイプラインを保持すること
    #[test]
    fn default_mode_patch_updates_config_when_pipeline_active() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|c| {
            c.pipeline = vec![RefineMode::Trim, RefineMode::TrimLines];
            c.mode = RefineMode::Trim;
        });
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, rx) = with_test_worker(&state);
        let patch = SettingsPatch {
            mode: Some(RefineMode::UrlDecode),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            &state,
            &menu,
            None,
            &worker,
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            state.with_config(|c| c.pipeline.clone()),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlDecode);
        assert_eq!(
            state.with_config(crate::config::AppConfig::effective_pipeline),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert!(config_dirty);
        assert!(!outcome.mode_changed);
        assert!(!outcome.pipeline_changed);
        assert!(rx.try_recv().is_err());
    }

    /// パイプライン空時のデフォルトモード変更が `update_refine` 経由で反映されること
    #[test]
    fn default_mode_patch_uses_update_refine_when_pipeline_empty() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, rx) = with_test_worker(&state);
        let patch = SettingsPatch {
            mode: Some(RefineMode::UrlDecode),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            &state,
            &menu,
            None,
            &worker,
            &mut outcome,
            &mut config_dirty,
        );

        assert!(state.with_config(|c| c.pipeline.is_empty()));
        assert_eq!(state.with_config(|c| c.mode), RefineMode::UrlDecode);
        assert!(outcome.mode_changed);
        match rx.try_recv() {
            Ok(ClipboardCommand::ProcessMode(mode)) => assert_eq!(mode, RefineMode::UrlDecode),
            other => panic!("ProcessMode コマンドが送信されること: {other:?}"),
        }
    }

    /// パイプライン変更が設定とメニューへ反映されること
    #[test]
    fn pipeline_patch_updates_config_and_menu() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, _rx) = with_test_worker(&state);
        let patch = SettingsPatch {
            pipeline: Some(vec![RefineMode::Trim, RefineMode::TrimLines]),
            ..SettingsPatch::default()
        };
        let mut outcome = PatchApplyOutcome {
            monitor_mode_changed: false,
            bump_monitor: false,
            mode_changed: false,
            pipeline_changed: false,
        };
        let mut config_dirty = false;

        apply_refine_patch(
            &patch,
            &state,
            &menu,
            None,
            &worker,
            &mut outcome,
            &mut config_dirty,
        );

        assert_eq!(
            state.with_config(|c| c.pipeline.clone()),
            vec![RefineMode::Trim, RefineMode::TrimLines]
        );
        assert_eq!(state.with_config(|c| c.mode), RefineMode::Trim);
        assert!(config_dirty);
        assert!(outcome.pipeline_changed);
    }

    /// `apply_settings_patch_without_ui` が IPC 相当の変更を一括反映すること
    #[test]
    fn apply_without_ui_updates_config_from_merged_patch() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, _rx) = with_test_worker(&state);
        let patch = SettingsPatch {
            interval_ms: Some(2_000),
            history_enabled: Some(true),
            ..SettingsPatch::default()
        };

        assert!(
            apply_settings_patch_without_ui(&patch, &state, &menu, None, &worker)
                .refresh
                .is_none()
        );
        assert_eq!(state.with_config(|c| c.interval_ms), 2_000);
        assert!(state.with_config(|c| c.history_enabled));
        assert!(menu.history.enabled_item.is_checked());
    }

    /// 適用フィードバック JSON が camelCase であること
    #[test]
    fn apply_feedback_json_uses_camel_case() {
        let feedback = SettingsApplyFeedback::error(
            Some("launchAtLogin"),
            "ログイン時自動起動の設定に失敗しました",
        );
        let json: serde_json::Value =
            serde_json::to_value(&feedback).expect("フィードバック JSON のシリアライズ");
        assert_eq!(json["ok"], false);
        assert_eq!(json["field"], "launchAtLogin");
        assert_eq!(json["message"], "ログイン時自動起動の設定に失敗しました");
        assert!(json.get("desktopAlreadyNotified").is_none());
    }

    /// 非表示ウィンドウでは未通知の適用エラーをデスクトップ通知へ送ること
    #[test]
    fn apply_feedback_hidden_error_should_use_desktop_fallback() {
        let feedback = SettingsApplyFeedback::error(Some("regex"), "パターンが無効です");
        assert!(should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 非表示でもデスクトップ通知済みのエラーは重複通知しないこと
    #[test]
    fn apply_feedback_hidden_error_skips_desktop_when_already_notified() {
        let feedback = SettingsApplyFeedback::error(Some("launchAtLogin"), "失敗")
            .with_desktop_already_notified();
        assert!(!should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 表示中のウィンドウではデスクトップ通知フォールバックを使わないこと
    #[test]
    fn apply_feedback_visible_window_skips_desktop_fallback() {
        let feedback = SettingsApplyFeedback::error(None, "エラー");
        assert!(!should_notify_apply_error_via_desktop(true, &feedback));
    }

    /// 非表示かつ成功時はデスクトップ通知しないこと
    #[test]
    fn apply_feedback_hidden_success_skips_desktop_fallback() {
        let feedback = SettingsApplyFeedback::ok("保存しました");
        assert!(!should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// ウィンドウ非表示後の無効な正規表現パッチがフォールバック通知対象であること
    #[test]
    fn invalid_regex_feedback_targets_desktop_fallback_after_hide() {
        let state = Arc::new(test_app_state());
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        let (worker, _rx) = with_test_worker(&state);
        let patch = SettingsPatch {
            regex: Some(crate::config::RegexSettings {
                pattern: "[unclosed".to_string(),
                ..crate::config::RegexSettings::default()
            }),
            ..SettingsPatch::default()
        };

        let result = apply_settings_patch_without_ui(&patch, &state, &menu, None, &worker);
        let feedback = result
            .feedback
            .expect("無効パターンでエラーフィードバックが返ること");
        assert!(!feedback.ok);
        assert!(!feedback.desktop_already_notified);
        assert!(should_notify_apply_error_via_desktop(false, &feedback));
    }

    /// 通知設定のみのパッチでは UI 再同期を省略すること
    #[test]
    fn refresh_plan_skips_notification_only_patch() {
        let patch = SettingsPatch {
            notification_settings: Some(NotificationSettings::default()),
            ..SettingsPatch::default()
        };
        assert!(refresh_plan_after_patch(&patch, false).is_none());
    }

    /// パイプライン変更では正規化結果を UI へ反映すること
    #[test]
    fn refresh_plan_syncs_pipeline_patch() {
        let patch = SettingsPatch {
            pipeline: Some(vec![RefineMode::Trim]),
            ..SettingsPatch::default()
        };
        assert_eq!(
            refresh_plan_after_patch(&patch, false),
            Some((SettingsRefreshScope::Full, false))
        );
    }

    /// 履歴件数変更では正規化結果を UI へ反映すること
    #[test]
    fn refresh_plan_syncs_history_limit_patch() {
        let patch = SettingsPatch {
            history_limit: Some(50),
            ..SettingsPatch::default()
        };
        assert_eq!(
            refresh_plan_after_patch(&patch, false),
            Some((SettingsRefreshScope::Full, false))
        );
    }

    /// 自動起動設定失敗時は強制再同期すること
    #[test]
    fn refresh_plan_forces_on_autostart_failure() {
        let patch = SettingsPatch::default();
        assert_eq!(
            refresh_plan_after_patch(&patch, true),
            Some((SettingsRefreshScope::Full, true))
        );
    }
}
