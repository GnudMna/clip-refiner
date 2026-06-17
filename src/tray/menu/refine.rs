use crate::refiner::{RefineCategory, RefineMode};

use anyhow::{Context, Result};
use strum::IntoEnumIterator;
use tray_icon::menu::{CheckMenuItem, Submenu};

use super::{CategoryGroup, RefineMenu, TrayMenu};

// ======================================================================
// 変換モードメニュー
// ======================================================================
impl TrayMenu {
    /// 変換モードメニューを構築する
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード
    ///
    /// # Returns
    /// 成功した場合は `RefineMenu` インスタンスを返し、失敗した場合は `Err` を返す
    pub(super) fn build_refine_menu(current_mode: RefineMode) -> Result<RefineMenu> {
        use std::collections::HashMap;

        let mut items_by_category: HashMap<RefineCategory, Vec<(CheckMenuItem, RefineMode)>> =
            HashMap::new();

        for mode in RefineMode::iter() {
            let item = CheckMenuItem::new(mode.label(), true, mode == current_mode, None);
            items_by_category
                .entry(mode.category())
                .or_default()
                .push((item, mode));
        }

        let normal_items = items_by_category
            .remove(&RefineCategory::Normal)
            .unwrap_or_default();

        // サブメニューの順序
        let category_order = RefineCategory::SUBMENU_ORDER;

        let mut groups = Vec::new();
        for &category in &category_order {
            if let Some(items) = items_by_category.remove(&category) {
                let submenu = Submenu::with_items(
                    category.label(),
                    true,
                    &items
                        .iter()
                        .map(|(i, _)| i as &dyn tray_icon::menu::IsMenuItem)
                        .collect::<Vec<_>>(),
                )?;
                groups.push(CategoryGroup {
                    submenu,
                    items,
                    category,
                });
            }
        }

        // メインの変換モードメニュー組み立て
        let mut mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> = Vec::new();

        // カテゴリグループの追加
        for group in &groups {
            if !group.category.is_deferred_in_menu() {
                mode_menu_items.push(&group.submenu);
            }
        }

        // 通常アイテムと遅延追加カテゴリの配置
        for (item, mode) in &normal_items {
            mode_menu_items.push(item);
            if *mode == RefineMode::ExcelToMarkdown {
                for group in &groups {
                    if group.category.is_deferred_in_menu() {
                        mode_menu_items.push(&group.submenu);
                    }
                }
            }
        }

        let main_submenu = Submenu::with_items("変換モード", true, &mode_menu_items)
            .context("変換モードメニューの作成に失敗しました")?;

        Ok(RefineMenu {
            main_submenu,
            normal_items,
            groups,
        })
    }

    /// 所属カテゴリに基づいてサブメニューのラベルを更新する
    ///
    /// 選択されているモードが属するカテゴリのサブメニューに「✓」プレフィックスを付与し、
    /// それ以外のサブメニューからは削除する
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード
    pub fn refresh_category_labels(&self, current_mode: RefineMode) {
        let current_category = current_mode.category();

        for group in &self.refine.groups {
            let prefix = if current_category == group.category {
                "✓"
            } else {
                ""
            };
            group
                .submenu
                .set_text(format!("{}{}", prefix, group.category.label()));
        }
    }
}
