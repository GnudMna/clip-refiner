use super::{NotificationMenu, TrayMenu};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, PredefinedMenuItem, Submenu};

// ======================================================================
// 通知メニュー
// ======================================================================
impl TrayMenu {
    /// 通知メニューを構築する
    ///
    /// # Arguments
    /// * `enabled` - 成功通知を有効にするかどうか
    /// * `notify_mode` - 通知にモード変化を表示するかどうか
    /// * `notify_result` - 通知にクリップボードの内容を表示するかどうか
    /// * `notify_pause` - 一時停止切替を通知するかどうか
    ///
    /// # Returns
    /// 成功した場合は `NotificationMenu` インスタンスを返し、失敗した場合は `Err` を返す
    #[allow(clippy::fn_params_excessive_bools)]
    pub(super) fn build_notification_menu(
        enabled: bool,
        notify_mode: bool,
        notify_result: bool,
        notify_pause: bool,
    ) -> Result<NotificationMenu> {
        let enabled_item = CheckMenuItem::new("成功通知を有効化", true, enabled, None);
        let notify_mode_item = CheckMenuItem::new("モード変更を通知", true, notify_mode, None);
        let notify_result_item =
            CheckMenuItem::new("クリップボードの内容を表示", true, notify_result, None);
        let notify_pause_item = CheckMenuItem::new("一時停止を通知", true, notify_pause, None);

        let content_submenu = Submenu::with_items(
            "通知内容",
            true,
            &[
                &notify_mode_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_result_item as &dyn tray_icon::menu::IsMenuItem,
                &notify_pause_item as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        let main_submenu = Submenu::with_items(
            "通知",
            true,
            &[
                &enabled_item as &dyn tray_icon::menu::IsMenuItem,
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
                &content_submenu as &dyn tray_icon::menu::IsMenuItem,
            ],
        )?;

        Ok(NotificationMenu {
            main_submenu,
            enabled_item,
            notify_mode_item,
            notify_result_item,
            notify_pause_item,
        })
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 成功通知 OFF 時も通知内容サブメニューは構築でき、各チェック状態が引数どおり反映されること
    #[test]
    fn build_notification_menu_creates_content_submenu() {
        let notification = TrayMenu::build_notification_menu(false, true, true, true)
            .expect("通知メニューの構築に失敗");
        assert!(!notification.enabled_item.is_checked());
        assert!(notification.notify_mode_item.is_checked());
        assert!(notification.notify_result_item.is_checked());
        assert!(notification.notify_pause_item.is_checked());
    }
}
