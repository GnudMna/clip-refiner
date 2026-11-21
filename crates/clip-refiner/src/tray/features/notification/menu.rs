use crate::config::NotificationSettings;
use crate::tray::menu::{NotificationMenu, TrayMenu};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, PredefinedMenuItem, Submenu};

// ======================================================================
// 通知メニュー
// ======================================================================
impl TrayMenu {
    /// 通知メニューを構築する
    ///
    /// # Arguments
    /// * `settings` - 通知設定
    ///
    /// # Returns
    /// * `Result<NotificationMenu>` - 成功した場合は `NotificationMenu` インスタンス、失敗した場合は `Err`
    pub(crate) fn build_notification_menu(
        settings: &NotificationSettings,
    ) -> Result<NotificationMenu> {
        let enabled_item =
            CheckMenuItem::new("デスクトップ通知を有効化", true, settings.enabled, None);
        let notify_process_item =
            CheckMenuItem::new("加工完了を通知", true, settings.notify_process, None);
        let notify_pause_item =
            CheckMenuItem::new("一時停止を通知", true, settings.notify_pause, None);
        let notify_history_restore_item = CheckMenuItem::new(
            "履歴復元を通知",
            true,
            settings.notify_history_restore,
            None,
        );
        let notify_ocr_item = CheckMenuItem::new("OCR を通知", true, settings.notify_ocr, None);
        let notify_registered_clip_item = CheckMenuItem::new(
            "登録クリップを通知",
            true,
            settings.notify_registered_clip,
            None,
        );
        let notify_favorites_item =
            CheckMenuItem::new("お気に入りを通知", true, settings.notify_favorites, None);
        let notify_continuous_record_item = CheckMenuItem::new(
            "連続記録を通知",
            true,
            settings.notify_continuous_record,
            None,
        );
        let notify_result_item = CheckMenuItem::new(
            "クリップボードの内容を表示",
            true,
            settings.notify_result,
            None,
        );

        let kinds_submenu = Submenu::with_items(
            "通知の種類",
            true,
            &[
                &notify_process_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_pause_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_history_restore_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_ocr_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_registered_clip_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_favorites_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_continuous_record_item as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        let main_submenu = Submenu::with_items(
            "通知",
            true,
            &[
                &enabled_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &kinds_submenu as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &notify_result_item as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        Ok(NotificationMenu {
            main_submenu,
            enabled_item,
            notify_process_item,
            notify_pause_item,
            notify_history_restore_item,
            notify_ocr_item,
            notify_registered_clip_item,
            notify_favorites_item,
            notify_continuous_record_item,
            notify_result_item,
        })
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 通知メニューが各チェック状態を設定どおり反映すること
    #[test]
    fn build_notification_menu_reflects_settings() {
        let mut settings = NotificationSettings::default();
        settings.enabled = false;
        settings.notify_process = true;
        settings.notify_pause = false;
        settings.notify_history_restore = true;
        settings.notify_ocr = false;
        settings.notify_registered_clip = true;
        settings.notify_favorites = false;
        settings.notify_continuous_record = true;
        settings.notify_result = true;
        let notification =
            TrayMenu::build_notification_menu(&settings).expect("通知メニューの構築に失敗");

        assert!(!notification.enabled_item.is_checked());
        assert!(notification.notify_process_item.is_checked());
        assert!(!notification.notify_pause_item.is_checked());
        assert!(notification.notify_history_restore_item.is_checked());
        assert!(!notification.notify_ocr_item.is_checked());
        assert!(notification.notify_registered_clip_item.is_checked());
        assert!(!notification.notify_favorites_item.is_checked());
        assert!(notification.notify_continuous_record_item.is_checked());
        assert!(notification.notify_result_item.is_checked());
    }
}
