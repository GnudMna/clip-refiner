//! システムトレイのメニュー構造体と構築ロジックを提供するモジュール
//!
//! 変換モード、監視設定、登録クリップ、履歴、通知などのサブメニューを組み立てる

mod build;
pub(crate) mod icon;
mod sync;
mod types;

pub use icon::create_window_icon;
pub use types::{
    CategoryGroup, ClipsMenu, ContinuousRecordMenu, HistoryMenu, IntervalMenu, MonitorMenu,
    NotificationMenu, RefineMenu, TrayMenu,
};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::icon::create_icon;
    use super::*;

    use crate::config::{HotkeySettings, MonitorMode, NotificationSettings};
    use crate::refiner::{RefineCategory, RefineMode};

    use tray_icon::{
        TrayIconBuilder,
        menu::{CheckMenuItem, MenuItem, Submenu},
    };

    /// 変換モードメニューに全モードが含まれるようにすること
    #[test]
    fn build_refine_menu_contains_all_modes() {
        use std::collections::HashSet;

        let active = HashSet::from([RefineMode::Trim]);
        let refine =
            TrayMenu::build_refine_menu(RefineMode::Trim, &active, &[], &HotkeySettings::default())
                .expect("変換モードメニューの構築に失敗");
        let modes: Vec<_> = refine.all_mode_items().map(|(_, mode)| *mode).collect();
        assert_eq!(modes.len(), 42);
        assert!(
            refine
                .all_mode_items()
                .any(|(item, mode)| *mode == RefineMode::Trim && item.is_checked())
        );
    }

    /// 選択カテゴリのサブメニューにチェックプレフィックスが付くとすること
    #[test]
    fn refresh_category_labels_marks_current_category() {
        use std::collections::HashSet;

        let active = HashSet::from([RefineMode::UrlEncode]);
        let refine = TrayMenu::build_refine_menu(
            RefineMode::UrlEncode,
            &active,
            &[],
            &HotkeySettings::default(),
        )
        .expect("変換モードメニューの構築に失敗");
        let tray = TrayMenu {
            tray_icon: TrayIconBuilder::new()
                .with_icon(create_icon().expect("アイコンの作成に失敗"))
                .build()
                .expect("トレイアイコンのビルドに失敗"),
            quit_item: MenuItem::new("終了", true, None),
            pause_item: CheckMenuItem::new("一時停止", true, false, None),
            refine,
            monitor: TrayMenu::build_monitor_menu(MonitorMode::Polling)
                .expect("監視方式メニューの構築に失敗"),
            interval: TrayMenu::build_interval_menu(1000, MonitorMode::Polling)
                .expect("監視周期メニューの構築に失敗"),
            history: TrayMenu::build_history_menu(false).expect("履歴メニューの構築に失敗"),
            continuous_record: TrayMenu::build_continuous_record_menu(false)
                .expect("連続記録メニューの構築に失敗"),
            clips: ClipsMenu {
                main_submenu: Submenu::new("登録クリップ", true),
                records: Mutex::new(Vec::new()),
                empty_hint_item: MenuItem::new("(未登録)", false, None),
                register_item: MenuItem::new("クリップボードを登録", true, None),
            },
            notification: TrayMenu::build_notification_menu(&NotificationSettings::default())
                .expect("通知メニューの構築に失敗"),
            open_settings_window_item: MenuItem::new("設定ウィンドウ", true, None),
            retry_clipboard_worker_item: MenuItem::new("クリップボード監視を再開", true, None),
        };

        tray.refresh_category_labels(RefineMode::UrlEncode);

        let url_group = tray
            .refine
            .groups
            .iter()
            .find(|g| g.category == RefineCategory::UrlActions)
            .expect("URL カテゴリが存在する");
        assert!(url_group.submenu.text().starts_with('✓'));
    }

    /// イベント監視時は監視周期メニューが無効になるようにすること
    #[test]
    fn build_interval_menu_disabled_for_event_mode() {
        let interval = TrayMenu::build_interval_menu(1000, MonitorMode::Event)
            .expect("監視周期メニューの構築に失敗");
        assert!(!interval.main_submenu.is_enabled());
    }

    /// ポーリング監視時は現在の間隔だけがチェックされるようにすること
    #[test]
    fn build_interval_menu_checks_current_interval() {
        let interval = TrayMenu::build_interval_menu(2000, MonitorMode::Polling)
            .expect("監視周期メニューの構築に失敗");
        assert!(interval.main_submenu.is_enabled());
        assert!(
            interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 2000 && item.is_checked())
        );
        assert!(
            interval
                .items
                .iter()
                .any(|(item, ms)| *ms == 1000 && !item.is_checked())
        );
    }

    /// 通知メニューの有効状態が初期値を反映すること
    #[test]
    fn build_notification_menu_reflects_settings() {
        let mut settings = NotificationSettings::default();
        settings.enabled = true;
        settings.notify_process = false;
        settings.notify_result = true;
        settings.notify_pause = false;
        let notification =
            TrayMenu::build_notification_menu(&settings).expect("通知メニューの構築に失敗");
        assert!(notification.enabled_item.is_checked());
        assert!(!notification.notify_process_item.is_checked());
        assert!(notification.notify_result_item.is_checked());
        assert!(!notification.notify_pause_item.is_checked());
    }

    /// 連続記録有効時は変換モードメニューが無効になるようにすること
    #[test]
    fn sync_auto_processing_state_disables_refine_when_continuous_record_enabled() {
        let state = crate::tray::state::test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");

        assert!(!menu.refine.main_submenu.is_enabled());
        assert_eq!(
            menu.continuous_record.main_submenu.text(),
            "連続記録 (自動加工停止)"
        );
    }

    /// 連続記録無効時は変換モードメニューが有効になるようにすること
    #[test]
    fn sync_auto_processing_state_enables_refine_when_continuous_record_disabled() {
        let state = crate::tray::state::test_app_state();
        state.with_config_mut(|c| c.continuous_record_enabled = false);
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");

        assert!(menu.refine.main_submenu.is_enabled());
        assert_eq!(menu.continuous_record.main_submenu.text(), "連続記録");
    }
}
