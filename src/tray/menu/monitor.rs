use super::{IntervalMenu, MonitorMenu, TrayMenu};

use crate::config::MonitorMode;

use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, Submenu};

// ======================================================================
// 監視方式メニュー
// ======================================================================
impl TrayMenu {
    /// 監視方式メニューを構築する
    ///
    /// # Arguments
    /// * `current_monitor_mode` - 現在選択されている監視方式
    ///
    /// # Returns
    /// 成功した場合は `MonitorMenu` インスタンスを返し、失敗した場合は `Err` を返す
    pub(super) fn build_monitor_menu(current_monitor_mode: MonitorMode) -> Result<MonitorMenu> {
        let polling_item = CheckMenuItem::new(
            "ポーリング",
            true,
            current_monitor_mode == MonitorMode::Polling,
            None,
        );

        let event_item = CheckMenuItem::new(
            "イベント",
            true,
            current_monitor_mode == MonitorMode::Event,
            None,
        );

        let monitor_mode_items = vec![
            (polling_item, MonitorMode::Polling),
            (event_item, MonitorMode::Event),
        ];

        let mut monitor_mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &monitor_mode_items {
            monitor_mode_menu_items.push(item);
        }
        let main_submenu = Submenu::with_items("監視方式", true, &monitor_mode_menu_items)
            .context("監視方式メニューの作成に失敗しました")?;

        Ok(MonitorMenu {
            main_submenu,
            items: monitor_mode_items,
        })
    }
}

// ======================================================================
// 監視周期メニュー
// ======================================================================
impl TrayMenu {
    /// 監視周期メニューを構築する
    ///
    /// # Arguments
    /// * `current_interval` - 現在設定されている監視間隔(ミリ秒)
    /// * `monitor_mode` - 現在の監視方式(イベントモード時はメニューを無効化するため)
    ///
    /// # Returns
    /// 成功した場合は `IntervalMenu` インスタンスを返し、失敗した場合は `Err` を返す
    pub(super) fn build_interval_menu(
        current_interval: u64,
        monitor_mode: MonitorMode,
    ) -> Result<IntervalMenu> {
        let interval_items = vec![
            (
                CheckMenuItem::new("0.5秒", true, current_interval == 500, None),
                500u64,
            ),
            (
                CheckMenuItem::new("1秒", true, current_interval == 1000, None),
                1000u64,
            ),
            (
                CheckMenuItem::new("2秒", true, current_interval == 2000, None),
                2000u64,
            ),
            (
                CheckMenuItem::new("5秒", true, current_interval == 5000, None),
                5000u64,
            ),
        ];

        let mut interval_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &interval_items {
            interval_menu_items.push(item);
        }
        let main_submenu = Submenu::with_items("監視周期", true, &interval_menu_items)
            .context("監視周期メニューの作成に失敗しました")?;

        // イベントモードの場合は監視周期を無効化
        if monitor_mode == MonitorMode::Event {
            main_submenu.set_enabled(false);
        }

        Ok(IntervalMenu {
            main_submenu,
            items: interval_items,
        })
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 現在の監視方式だけがチェックされること
    #[test]
    fn build_monitor_menu_checks_current_mode() {
        let monitor =
            TrayMenu::build_monitor_menu(MonitorMode::Event).expect("監視方式メニューの構築に失敗");
        assert!(
            monitor
                .items
                .iter()
                .any(|(item, mode)| *mode == MonitorMode::Event && item.is_checked())
        );
        assert!(
            monitor
                .items
                .iter()
                .any(|(item, mode)| *mode == MonitorMode::Polling && !item.is_checked())
        );
    }
}
