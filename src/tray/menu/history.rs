use std::sync::Mutex;

use super::{HistoryMenu, TrayMenu};

use crate::tray::state::{AppState, LockExt};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

/// 履歴メニュー表示用にテキストを短縮する
pub(crate) fn format_history_menu_label(text: &str) -> String {
    if text.chars().count() > 30 {
        format!("{}...", text.chars().take(27).collect::<String>())
    } else {
        text.to_string()
    }
}

// ======================================================================
// 履歴メニュー
// ======================================================================
impl TrayMenu {
    /// 履歴メニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `history_enabled` - 履歴機能が有効かどうか
    ///
    /// # Returns
    /// 成功した場合は `HistoryMenu` インスタンスを返し、失敗した場合は `Err` を返す
    pub(super) fn build_history_menu(history_enabled: bool) -> Result<HistoryMenu> {
        let enabled_item = CheckMenuItem::new("履歴機能を有効にする", true, history_enabled, None);
        let clear_item = MenuItem::new("履歴をクリア", true, None);
        let main_submenu = Submenu::new("履歴", true);
        let records = Mutex::new(Vec::new());

        // 初期の履歴メニュー構築
        main_submenu.append_items(&[
            &enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &clear_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok(HistoryMenu {
            main_submenu,
            enabled_item,
            clear_item,
            records,
        })
    }
}

// ======================================================================
// 履歴更新
// ======================================================================
impl TrayMenu {
    /// クリップボード履歴リストの内容を現在の状態に合わせて再構築する
    ///
    /// # Arguments
    /// * `state` - 最新の履歴データを持つアプリケーション状態
    ///
    /// # Returns
    /// * `Result<()>` - 更新に成功した場合は `Ok(())` を返す
    pub fn refresh_history(&self, state: &AppState) -> Result<()> {
        let history = state.get_history();
        let mut records = self.history.records.lock_ignore_poison();
        records.clear();

        // 既存の履歴アイテムをクリア(有効化スイッチと区切り線以外)
        for _ in 0..self.history.main_submenu.items().len() {
            self.history.main_submenu.remove_at(0);
        }

        // 基本部分を再構築
        self.history.main_submenu.append_items(&[
            &self.history.enabled_item as &dyn tray_icon::menu::IsMenuItem,
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        // 履歴が空でない場合は、履歴アイテムを追加
        if !history.is_empty() {
            for (index, text) in history.into_iter().enumerate() {
                let label = format_history_menu_label(&text);
                let item = MenuItem::new(label, true, None);
                records.push((item.id().clone(), index));
                self.history
                    .main_submenu
                    .append_items(&[&item as &dyn tray_icon::menu::IsMenuItem])?;
            }
            self.history.main_submenu.append_items(&[
                &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem
            ])?;
        }

        // 「履歴をクリア」項目は常に最後に配置
        self.history
            .main_submenu
            .append_items(&[&self.history.clear_item as &dyn tray_icon::menu::IsMenuItem])?;

        Ok(())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::tray::state::LockExt;

    /// 30 文字以下はそのまま返すこと
    #[test]
    fn format_history_menu_label_within_limit() {
        let text = "あ".repeat(30);
        assert_eq!(format_history_menu_label(&text), text);
    }

    /// 30 文字超は省略記号付きで 30 文字になること
    #[test]
    fn format_history_menu_label_truncates() {
        let text = "あ".repeat(35);
        let label = format_history_menu_label(&text);
        assert!(label.ends_with("..."));
        assert_eq!(label.chars().count(), 30);
    }

    fn build_menu_and_state() -> (TrayMenu, std::sync::Arc<AppState>) {
        let state = std::sync::Arc::new(crate::tray::state::test_app_state());
        state.with_config_mut(|c| c.history_enabled = true);
        let menu = TrayMenu::build(&state).expect("テスト用トレイメニューの構築に失敗");
        (menu, state)
    }

    /// `refresh_history` が履歴件数分のレコードを構築すること
    #[test]
    fn refresh_history_builds_records_for_entries() {
        let (menu, state) = build_menu_and_state();
        state.add_to_history("first entry");
        state.add_to_history("second entry");

        menu.refresh_history(&state)
            .expect("履歴メニューの更新に失敗");

        let records = menu.history.records.lock_ignore_poison();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].1, 0);
        assert_eq!(records[1].1, 1);
        assert_eq!(state.get_history_entry(0).as_deref(), Some("second entry"));
    }

    /// 履歴クリア後の `refresh_history` でレコードが空になること
    #[test]
    fn refresh_history_clears_records_when_history_empty() {
        let (menu, state) = build_menu_and_state();
        state.add_to_history("only");
        menu.refresh_history(&state)
            .expect("履歴メニューの更新に失敗");

        state.clear_history();
        menu.refresh_history(&state)
            .expect("履歴メニューの更新に失敗");

        let records = menu.history.records.lock_ignore_poison();
        assert!(records.is_empty());
    }
}
