use crate::tray::menu::{IntervalMenu, MonitorMenu, TrayMenu};

use crate::config::MonitorMode;

use anyhow::{Context, Result};
use tray_icon::menu::{CheckMenuItem, PredefinedMenuItem, Submenu};

/// トレイメニューの監視周期プリセット (ミリ秒)
pub(crate) const INTERVAL_PRESETS_MS: &[u64] = &[500, 1000, 2000, 5000];

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
    /// * `Result<MonitorMenu>` - 成功した場合は `MonitorMenu` インスタンス、失敗した場合は `Err`
    pub(crate) fn build_monitor_menu(current_monitor_mode: MonitorMode) -> Result<MonitorMenu> {
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
    /// * `Result<IntervalMenu>` - 成功した場合は `IntervalMenu` インスタンス、失敗した場合は `Err`
    pub(crate) fn build_interval_menu(
        current_interval: u64,
        monitor_mode: MonitorMode,
    ) -> Result<IntervalMenu> {
        let preset_labels = ["0.5秒", "1秒", "2秒", "5秒"];
        let interval_items: Vec<(CheckMenuItem, u64)> = INTERVAL_PRESETS_MS
            .iter()
            .zip(preset_labels)
            .map(|(&ms, label)| {
                (
                    CheckMenuItem::new(label, true, current_interval == ms, None),
                    ms,
                )
            })
            .collect();

        let is_custom = !is_interval_preset(current_interval);
        let custom_item = CheckMenuItem::new(
            custom_interval_label(current_interval),
            true,
            is_custom,
            None,
        );

        let mut interval_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();
        for (item, _) in &interval_items {
            interval_menu_items.push(item);
        }
        let separator = PredefinedMenuItem::separator();
        interval_menu_items.push(&separator);
        interval_menu_items.push(&custom_item);

        let main_submenu = Submenu::with_items("監視周期", true, &interval_menu_items)
            .context("監視周期メニューの作成に失敗しました")?;

        // イベントモードの場合は監視周期を無効化
        if monitor_mode == MonitorMode::Event {
            main_submenu.set_enabled(false);
        }

        Ok(IntervalMenu {
            main_submenu,
            items: interval_items,
            custom_item,
        })
    }

    /// プリセットとカスタム項目のチェック状態・ラベルを同期する
    ///
    /// # Arguments
    /// * `interval_ms` - 現在のポーリング間隔 (ミリ秒)
    pub(crate) fn sync_interval_menu(&self, interval_ms: u64) {
        let is_custom = !is_interval_preset(interval_ms);
        for (item, ms) in &self.interval.items {
            item.set_checked(*ms == interval_ms);
        }
        self.interval.custom_item.set_checked(is_custom);
        self.interval
            .custom_item
            .set_text(custom_interval_label(interval_ms));
    }
}

/// トレイのプリセット間隔かどうか
///
/// # Arguments
/// * `interval_ms` - 判定する間隔 (ミリ秒)
///
/// # Returns
/// * `bool` - プリセットに含まれる場合は `true`
pub(crate) fn is_interval_preset(interval_ms: u64) -> bool {
    INTERVAL_PRESETS_MS.contains(&interval_ms)
}

/// カスタム周期メニュー項目の表示文言を返す
///
/// # Arguments
/// * `interval_ms` - 現在のポーリング間隔 (ミリ秒)
///
/// # Returns
/// * `String` - プリセット時は `カスタム…`、それ以外は実値付き
fn custom_interval_label(interval_ms: u64) -> String {
    if is_interval_preset(interval_ms) {
        "カスタム…".to_string()
    } else {
        format!("カスタム ({interval_ms} ms)")
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 現在の監視方式だけがチェックされるようにすること
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
