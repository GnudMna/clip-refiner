use super::types::TrayMenu;
use crate::tray::dispatch;
use crate::tray::state::{AppState, LockExt, MenuSnapshot};

use anyhow::Result;

// ======================================================================
// 設定同期
// ======================================================================
impl TrayMenu {
    /// 現在の設定内容をメニュー表示へ反映する
    ///
    /// ディスクからの再読み込み後に、チェック状態やサブメニューを同期する
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Result<()>` - 同期成功時は `Ok(())`、失敗時は `Err`
    pub fn sync_from_config(&self, state: &AppState) -> Result<()> {
        let snap: MenuSnapshot = state.menu_snapshot();

        self.refine
            .favorite_records
            .lock_ignore_poison()
            .iter()
            .chain(self.refine.all_mode_items())
            .for_each(|(item, mode)| item.set_checked(snap.active_modes.contains(mode)));
        self.refresh_category_labels(snap.mode);
        self.refine
            .sync_favorite_actions(snap.mode, &snap.favorite_modes);
        self.refine.sync_mode_labels(&snap.favorite_modes);
        if let Err(err) =
            self.refine
                .rebuild_favorites(&snap.active_modes, &snap.favorite_modes, &snap.hotkeys)
        {
            dispatch::log_menu_operation_error("お気に入りメニューの再構築", err);
        }

        for (item, monitor_mode) in &self.monitor.items {
            item.set_checked(*monitor_mode == snap.monitor_mode);
        }
        super::super::clipboard_monitor::update_monitor_mode_impl(self, snap.monitor_mode);

        self.sync_interval_menu(snap.interval_ms);

        self.history.enabled_item.set_checked(snap.history_enabled);
        self.continuous_record
            .enabled_item
            .set_checked(snap.continuous_record_enabled);
        self.notification
            .enabled_item
            .set_checked(snap.notification_settings.enabled);
        self.notification
            .notify_process_item
            .set_checked(snap.notification_settings.notify_process);
        self.notification
            .notify_pause_item
            .set_checked(snap.notification_settings.notify_pause);
        self.notification
            .notify_history_restore_item
            .set_checked(snap.notification_settings.notify_history_restore);
        self.notification
            .notify_ocr_item
            .set_checked(snap.notification_settings.notify_ocr);
        self.notification
            .notify_registered_clip_item
            .set_checked(snap.notification_settings.notify_registered_clip);
        self.notification
            .notify_favorites_item
            .set_checked(snap.notification_settings.notify_favorites);
        self.notification
            .notify_continuous_record_item
            .set_checked(snap.notification_settings.notify_continuous_record);
        self.notification
            .notify_result_item
            .set_checked(snap.notification_settings.notify_result);
        self.pause_item.set_checked(snap.is_paused);

        self.refresh_clips(state)?;
        self.refresh_regex_presets_logged(state);
        self.refresh_history(state)?;
        self.refresh_continuous_record(state)?;
        self.sync_auto_processing_state(state);

        Ok(())
    }

    /// 連続記録の有効状態に応じて自動加工関連のメニュー表示を同期する
    ///
    /// 連続記録中は変換モード・正規表現プリセットメニューを無効化し、連続記録サブメニュー名と
    /// トレイツールチップで自動加工が停止していることを示す
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    pub fn sync_auto_processing_state(&self, state: &AppState) {
        let continuous_record_enabled = state.with_config(|c| c.continuous_record_enabled);

        self.refine
            .main_submenu
            .set_enabled(!continuous_record_enabled);
        self.regex_presets
            .main_submenu
            .set_enabled(!continuous_record_enabled);

        let submenu_title = if continuous_record_enabled {
            "連続記録 (自動加工停止)"
        } else {
            "連続記録"
        };
        self.continuous_record.main_submenu.set_text(submenu_title);

        let tooltip = if continuous_record_enabled {
            "ClipRefiner (連続記録中・自動加工停止)"
        } else {
            "ClipRefiner"
        };
        if let Err(err) = self.tray_icon.set_tooltip(Some(tooltip)) {
            dispatch::log_menu_operation_error("トレイツールチップの更新", err);
        }
    }

    /// クリップボードワーカー再開メニューの有効状態を更新する
    ///
    /// # Arguments
    /// * `enabled` - メニュー項目を有効にするかどうか
    pub fn set_clipboard_worker_retry_enabled(&self, enabled: bool) {
        self.retry_clipboard_worker_item.set_enabled(enabled);
    }
}
