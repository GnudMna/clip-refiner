use std::sync::Arc;

use crate::config::{FavoriteMoveDirection, FavoriteToggleResult};
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::notify::{NotificationCategory, show_category_notification};
use crate::tray::selector::{
    QuickSelectorWindow, quick_selector_current_mode, quick_selector_json,
};
use crate::tray::state::{AppState, HotkeyEvent};

// ======================================================================
// メニューイベント処理
// ======================================================================
/// お気に入り変換モードのメニューイベントを処理する
pub(crate) fn handle_favorites_event(
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
            show_category_notification(
                state,
                NotificationCategory::Favorites,
                "お気に入り",
                &format!("「{}」をお気に入りに登録しました", mode.label()),
            );
        }
        FavoriteToggleResult::Removed => {
            state.save_config();
            refresh_favorites_views(state, menu, quick_selector);
            show_category_notification(
                state,
                NotificationCategory::Favorites,
                "お気に入り",
                &format!("「{}」をお気に入りから解除しました", mode.label()),
            );
        }
        FavoriteToggleResult::LimitReached => {
            show_category_notification(
                state,
                NotificationCategory::Favorites,
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
    dispatch::send_app_event(&state.proxy, HotkeyEvent::ReloadFavorites.into());
}

/// 表示中のクイックセレクターへモード一覧と現在モードを反映する
pub(crate) fn refresh_quick_selector_modes(
    state: &Arc<AppState>,
    quick_selector: Option<&QuickSelectorWindow>,
) {
    if let Some(quick_selector) = quick_selector
        && quick_selector.is_visible()
    {
        let modes_json = state.with_config(quick_selector_json);
        let current_mode = state.with_config(quick_selector_current_mode);
        quick_selector.refresh_modes(&modes_json, current_mode);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FavoriteMoveDirection;
    use crate::consts;
    use crate::refiner::RefineMode;
    use crate::test_helpers::TrayTestContext;
    use crate::tray::state::LockExt;

    use strum::IntoEnumIterator;

    /// お気に入り登録で設定とメニュー表示が更新されるようにすること
    #[test]
    fn toggle_favorite_mode_adds_to_config_and_menu() {
        let ctx = TrayTestContext::with_config(|config| config.mode = RefineMode::Trim);

        toggle_favorite_mode(ctx.state(), ctx.menu(), None, RefineMode::Trim);

        assert!(
            ctx.state()
                .with_config(|config| config.is_favorite_mode(RefineMode::Trim))
        );
        let records = ctx.menu().refine.favorite_records.lock_ignore_poison();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1, RefineMode::Trim);
        assert!(!ctx.menu().refine.add_favorite_item.is_enabled());
        assert!(ctx.menu().refine.remove_favorite_item.is_enabled());
    }

    /// お気に入り解除で設定とメニュー表示が更新されるようにすること
    #[test]
    fn toggle_favorite_mode_removes_from_config_and_menu() {
        let ctx = TrayTestContext::with_config(|config| {
            config.mode = RefineMode::Trim;
            config.favorite_modes = vec![RefineMode::Trim];
        });

        toggle_favorite_mode(ctx.state(), ctx.menu(), None, RefineMode::Trim);

        assert!(
            !ctx.state()
                .with_config(|config| config.is_favorite_mode(RefineMode::Trim))
        );
        assert!(
            ctx.menu()
                .refine
                .favorite_records
                .lock_ignore_poison()
                .is_empty()
        );
        assert!(ctx.menu().refine.add_favorite_item.is_enabled());
        assert!(!ctx.menu().refine.remove_favorite_item.is_enabled());
    }

    /// 上限到達時は設定を変更せず登録項目を無効のままにすること
    #[test]
    fn toggle_favorite_mode_respects_limit() {
        let favorites: Vec<_> = RefineMode::iter()
            .take(consts::MAX_FAVORITE_MODES)
            .collect();
        let extra = RefineMode::iter()
            .find(|mode| !favorites.contains(mode))
            .expect("未登録モードが存在する");
        let favorites_for_assert = favorites.clone();
        let ctx = TrayTestContext::with_config(|config| {
            config.mode = extra;
            config.favorite_modes = favorites;
        });

        toggle_favorite_mode(ctx.state(), ctx.menu(), None, extra);

        assert_eq!(
            ctx.state()
                .with_config(|config| config.favorite_modes.clone()),
            favorites_for_assert
        );
        assert!(!ctx.menu().refine.add_favorite_item.is_enabled());
    }

    /// 表示順変更が設定とお気に入りサブメニューへ反映されるようにすること
    #[test]
    fn move_favorite_mode_updates_order() {
        let ctx = TrayTestContext::with_config(|config| {
            config.favorite_modes = vec![RefineMode::Trim, RefineMode::UrlDecode];
        });

        move_favorite_mode(
            ctx.state(),
            ctx.menu(),
            None,
            RefineMode::UrlDecode,
            FavoriteMoveDirection::Up,
        );

        assert_eq!(
            ctx.state()
                .with_config(|config| config.favorite_modes.clone()),
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );
        let records = ctx.menu().refine.favorite_records.lock_ignore_poison();
        assert_eq!(
            records.iter().map(|(_, mode)| *mode).collect::<Vec<_>>(),
            vec![RefineMode::UrlDecode, RefineMode::Trim]
        );

        // クイックセレクター再オープン時と同じ JSON 経路で favoriteIndex が入れ替わること
        let modes_json = ctx.state().with_config(quick_selector_json);
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&modes_json).expect("quick_selector_json が JSON");
        let url_decode = parsed
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some("UrlDecode"))
            .expect("UrlDecode");
        let trim = parsed
            .iter()
            .find(|item| item.get("id").and_then(|v| v.as_str()) == Some("Trim"))
            .expect("Trim");
        assert_eq!(
            url_decode
                .get("favoriteIndex")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            trim.get("favoriteIndex")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    /// トレイメニューの登録項目からお気に入りを追加できるようにすること
    #[test]
    fn handle_favorites_event_adds_current_mode() {
        let ctx = TrayTestContext::with_config(|config| config.mode = RefineMode::JsonFormat);

        assert!(handle_favorites_event(
            ctx.menu().refine.add_favorite_item.id(),
            ctx.menu(),
            ctx.state(),
            None,
        ));
        assert!(
            ctx.state()
                .with_config(|config| config.is_favorite_mode(RefineMode::JsonFormat))
        );
    }

    /// 未登録モードの解除操作は no-op となるようにすること
    #[test]
    fn handle_favorites_event_remove_skips_unregistered_mode() {
        let ctx = TrayTestContext::with_config(|config| config.mode = RefineMode::Trim);

        assert!(handle_favorites_event(
            ctx.menu().refine.remove_favorite_item.id(),
            ctx.menu(),
            ctx.state(),
            None,
        ));
        assert!(
            !ctx.state()
                .with_config(|config| config.is_favorite_mode(RefineMode::Trim))
        );
    }
}
