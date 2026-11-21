//! システムトレイのメニュー構造体と構築ロジックを提供するモジュール
//!
//! 変換モード (正規表現プリセットを含む)・登録クリップ・履歴・連続記録などのサブメニューを組み立てる

mod build;
pub(crate) mod icon;
mod rebuild;
mod sync;
mod types;

pub use icon::create_window_icon;
pub(crate) use rebuild::{
    DYNAMIC_RECORD_MENU_SNIPPET_MAX_CHARS, escape_menu_text, rebuild_toggle_records_submenu,
};
pub use types::{
    CategoryGroup, ClipsMenu, ContinuousRecordMenu, HistoryMenu, RefineMenu, RegexPresetMenu,
    TrayMenu,
};

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tray_icon::{
        TrayIconBuilder,
        menu::{MenuItem, Submenu},
    };

    use super::icon::create_icon;
    use super::*;
    use crate::config::HotkeySettings;
    use crate::refiner::{RefineCategory, RefineMode};

    /// 変換モードメニューに全モードが含まれるようにすること
    #[test]
    fn build_refine_menu_contains_all_modes() {
        use std::collections::HashSet;

        let active = HashSet::from([RefineMode::Trim]);
        let refine =
            TrayMenu::build_refine_menu(RefineMode::Trim, &active, &[], &HotkeySettings::default())
                .expect("変換モードメニューの構築に失敗");
        let modes: Vec<_> = refine.all_mode_items().map(|(_, mode)| *mode).collect();
        assert_eq!(modes.len(), 55);
        assert!(
            refine
                .all_mode_items()
                .any(|(item, mode)| *mode == RefineMode::Trim && item.is_checked())
        );
    }

    /// 選択カテゴリのサブメニューにチェックプレフィックスが付くようにすること
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
            refine,
            history: TrayMenu::build_history_menu(false).expect("履歴メニューの構築に失敗"),
            continuous_record: TrayMenu::build_continuous_record_menu(false)
                .expect("連続記録メニューの構築に失敗"),
            clips: ClipsMenu {
                main_submenu: Submenu::new("登録クリップ", true),
                records: Mutex::new(Vec::new()),
                empty_hint_item: MenuItem::new("(未登録)", false, None),
                register_item: MenuItem::new("クリップボードを登録", true, None),
            },
            regex_presets: RegexPresetMenu {
                main_submenu: Submenu::new("正規表現プリセット", true),
                records: Mutex::new(Vec::new()),
                empty_hint_item: MenuItem::new("(未登録)", false, None),
            },
            quick_selector_item: MenuItem::new("クイック変換…", true, None),
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

    /// 自動変換の切り替えと変換モード変更が変換モードのサブメニュー名へ反映されるようにすること
    #[test]
    fn refine_submenu_title_follows_auto_refine_and_mode() {
        let ctx = crate::test_helpers::TrayTestContext::with_config(|c| {
            c.auto_refine_enabled = false;
            c.mode = RefineMode::Trim;
        });
        let trim = RefineMode::Trim.label();
        assert_eq!(
            ctx.menu().refine.main_submenu.text(),
            format!("変換モード: {trim} (オフ)")
        );

        crate::tray::event::set_auto_refine_enabled(ctx.state(), ctx.menu(), true, None);
        assert_eq!(
            ctx.menu().refine.main_submenu.text(),
            format!("変換モード: {trim} (オン)")
        );

        crate::tray::event::update_refine(
            ctx.state(),
            ctx.menu(),
            ctx.worker(),
            RefineMode::JsonFormat,
            None,
        );
        assert_eq!(
            ctx.menu().refine.main_submenu.text(),
            format!("変換モード: {} (オン)", RefineMode::JsonFormat.label())
        );
    }

    /// 連続記録有効時は正規表現プリセットメニューだけが無効になり、変換モードメニューは操作できるようにすること
    #[test]
    fn sync_auto_processing_state_disables_regex_presets_when_continuous_record_enabled() {
        let ctx = crate::test_helpers::TrayTestContext::with_config(|c| {
            c.continuous_record_enabled = true;
        });

        assert!(ctx.menu().refine.main_submenu.is_enabled());
        assert!(!ctx.menu().regex_presets.main_submenu.is_enabled());
        assert_eq!(
            ctx.menu().continuous_record.main_submenu.text(),
            "連続記録モード (オン)"
        );
    }

    /// 連続記録無効時は変換モード・正規表現プリセットメニューが有効になるようにすること
    #[test]
    fn sync_auto_processing_state_enables_refine_when_continuous_record_disabled() {
        let ctx = crate::test_helpers::TrayTestContext::with_config(|c| {
            c.continuous_record_enabled = false;
        });

        assert!(ctx.menu().refine.main_submenu.is_enabled());
        assert!(ctx.menu().regex_presets.main_submenu.is_enabled());
        assert_eq!(
            ctx.menu().continuous_record.main_submenu.text(),
            "連続記録モード (オフ)"
        );
    }
}
