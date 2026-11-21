use std::sync::Arc;

use crate::tray::menu::TrayMenu;
use crate::tray::state::{AppState, LockExt};

// ======================================================================
// メニューイベント処理
// ======================================================================
/// 正規表現プリセット選択のメニューイベントを処理する
///
/// # Arguments
/// * `id` - クリックされたメニュー項目の ID
/// * `menu` - トレイメニュー構造体
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `bool` - この関数で処理した場合は `true`
pub(crate) fn handle_regex_preset_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
) -> bool {
    let selected_id = {
        let records = menu.regex_presets.records.lock_ignore_poison();
        records
            .iter()
            .find(|(item, _)| item.id() == id)
            .map(|(_, preset_id)| preset_id.clone())
    };

    let Some(preset_id) = selected_id else {
        return false;
    };

    let changed = state.with_config_mut(|config| config.regex.select_preset_by_id(&preset_id));
    if !changed {
        menu.refresh_regex_presets_logged(state);
        return true;
    }

    menu.regex_presets.sync_checked(&preset_id);
    state.save_config();
    true
}
