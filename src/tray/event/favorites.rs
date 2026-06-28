use std::sync::Arc;

use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::quick_selector::QuickSelectorWindow;
use super::super::state::AppState;
use crate::config::{AppConfig, FavoriteMoveDirection, FavoriteToggleResult};
use crate::refiner::RefineMode;

/// お気に入り変換モードのメニューイベントを処理する
pub(super) fn handle_favorites_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    state: &Arc<AppState>,
    quick_selector: Option<&QuickSelectorWindow>,
) -> bool {
    if id == menu.refine.add_favorite_item.id() {
        let mode = state.with_config(|config| config.mode);
        toggle_favorite_mode(state, menu, quick_selector, mode);
        return true;
    }

    if id == menu.refine.remove_favorite_item.id() {
        let mode = state.with_config(|config| config.mode);
        if state.with_config(|config| config.is_favorite_mode(mode)) {
            toggle_favorite_mode(state, menu, quick_selector, mode);
        }
        return true;
    }

    false
}

/// お気に入り変換モードの登録状態を切り替える
pub(crate) fn toggle_favorite_mode(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    mode: RefineMode,
) {
    let result = state.with_config_mut(|config| config.toggle_favorite_mode(mode));
    match result {
        FavoriteToggleResult::Added => {
            state.save_config();
            refresh_favorites_views(state, menu, quick_selector);
            notify::show_when_enabled(
                state,
                "お気に入り",
                &format!("「{}」をお気に入りに登録しました", mode.label()),
            );
        }
        FavoriteToggleResult::Removed => {
            state.save_config();
            refresh_favorites_views(state, menu, quick_selector);
            notify::show_when_enabled(
                state,
                "お気に入り",
                &format!("「{}」をお気に入りから解除しました", mode.label()),
            );
        }
        FavoriteToggleResult::LimitReached => {
            notify::show_when_enabled(
                state,
                "お気に入り",
                &format!(
                    "お気に入りは最大 {} 件まで登録できます",
                    crate::consts::MAX_FAVORITE_MODES
                ),
            );
        }
    }
}

/// お気に入り変換モードの表示順を変更する
pub(crate) fn move_favorite_mode(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
    mode: RefineMode,
    direction: FavoriteMoveDirection,
) {
    let moved = state.with_config_mut(|config| config.move_favorite_mode(mode, direction));
    if !moved {
        return;
    }

    state.save_config();
    refresh_favorites_views(state, menu, quick_selector);
}

/// お気に入り表示をメニューとクイックセレクターへ反映する
pub(crate) fn refresh_favorites_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    let _ = menu.refresh_favorites(state);
    if let Some(quick_selector) = quick_selector
        && quick_selector.is_visible()
    {
        let modes_json = state.with_config(AppConfig::modes_to_json_list);
        let current_mode = state.with_config(|config| config.mode);
        quick_selector.refresh_modes(&modes_json, current_mode);
    }
}
