use std::sync::Mutex;

use crate::tray::menu::{HistoryMenu, TrayMenu};

use crate::security::format_public_snippet;
use crate::tray::state::{AppState, LockExt};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

/// 履歴メニュー表示用スニペットの最大文字数
const HISTORY_MENU_SNIPPET_MAX_CHARS: usize = 30;

/// 履歴メニュー表示用にテキストを短縮する
pub(crate) fn format_history_menu_label(text: &str) -> String {
    format_public_snippet(text, HISTORY_MENU_SNIPPET_MAX_CHARS)
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
    /// * `Result<HistoryMenu>` - 成功した場合は `HistoryMenu` インスタンス、失敗した場合は `Err`
    pub(crate) fn build_history_menu(history_enabled: bool) -> Result<HistoryMenu> {
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
        let history_len = state.clipboard_session().history_len();
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
        if history_len > 0 {
            for index in 0..history_len {
                let Some(text) = state.clipboard_session().get_history_entry(index) else {
                    continue;
                };
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

    use crate::consts::SENSITIVE_SNIPPET_LABEL;
    use crate::tray::state::LockExt;

    /// 30 文字以下はそのまま返すようにすること
    #[test]
    fn format_history_menu_label_within_limit() {
        let text = "あ".repeat(30);
        assert_eq!(format_history_menu_label(&text), text);
    }

    /// 30 文字超は省略記号付きで 30 文字になるようにすること
    #[test]
    fn format_history_menu_label_truncates() {
        let text = "あ".repeat(35);
        let label = format_history_menu_label(&text);
        assert!(label.ends_with("..."));
        assert_eq!(label.chars().count(), 30);
    }

    /// 機密らしい内容はマスクすること
    #[test]
    fn format_history_menu_label_masks_sensitive() {
        let label = format_history_menu_label("password=hunter2");
        assert_eq!(label, SENSITIVE_SNIPPET_LABEL);
    }

    fn build_menu_and_state() -> crate::test_helpers::TrayTestContext {
        crate::test_helpers::TrayTestContext::with_config(|c| c.history_enabled = true)
    }

    /// `refresh_history` が履歴件数分のレコードを構築すること
    #[test]
    fn refresh_history_builds_records_for_entries() {
        let ctx = build_menu_and_state();
        ctx.state().add_to_history("first entry");
        ctx.state().add_to_history("second entry");

        ctx.menu()
            .refresh_history(ctx.state())
            .expect("履歴メニューの更新に失敗");

        let records = ctx.menu().history.records.lock_ignore_poison();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].1, 0);
        assert_eq!(records[1].1, 1);
        assert_eq!(
            ctx.state()
                .clipboard_session()
                .get_history_entry(0)
                .as_ref()
                .map(|s| s.as_str()),
            Some("second entry")
        );
    }

    /// 履歴クリア後の `refresh_history` でレコードが空になるようにすること
    #[test]
    fn refresh_history_clears_records_when_history_empty() {
        let ctx = build_menu_and_state();
        ctx.state().add_to_history("only");
        ctx.menu()
            .refresh_history(ctx.state())
            .expect("履歴メニューの更新に失敗");

        ctx.state().clipboard_session().clear_history();
        ctx.menu()
            .refresh_history(ctx.state())
            .expect("履歴メニューの更新に失敗");

        let records = ctx.menu().history.records.lock_ignore_poison();
        assert!(records.is_empty());
    }
}
