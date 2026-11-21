use std::sync::Mutex;

use crate::tray::menu::{ContinuousRecordMenu, TrayMenu};

use crate::security::format_public_snippet;
use crate::tray::state::{AppState, LockExt};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

/// 連続記録メニュー表示用スニペットの最大文字数
const RECORD_MENU_SNIPPET_MAX_CHARS: usize = 30;

/// 連続記録メニュー表示用にテキストを短縮する
pub(crate) fn format_record_menu_label(index: usize, text: &str) -> String {
    format!(
        "{}. {}",
        index + 1,
        format_public_snippet(text, RECORD_MENU_SNIPPET_MAX_CHARS)
    )
}

// ======================================================================
// 連続記録メニュー
// ======================================================================
impl TrayMenu {
    /// 連続記録メニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `continuous_record_enabled` - 連続記録が有効かどうか
    ///
    /// # Returns
    /// * `Result<ContinuousRecordMenu>` - 成功した場合は `ContinuousRecordMenu` インスタンス、失敗した場合は `Err`
    pub(crate) fn build_continuous_record_menu(
        continuous_record_enabled: bool,
    ) -> Result<ContinuousRecordMenu> {
        let enabled_item = CheckMenuItem::new(
            "連続記録を有効にする",
            true,
            continuous_record_enabled,
            None,
        );
        let clear_item = MenuItem::new("記録をクリア", true, None);
        let main_submenu = Submenu::new("連続記録", true);
        let records = Mutex::new(Vec::new());

        main_submenu.append_items(&[
            &enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &clear_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok(ContinuousRecordMenu {
            main_submenu,
            enabled_item,
            clear_item,
            records,
        })
    }
}

// ======================================================================
// 連続記録更新
// ======================================================================
impl TrayMenu {
    /// 連続記録リストの内容を現在の状態に合わせて再構築する
    ///
    /// # Arguments
    /// * `state` - 最新の連続記録データを持つアプリケーション状態
    ///
    /// # Returns
    /// * `Result<()>` - 更新に成功した場合は `Ok(())` を返す
    pub fn refresh_continuous_record(&self, state: &AppState) -> Result<()> {
        let record_len = state.continuous_record_len();
        let mut records = self.continuous_record.records.lock_ignore_poison();
        records.clear();

        for _ in 0..self.continuous_record.main_submenu.items().len() {
            self.continuous_record.main_submenu.remove_at(0);
        }

        self.continuous_record.main_submenu.append_items(&[
            &self.continuous_record.enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        if record_len > 0 {
            for index in 0..record_len {
                let Some(text) = state.get_continuous_record_entry(index) else {
                    continue;
                };
                let label = format_record_menu_label(index, &text);
                let item = MenuItem::new(label, true, None);
                records.push((item.id().clone(), index));
                self.continuous_record
                    .main_submenu
                    .append_items(&[&item as &dyn tray_icon::menu::IsMenuItem])?;
            }
            self.continuous_record.main_submenu.append_items(&[
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            ])?;
        }

        self.continuous_record.main_submenu.append_items(&[
            &self.continuous_record.clear_item as &dyn tray_icon::menu::IsMenuItem
        ])?;

        Ok(())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::consts::SENSITIVE_SNIPPET_LABEL;
    use crate::tray::state::LockExt;

    /// 30 文字以下は番号付きでそのまま返すようにすること
    #[test]
    fn format_record_menu_label_within_limit() {
        let text = "あ".repeat(30);
        assert_eq!(format_record_menu_label(0, &text), format!("1. {text}"));
    }

    /// 30 文字超は省略記号付きで 30 文字になるようにすること
    #[test]
    fn format_record_menu_label_truncates() {
        let text = "あ".repeat(35);
        let label = format_record_menu_label(1, &text);
        assert!(label.starts_with("2. "));
        assert!(label.ends_with("..."));
        assert_eq!(label.chars().count(), 33);
    }

    /// 機密らしい内容はマスクすること
    #[test]
    fn format_record_menu_label_masks_sensitive() {
        let label = format_record_menu_label(0, "password=hunter2");
        assert_eq!(label, format!("1. {SENSITIVE_SNIPPET_LABEL}"));
    }

    fn build_menu_and_state() -> (TrayMenu, std::sync::Arc<AppState>) {
        let state = std::sync::Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.continuous_record_enabled = true);
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        (menu, state)
    }

    /// `refresh_continuous_record` が記録件数分のレコードを構築すること
    #[test]
    fn refresh_continuous_record_builds_records_for_entries() {
        let (menu, state) = build_menu_and_state();
        state.enqueue_continuous_record("first entry");
        state.enqueue_continuous_record("second entry");

        menu.refresh_continuous_record(&state)
            .expect("連続記録メニューの更新に失敗");

        let records = menu.continuous_record.records.lock_ignore_poison();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].1, 0);
        assert_eq!(records[1].1, 1);
        assert_eq!(
            state
                .get_continuous_record_entry(0)
                .as_ref()
                .map(|s| s.as_str()),
            Some("first entry")
        );
    }

    /// 記録クリア後の `refresh_continuous_record` でレコードが空になるようにすること
    #[test]
    fn refresh_continuous_record_clears_records_when_empty() {
        let (menu, state) = build_menu_and_state();
        state.enqueue_continuous_record("only");
        menu.refresh_continuous_record(&state)
            .expect("連続記録メニューの更新に失敗");

        state.clear_continuous_record();
        menu.refresh_continuous_record(&state)
            .expect("連続記録メニューの更新に失敗");

        let records = menu.continuous_record.records.lock_ignore_poison();
        assert!(records.is_empty());
    }
}
