use std::collections::HashSet;
use std::sync::Mutex;

use super::{CategoryGroup, RefineMenu, TrayMenu};

use crate::config::HotkeySettings;
use crate::refiner::{RefineCategory, RefineMode};
use crate::tray::state::{AppState, LockExt};

use anyhow::{Context, Result};
use strum::IntoEnumIterator;
use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};

// ======================================================================
// 変換モードメニュー
// ======================================================================
impl TrayMenu {
    /// 変換モードメニューを構築する
    ///
    /// # Arguments
    /// * `current_mode` - 現在選択されている変換モード
    /// * `favorite_modes` - お気に入り登録済みモード
    ///
    /// # Returns
    /// 成功した場合は `RefineMenu` インスタンスを返し、失敗した場合は `Err` を返す
    pub(super) fn build_refine_menu(
        current_mode: RefineMode,
        active_modes: &std::collections::HashSet<RefineMode>,
        favorite_modes: &[RefineMode],
        hotkeys: &HotkeySettings,
    ) -> Result<RefineMenu> {
        use std::collections::HashMap;

        let favorite_set: HashSet<RefineMode> = favorite_modes.iter().copied().collect();
        let mut items_by_category: HashMap<RefineCategory, Vec<(CheckMenuItem, RefineMode)>> =
            HashMap::new();

        for mode in RefineMode::iter().filter(|mode| mode.is_supported_on_current_platform()) {
            let item = CheckMenuItem::new(
                mode_menu_label(mode, favorite_set.contains(&mode)),
                true,
                active_modes.contains(&mode),
                None,
            );
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

        let favorites_submenu = Submenu::new("お気に入り", true);
        let add_favorite_item = MenuItem::new("現在のモードをお気に入りに登録", true, None);
        let remove_favorite_item = MenuItem::new("現在のモードをお気に入りから解除", true, None);
        let favorite_records = Mutex::new(Vec::new());

        // メインの変換モードメニュー組み立て
        let mut mode_menu_items: Vec<&dyn tray_icon::menu::IsMenuItem> =
            vec![&favorites_submenu as &dyn tray_icon::menu::IsMenuItem];

        // カテゴリグループの追加 (遅延配置を除く)
        for group in &groups {
            if !group.category.is_deferred_in_menu() {
                mode_menu_items.push(&group.submenu);
            }
        }

        // ルート直下の通常項目
        for (item, _) in &normal_items {
            mode_menu_items.push(item);
        }

        // 遅延配置カテゴリ (日時変換・数値変換)
        for group in &groups {
            if group.category.is_deferred_in_menu() {
                mode_menu_items.push(&group.submenu);
            }
        }

        let main_submenu = Submenu::with_items("変換モード", true, &mode_menu_items)
            .context("変換モードメニューの作成に失敗しました")?;

        let refine = RefineMenu {
            main_submenu,
            favorites_submenu,
            favorite_records,
            add_favorite_item,
            remove_favorite_item,
            normal_items,
            groups,
        };
        refine.rebuild_favorites(active_modes, favorite_modes, hotkeys)?;
        refine.sync_favorite_actions(current_mode, favorite_modes);
        Ok(refine)
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

    /// お気に入り変換モードの表示を設定内容に合わせて更新する
    pub fn refresh_favorites(&self, state: &AppState) -> Result<()> {
        let (current_mode, active_modes, favorite_modes, hotkeys) = state.with_config(|config| {
            (
                config.mode,
                config
                    .effective_pipeline()
                    .into_iter()
                    .collect::<HashSet<_>>(),
                config.favorite_modes.clone(),
                config.hotkeys.clone(),
            )
        });
        self.refine
            .rebuild_favorites(&active_modes, &favorite_modes, &hotkeys)?;
        self.refine
            .sync_favorite_actions(current_mode, &favorite_modes);
        self.refine.sync_mode_labels(&favorite_modes);
        Ok(())
    }
}

// ======================================================================
// お気に入り変換モードメニュー更新
// ======================================================================
impl RefineMenu {
    /// お気に入りサブメニューの内容を再構築する
    pub fn rebuild_favorites(
        &self,
        active_modes: &HashSet<RefineMode>,
        favorite_modes: &[RefineMode],
        hotkeys: &HotkeySettings,
    ) -> Result<()> {
        let mut records = self.favorite_records.lock_ignore_poison();
        records.clear();

        for _ in 0..self.favorites_submenu.items().len() {
            self.favorites_submenu.remove_at(0);
        }

        if favorite_modes.is_empty() {
            let hint = MenuItem::new("(未登録)", false, None);
            self.favorites_submenu
                .append_items(&[&hint as &dyn tray_icon::menu::IsMenuItem])?;
        } else {
            for (index, mode) in favorite_modes.iter().enumerate() {
                records.push((
                    CheckMenuItem::new(
                        favorite_menu_label(*mode, hotkeys, index),
                        true,
                        active_modes.contains(mode),
                        None,
                    ),
                    *mode,
                ));
            }
            for (item, _) in records.iter() {
                self.favorites_submenu
                    .append_items(&[item as &dyn tray_icon::menu::IsMenuItem])?;
            }
        }

        self.favorites_submenu.append_items(&[
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &self.add_favorite_item as &dyn tray_icon::menu::IsMenuItem,
            &self.remove_favorite_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok(())
    }

    /// お気に入り登録・解除項目の有効状態を同期する
    pub fn sync_favorite_actions(&self, current_mode: RefineMode, favorite_modes: &[RefineMode]) {
        let is_favorite = favorite_modes.contains(&current_mode);
        let at_limit = favorite_modes.len() >= crate::consts::MAX_FAVORITE_MODES;

        self.add_favorite_item
            .set_enabled(!is_favorite && !at_limit);
        self.remove_favorite_item.set_enabled(is_favorite);
    }

    /// 各モード項目のラベルへお気に入り印を反映する
    pub fn sync_mode_labels(&self, favorite_modes: &[RefineMode]) {
        let favorite_set: HashSet<RefineMode> = favorite_modes.iter().copied().collect();

        for (item, mode) in self.all_mode_items() {
            item.set_text(mode_menu_label(*mode, favorite_set.contains(mode)));
        }
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// トレイメニュー表示用の変換モードラベルを生成する
fn mode_menu_label(mode: RefineMode, is_favorite: bool) -> String {
    if is_favorite {
        format!("★ {}", mode.label())
    } else {
        mode.label().to_string()
    }
}

/// お気に入りサブメニュー表示用のラベルを生成する
fn favorite_menu_label(mode: RefineMode, hotkeys: &HotkeySettings, index: usize) -> String {
    if let Some(binding) = hotkeys.favorite_slot_binding(index) {
        format!("{} ({})", mode.label(), binding)
    } else {
        mode.label().to_string()
    }
}
