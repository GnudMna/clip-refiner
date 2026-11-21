use std::sync::Arc;

use crate::tray::clipboard_monitor::bump_monitor_generation;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::notify::{
    NotificationCategory, show_category_notification, show_exclusive_mode_notification,
};
use crate::tray::state::{AppState, LockExt, SettingsEvent};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

/// 連続記録の有効/無効を切り替える
///
/// # Arguments
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `_clipboard_worker` - クリップボード・ワーカーへの送信ハンドル
/// * `source` - 切替通知のタイトル (操作元)。`None` なら排他時の通知だけを表示する
pub(crate) fn toggle_continuous_record(
    menu: &TrayMenu,
    state: &Arc<AppState>,
    _clipboard_worker: &ClipboardWorkerHandle,
    source: Option<&str>,
) {
    let enabled = !state.with_config(|c| c.continuous_record_enabled);
    set_continuous_record_enabled(state, menu, enabled, source);
    state.save_config();
}

/// 連続記録の有効・無効を設定し、メニュー表示と監視状態へ反映する
///
/// 有効にするときに自動変換が有効なら、後から有効にした連続記録を優先して自動変換を無効にし、
/// その旨を通知する (切替通知を兼ねる)。`source` を渡した場合は、それ以外の切替も通知する
/// 設定ファイルへの保存は呼び出し側で行う
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `menu` - トレイメニュー構造体
/// * `enabled` - 連続記録を有効にする場合は `true`
/// * `source` - 切替通知のタイトル (操作元)。`None` なら排他時の通知だけを表示する
///
/// # Returns
/// * `bool` - 連続記録または自動変換の設定値が変わった場合は `true`
pub(crate) fn set_continuous_record_enabled(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    enabled: bool,
    source: Option<&str>,
) -> bool {
    let (changed, auto_refine_disabled) = state.with_config_mut(|c| {
        let changed = c.continuous_record_enabled != enabled;
        c.continuous_record_enabled = enabled;
        let auto_refine_disabled = enabled && std::mem::replace(&mut c.auto_refine_enabled, false);
        (changed, auto_refine_disabled)
    });
    if !changed && !auto_refine_disabled {
        return false;
    }

    menu.continuous_record.enabled_item.set_checked(enabled);
    if enabled {
        // 有効化前の加工書き戻し指紋が、同一内容の再コピーを誤って抑止しないようクリアする
        state
            .clipboard_session()
            .with_processed_state(|ps| ps.last_written = None);
    }
    bump_monitor_generation(state);
    if let Err(err) = menu.refresh_continuous_record(state) {
        dispatch::log_menu_operation_error("連続記録メニューの更新", err);
    }
    menu.sync_auto_processing_state(state);
    if auto_refine_disabled {
        show_exclusive_mode_notification(
            state,
            source.unwrap_or("連続記録モード"),
            "連続記録モードを有効にしたため、自動変換を無効にしました",
        );
        dispatch::send_app_event(&state.proxy, SettingsEvent::Refresh.into());
    } else if changed && let Some(source) = source {
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            source,
            if enabled {
                "連続記録モードを有効にしました"
            } else {
                "連続記録モードを無効にしました"
            },
        );
    }
    true
}

/// 連続記録キュー末尾 (直近の記録) を 1 件取り消す
///
/// クリップボード本体は変更しない
///
/// # Arguments
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `source_label` - 通知タイトルに使う操作元ラベル
pub(crate) fn undo_last_continuous_record(
    menu: &TrayMenu,
    state: &Arc<AppState>,
    source_label: &str,
) {
    if state.undo_last_continuous_record() {
        if let Err(err) = menu.refresh_continuous_record(state) {
            dispatch::log_menu_operation_error("連続記録メニューの更新", err);
        }
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            source_label,
            "直近の記録を取り消しました",
        );
    } else {
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            source_label,
            "取り消せる記録がありません",
        );
    }
}

/// 連続記録に関連するメニューイベント(有効化切替、取り消し、消去、過去項目の選択)を処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
/// * `clipboard_worker` - クリップボードワーカーへの送信ハンドル
///
/// # Returns
/// * `bool` - イベントがこの関数内で処理された場合は `true`、そうでない場合は `false` を返す
pub(crate) fn handle_continuous_record_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> bool {
    if id == menu.continuous_record.enabled_item.id() {
        toggle_continuous_record(menu, state, clipboard_worker, None);
        return true;
    }
    if id == menu.continuous_record.undo_item.id() {
        undo_last_continuous_record(menu, state, "連続記録モード");
        return true;
    }
    if id == menu.continuous_record.clear_item.id() {
        state.clipboard_session().clear_continuous_record();
        state.save_config();
        if let Err(err) = menu.refresh_continuous_record(state) {
            dispatch::log_menu_operation_error("連続記録メニューの更新", err);
        }
        return true;
    }

    let menu_records = menu.continuous_record.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        dispatch::send_clipboard_command(
            clipboard_worker,
            ClipboardCommand::DischargeContinuousRecordAt(*index),
        );
        return true;
    }

    false
}
