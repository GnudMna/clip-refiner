use std::sync::Mutex;

use crate::tray::state::{AppState, LockExt};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

use super::{HistoryMenu, TrayMenu};

// ======================================================================
// 履歴メニュー
// ======================================================================
impl TrayMenu {
    /// 履歴メニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `history_enabled` - 履歴機能が有効かどうか。
    ///
    /// # Returns
    /// 成功した場合は `HistoryMenu` インスタンスを返し、失敗した場合は `Err` を返す。
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
    /// * `Result<()>` - 更新に成功した場合は `Ok(())` を返します。
    pub fn refresh_history(&self, state: &AppState) -> Result<()> {
        let history = state.get_history();
        let mut records = self.history.records.lock_ignore_poison();
        records.clear();

        // 既存の履歴アイテムをクリア（有効化スイッチと区切り線以外）
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
            for text in history {
                let label = if text.chars().count() > 30 {
                    format!("{}...", text.chars().take(27).collect::<String>())
                } else {
                    text.clone()
                };
                let item = MenuItem::new(label, true, None);
                records.push((item.id().clone(), text));
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
