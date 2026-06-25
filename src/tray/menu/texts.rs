use std::sync::Mutex;

use super::{TextsMenu, TrayMenu};

use crate::tray::state::{AppState, LockExt};

use anyhow::Result;
use tray_icon::menu::{MenuItem, PredefinedMenuItem, Submenu};

// ======================================================================
// 登録文字列メニュー
// ======================================================================
impl TrayMenu {
    /// 登録文字列メニューの基本構造を構築する
    pub(super) fn build_texts_menu(state: &AppState) -> Result<TextsMenu> {
        let main_submenu = Submenu::new("登録文字列", true);
        let records = Mutex::new(Vec::new());
        let empty_hint_item = MenuItem::new("(未登録)", false, None);
        let register_item = MenuItem::new("クリップボードを登録", true, None);

        let texts_menu = TextsMenu {
            main_submenu,
            records,
            empty_hint_item,
            register_item,
        };

        texts_menu.rebuild(state)?;
        Ok(texts_menu)
    }

    /// 登録文字列メニューを設定内容に合わせて更新する
    pub fn refresh_texts(&self, state: &AppState) -> Result<()> {
        self.texts.rebuild(state)
    }
}

// ======================================================================
// 登録文字列メニュー更新
// ======================================================================
impl TextsMenu {
    /// 登録文字列リストの内容を現在の設定に合わせて再構築する
    pub fn rebuild(&self, state: &AppState) -> Result<()> {
        let entries: Vec<(String, usize)> = state.with_config(|config| {
            config
                .texts
                .iter()
                .enumerate()
                .map(|(index, entry)| (entry.label.clone(), index))
                .collect()
        });

        let mut records = self.records.lock_ignore_poison();
        records.clear();

        for _ in 0..self.main_submenu.items().len() {
            self.main_submenu.remove_at(0);
        }

        if entries.is_empty() {
            self.main_submenu
                .append_items(&[&self.empty_hint_item as &dyn tray_icon::menu::IsMenuItem])?;
        } else {
            for (label, index) in entries {
                let item = MenuItem::new(label, true, None);
                records.push((item.id().clone(), index));
                self.main_submenu
                    .append_items(&[&item as &dyn tray_icon::menu::IsMenuItem])?;
            }
        }

        self.main_submenu.append_items(&[
            &PredefinedMenuItem::separator() as &dyn tray_icon::menu::IsMenuItem,
            &self.register_item as &dyn tray_icon::menu::IsMenuItem,
        ])?;

        Ok(())
    }
}
