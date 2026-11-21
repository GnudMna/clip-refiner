//! セレクター / 登録クリップ / お気に入りのユーザーイベント処理

use std::time::Instant;

use super::misc::exit_if_e2e_selector_closed;
use crate::config::{AppConfig, FavoriteMoveDirection};
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::e2e;
use crate::tray::event::{
    AppEventContext, copy_registered_clip, delete_registered_clip, move_favorite_mode,
    refresh_clips_views, toggle_favorite_mode, update_refine,
};
use crate::tray::selector::hide_with_retention;
use crate::tray::settings::{refresh_settings_clips, refresh_settings_favorites_and_shortcuts};
use crate::tray::state::{ClipsEvent, FavoritesEvent, SelectorEvent};
use crate::tray::worker::ClipboardCommand;

// ======================================================================
// セレクター
// ======================================================================
/// セレクター関連イベントを処理する
pub(super) fn handle(ctx: &mut AppEventContext<'_>, event: SelectorEvent) {
    match event {
        SelectorEvent::RequestModeChange(mode) => {
            hide_selector(ctx, SelectorKind::Quick);
            update_refine(ctx.state, ctx.menu, ctx.clipboard_worker, mode, None);
            exit_if_e2e_selector_closed(ctx);
        }
        SelectorEvent::HideQuick => {
            hide_selector(ctx, SelectorKind::Quick);
            exit_if_e2e_selector_closed(ctx);
        }
        SelectorEvent::HideClip => {
            hide_selector(ctx, SelectorKind::Clip);
            exit_if_e2e_selector_closed(ctx);
        }
        SelectorEvent::ShowQuick => show_quick_selector(ctx),
        SelectorEvent::ShowClip => show_clip_selector(ctx),
    }
}

/// 登録クリップ関連イベントを処理する
pub(super) fn handle_clips(ctx: &mut AppEventContext<'_>, event: ClipsEvent) {
    match event {
        ClipsEvent::RequestCopy(index) => handle_clip_copy(ctx, index),
        ClipsEvent::RequestRegister => {
            dispatch::send_clipboard_command(
                ctx.clipboard_worker,
                ClipboardCommand::RegisterClipFromClipboard,
            );
        }
        ClipsEvent::RequestDelete(index) => handle_clip_delete(ctx, index),
        ClipsEvent::Refresh => {
            refresh_clips_views(ctx.state, ctx.menu, &ctx.selectors.clip);
            refresh_settings_clips(ctx.state, ctx.settings);
        }
    }
}

/// お気に入り変換モード関連イベントを処理する
pub(super) fn handle_favorites(ctx: &mut AppEventContext<'_>, event: FavoritesEvent) {
    match event {
        FavoritesEvent::RequestToggle(mode) => handle_favorite_toggle(ctx, mode),
        FavoritesEvent::RequestMove(mode, direction) => {
            handle_favorite_move(ctx, mode, direction);
        }
    }
}

#[derive(Clone, Copy)]
enum SelectorKind {
    Quick,
    Clip,
}

/// 保持方針に従ってセレクターを非表示にする
fn hide_selector(ctx: &AppEventContext<'_>, kind: SelectorKind) {
    let retention = ctx.state.with_config(|c| c.selector_webview_retention);
    match kind {
        SelectorKind::Quick => hide_with_retention(&ctx.selectors.quick, retention),
        SelectorKind::Clip => hide_with_retention(&ctx.selectors.clip, retention),
    }
}

/// クイックセレクターを表示する
fn show_quick_selector(ctx: &mut AppEventContext<'_>) {
    if ctx.selectors.clip.is_visible() {
        hide_selector(ctx, SelectorKind::Clip);
    }
    *ctx.last_quick_selector_show = Instant::now();
    let modes_json = ctx.state.with_config(AppConfig::modes_to_json_list);
    ctx.selectors.quick.show(
        ctx.event_loop,
        ctx.state.with_config(|c| c.mode),
        &modes_json,
    );
    e2e::signal_ready();
}

/// 登録クリップセレクターを表示する
fn show_clip_selector(ctx: &mut AppEventContext<'_>) {
    if ctx.selectors.quick.is_visible() {
        hide_selector(ctx, SelectorKind::Quick);
    }
    *ctx.last_clip_selector_show = Instant::now();
    let clips_json = ctx.state.with_config(AppConfig::clips_to_json_list);
    ctx.selectors.clip.show(ctx.event_loop, &clips_json);
    e2e::signal_ready();
}

/// 登録クリップのコピー要求を処理する
fn handle_clip_copy(ctx: &mut AppEventContext<'_>, index: usize) {
    hide_selector(ctx, SelectorKind::Clip);
    copy_registered_clip(ctx.state, ctx.clipboard_worker, index);
    exit_if_e2e_selector_closed(ctx);
}

/// 登録クリップの削除要求を処理する
fn handle_clip_delete(ctx: &mut AppEventContext<'_>, index: usize) {
    delete_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, index);
    refresh_settings_clips(ctx.state, ctx.settings);
}

/// お気に入り変換モードの登録切替要求を処理する
fn handle_favorite_toggle(ctx: &mut AppEventContext<'_>, mode: RefineMode) {
    toggle_favorite_mode(ctx.state, ctx.menu, Some(&ctx.selectors.quick), mode);
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}

/// お気に入り変換モードの表示順変更要求を処理する
fn handle_favorite_move(
    ctx: &mut AppEventContext<'_>,
    mode: RefineMode,
    direction: FavoriteMoveDirection,
) {
    move_favorite_mode(
        ctx.state,
        ctx.menu,
        Some(&ctx.selectors.quick),
        mode,
        direction,
    );
    refresh_settings_favorites_and_shortcuts(ctx.state, ctx.settings);
}
