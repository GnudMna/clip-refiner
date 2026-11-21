use std::sync::Arc;

use super::super::dispatch;
use super::super::menu::TrayMenu;
use super::super::notify;
use super::super::quick_selector::QuickSelectorWindow;
use super::super::state::{AppEvent, AppState};

use crate::config::{AppConfig, FavoriteMoveDirection, FavoriteToggleResult};
use crate::refiner::RefineMode;

// ======================================================================
// メニューイベント処理
// ======================================================================
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

// ======================================================================
// お気に入り操作
// ======================================================================
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

// ======================================================================
// UI 更新
// ======================================================================
/// お気に入り表示をメニューとクイックセレクターへ反映する
pub(crate) fn refresh_favorites_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    if let Err(err) = menu.refresh_favorites(state) {
        dispatch::log_menu_operation_error("お気に入りメニューの更新", err);
    }
    refresh_quick_selector_modes(state, quick_selector);
    dispatch::send_app_event(&state.proxy, AppEvent::ReloadFavoriteHotkeys);
}

/// 表示中のクイックセレクターへモード一覧と現在モードを反映する
pub(crate) fn refresh_quick_selector_modes(
    state: &Arc<AppState>,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    if let Some(quick_selector) = quick_selector
        && quick_selector.is_visible()
    {
        let modes_json = state.with_config(AppConfig::modes_to_json_list);
        let current_mode = state.with_config(|config| config.mode);
        quick_selector.refresh_modes(&modes_json, current_mode);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use crate::config::FavoriteMoveDirection;
    use crate::consts;
    use crate::refiner::RefineMode;
    use crate::tray::menu::TrayMenu;
    use crate::tray::state::{LockExt, test_app_state};

    use strum::IntoEnumIterator;

    fn build_menu(state: &Arc<AppState>) -> TrayMenu {
        TrayMenu::build(state).expect("テスト用トレイメニューの構築に失敗")
    }

    /// お気に入り登録で設定とメニュー表示が更新されること
    #[test]
    fn toggle_favorite_mode_adds_to_config_and_menu() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|config| config.mode = RefineMode::Trim);
        let menu = build_menu(&state);

        toggle_favorite_mode(&state, &menu, None, RefineMode::Trim);

        assert!(state.with_config(|config| config.is_favorite_mode(RefineMode::Trim)));
        let records = menu.refine.favorite_records.lock_ignore_poison();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1, RefineMode::Trim);
        assert!(!menu.refine.add_favorite_item.is_enabled());
        assert!(menu.refine.remove_favorite_item.is_enabled());
    }

    /// お気に入り解除で設定とメニュー表示が更新されること
    #[test]
    fn toggle_favorite_mode_removes_from_config_and_menu() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|config| {
            config.mode = RefineMode::Trim;
            config.favorite_modes = vec![RefineMode::Trim];
        });
        let menu = build_menu(&state);

        toggle_favorite_mode(&state, &menu, None, RefineMode::Trim);

        assert!(!state.with_config(|config| config.is_favorite_mode(RefineMode::Trim)));
        assert!(menu.refine.favorite_records.lock_ignore_poison().is_empty());
        assert!(menu.refine.add_favorite_item.is_enabled());
        assert!(!menu.refine.remove_favorite_item.is_enabled());
    }

    /// 上限到達時は設定を変更せず登録項目を無効のままにすること
    #[test]
    fn toggle_favorite_mode_respects_limit() {
        let state = Arc::new(test_app_state());
        let favorites: Vec<_> = RefineMode::iter()
            .take(consts::MAX_FAVORITE_MODES)
            .collect();
        let extra = RefineMode::iter()
            .find(|mode| !favorites.contains(mode))
            .expect("未登録モードが存在する");
        state.with_config_mut(|config| {
            config.mode = extra;
            config.favorite_modes = favorites.clone();
        });
        let menu = build_menu(&state);

        toggle_favorite_mode(&state, &menu, None, extra);

        assert_eq!(
            state.with_config(|config| config.favorite_modes.clone()),
            favorites
        );
        assert!(!menu.refine.add_favorite_item.is_enabled());
    }

    /// 表示順変更が設定とお気に入りサブメニューへ反映されること
    #[test]
    fn move_favorite_mode_updates_order() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|config| {
            config.favorite_modes = vec![RefineMode::Trim, RefineMode::UrlDecode];
        });
        let menu = build_menu(&state);

        move_favorite_mode(
            &state,
            &menu,
            None,
            RefineMode::UrlDecode,
            FavoriteMoveDirection::Up,
        );

        assert_eq!(
            state.with_config(|config| config.favorite_modes.clone()),
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
        let records = menu.refine.favorite_records.lock_ignore_poison();
        assert_eq!(
            records.iter().map(|(_, mode)| *mode).collect::<Vec<_>>(),
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
    }

    /// トレイメニューの登録項目からお気に入りを追加できること
    #[test]
    fn handle_favorites_event_adds_current_mode() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|config| config.mode = RefineMode::JsonFormat);
        let menu = build_menu(&state);

        assert!(handle_favorites_event(
            menu.refine.add_favorite_item.id(),
            &menu,
            &state,
            None,
        ));
        assert!(state.with_config(|config| config.is_favorite_mode(RefineMode::JsonFormat)));
    }

    /// 未登録モードの解除操作は no-op であること
    #[test]
    fn handle_favorites_event_remove_skips_unregistered_mode() {
        let state = Arc::new(test_app_state());
        state.with_config_mut(|config| config.mode = RefineMode::Trim);
        let menu = build_menu(&state);

        assert!(handle_favorites_event(
            menu.refine.remove_favorite_item.id(),
            &menu,
            &state,
            None,
        ));
        assert!(!state.with_config(|config| config.is_favorite_mode(RefineMode::Trim)));
    }
}
