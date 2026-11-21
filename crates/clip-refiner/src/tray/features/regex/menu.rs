use std::sync::Mutex;

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, MenuItem, Submenu};

use crate::tray::dispatch;
use crate::tray::menu::{RegexPresetMenu, TrayMenu, escape_menu_text};
use crate::tray::state::{AppState, LockExt};

// ======================================================================
// 正規表現プリセットメニュー
// ======================================================================
impl TrayMenu {
    /// 正規表現プリセットメニューの基本構造を構築する
    ///
    /// # Arguments
    /// * `active_preset_id` - 有効なプリセット ID
    /// * `presets` - プリセット一覧 (`id`, `name`)
    ///
    /// # Returns
    /// * `Result<RegexPresetMenu>` - 構築したメニュー
    pub(crate) fn build_regex_preset_menu(
        active_preset_id: &str,
        presets: &[(String, String)],
    ) -> Result<RegexPresetMenu> {
        let main_submenu = Submenu::new("正規表現プリセット", true);
        let empty_hint_item = MenuItem::new("(未登録)", false, None);
        let menu = RegexPresetMenu {
            main_submenu,
            records: Mutex::new(Vec::new()),
            empty_hint_item,
        };
        menu.rebuild(active_preset_id, presets)?;
        Ok(menu)
    }

    /// 正規表現プリセットメニューを設定内容に合わせて更新する
    ///
    /// # Arguments
    /// * `state` - アプリケーションの共有状態
    ///
    /// # Returns
    /// * `Result<()>` - 成功時は `Ok(())`
    pub fn refresh_regex_presets(&self, state: &AppState) -> Result<()> {
        let snap = state.menu_snapshot();
        self.regex_presets
            .rebuild(&snap.regex_active_preset_id, &snap.regex_presets)
    }
}

// ======================================================================
// 正規表現プリセットメニュー更新
// ======================================================================
impl RegexPresetMenu {
    /// プリセット一覧を現在の設定に合わせて再構築する
    ///
    /// # Arguments
    /// * `active_preset_id` - 有効なプリセット ID
    /// * `presets` - プリセット一覧 (`id`, `name`)
    ///
    /// # Returns
    /// * `Result<()>` - 成功時は `Ok(())`
    pub fn rebuild(&self, active_preset_id: &str, presets: &[(String, String)]) -> Result<()> {
        let mut records = self.records.lock_ignore_poison();
        records.clear();

        for _ in 0..self.main_submenu.items().len() {
            self.main_submenu.remove_at(0);
        }

        if presets.is_empty() {
            self.main_submenu
                .append_items(&[&self.empty_hint_item as &dyn tray_icon::menu::IsMenuItem])?;
            return Ok(());
        }

        for (id, name) in presets {
            let label = if name.is_empty() {
                "(無題)"
            } else {
                name.as_str()
            };
            let item =
                CheckMenuItem::new(escape_menu_text(label), true, id == active_preset_id, None);
            records.push((item, id.clone()));
        }
        for (item, _) in records.iter() {
            self.main_submenu
                .append_items(&[item as &dyn tray_icon::menu::IsMenuItem])?;
        }

        Ok(())
    }

    /// チェック状態だけを有効プリセットへ同期する
    ///
    /// # Arguments
    /// * `active_preset_id` - 有効なプリセット ID
    pub fn sync_checked(&self, active_preset_id: &str) {
        for (item, id) in self.records.lock_ignore_poison().iter() {
            item.set_checked(id == active_preset_id);
        }
    }
}

impl TrayMenu {
    /// 正規表現プリセットメニューの再構築失敗をログする
    pub(crate) fn refresh_regex_presets_logged(&self, state: &AppState) {
        if let Err(err) = self.refresh_regex_presets(state) {
            dispatch::log_menu_operation_error("正規表現プリセットメニューの再構築", err);
        }
    }
}
