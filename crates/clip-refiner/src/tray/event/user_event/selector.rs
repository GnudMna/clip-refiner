//! セレクター / 登録クリップ / お気に入りのユーザーイベント処理

use std::time::Instant;

use super::misc::exit_if_e2e_selector_closed;
use crate::config::FavoriteMoveDirection;
use crate::refiner::RefineMode;
use crate::tray::dispatch;
use crate::tray::e2e;
use crate::tray::event::{
    AppEventContext, STALE_CLIP_MESSAGE, StaleClip, apply_refine_once, copy_registered_clip,
    delete_registered_clip, move_favorite_mode, move_registered_clip, refresh_clips_views,
    registered_clip_for_edit, reset_registered_clip_counter, select_auto_refine_mode,
    set_registered_clip_template, show_registered_clip_full_text, toggle_favorite_mode,
    update_registered_clip,
};
use crate::tray::selector::{
    clip_selector_json, hide_with_retention, quick_selector_current_mode, quick_selector_json,
};
use crate::tray::settings::{refresh_settings_clips, refresh_settings_favorites_and_shortcuts};
use crate::tray::state::{ClipRef, ClipsEvent, FavoritesEvent, SelectorEvent};
use crate::tray::worker::ClipboardCommand;

// ======================================================================
// セレクター
// ======================================================================
/// セレクター関連イベントを処理する
pub(super) fn handle(ctx: &mut AppEventContext<'_>, event: SelectorEvent) {
    match event {
        SelectorEvent::RequestModeChange(mode) => {
            hide_selector(ctx, SelectorKind::Quick);
            select_auto_refine_mode(ctx.state, ctx.menu, ctx.clipboard_worker, mode, None);
            exit_if_e2e_selector_closed(ctx);
        }
        SelectorEvent::RequestApplyOnce(mode) => {
            hide_selector(ctx, SelectorKind::Quick);
            apply_refine_once(ctx.clipboard_worker, mode);
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
        ClipsEvent::RequestMove(index, direction) => handle_clip_move(ctx, index, direction),
        ClipsEvent::RequestSetTemplate(index, enabled) => {
            handle_clip_set_template(ctx, index, enabled);
        }
        ClipsEvent::RequestResetCounter(index) => handle_clip_reset_counter(ctx, index),
        ClipsEvent::RequestFullText(index) => {
            show_registered_clip_full_text(ctx.state, &ctx.selectors.clip, index);
        }
        ClipsEvent::RequestEditOpen(index) => handle_clip_edit_open(ctx, index),
        ClipsEvent::RequestEditApply => handle_clip_edit_apply(ctx),
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
    let modes_json = ctx.state.with_config(quick_selector_json);
    ctx.selectors.quick.show(
        ctx.event_loop,
        ctx.state.with_config(quick_selector_current_mode),
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
    let clips_json = ctx.state.with_config(clip_selector_json);
    ctx.selectors.clip.show(ctx.event_loop, &clips_json);
    e2e::signal_ready();
}

/// 対象の登録クリップが見つからなかった操作を設定ウィンドウへ伝える
///
/// 設定ウィンドウは表示時のトークンを付けて操作するため、一覧が更新されて照合に失敗した場合に限り通知する
/// 一覧はこの後の再描画で最新になる
fn report_stale_clip(ctx: &AppEventContext<'_>, target: ClipRef, result: Result<(), StaleClip>) {
    if result.is_err() && target.token.is_some() {
        ctx.settings
            .notify_clip_edit_result(false, STALE_CLIP_MESSAGE);
    }
}

/// 登録クリップのコピー要求を処理する
fn handle_clip_copy(ctx: &mut AppEventContext<'_>, target: ClipRef) {
    hide_selector(ctx, SelectorKind::Clip);
    let result = copy_registered_clip(ctx.state, ctx.clipboard_worker, target);
    report_stale_clip(ctx, target, result);
    exit_if_e2e_selector_closed(ctx);
}

/// 登録クリップの削除要求を処理する
fn handle_clip_delete(ctx: &mut AppEventContext<'_>, target: ClipRef) {
    let result = delete_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, target);
    refresh_settings_clips(ctx.state, ctx.settings);
    report_stale_clip(ctx, target, result);
}

/// 登録クリップの表示順変更要求を処理する
fn handle_clip_move(
    ctx: &mut AppEventContext<'_>,
    target: ClipRef,
    direction: FavoriteMoveDirection,
) {
    let result = move_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, target, direction);
    refresh_settings_clips(ctx.state, ctx.settings);
    report_stale_clip(ctx, target, result);
}

/// 登録クリップのテンプレート展開の切り替え要求を処理する
fn handle_clip_set_template(ctx: &mut AppEventContext<'_>, target: ClipRef, enabled: bool) {
    let result =
        set_registered_clip_template(ctx.state, ctx.menu, &ctx.selectors.clip, target, enabled);
    refresh_settings_clips(ctx.state, ctx.settings);
    report_stale_clip(ctx, target, result);
}

/// 登録クリップの連番リセット要求を処理する
fn handle_clip_reset_counter(ctx: &mut AppEventContext<'_>, target: ClipRef) {
    let result = reset_registered_clip_counter(ctx.state, ctx.menu, &ctx.selectors.clip, target);
    refresh_settings_clips(ctx.state, ctx.settings);
    report_stale_clip(ctx, target, result);
}

/// 登録クリップの編集開始要求を処理し、設定ウィンドウへ本文を渡す
///
/// 一覧が更新されて照合に失敗した場合は、最新の一覧を反映してから失敗を伝える
fn handle_clip_edit_open(ctx: &mut AppEventContext<'_>, target: ClipRef) {
    match registered_clip_for_edit(ctx.state, target) {
        Ok((label, text, token)) => {
            ctx.settings
                .show_clip_editor(target.index, token, &label, &text);
        }
        Err(message) => {
            if message == STALE_CLIP_MESSAGE {
                refresh_settings_clips(ctx.state, ctx.settings);
            }
            ctx.settings.notify_clip_edit_result(false, message);
        }
    }
}

/// 設定ウィンドウから預かった登録クリップの編集内容をすべて保存する
fn handle_clip_edit_apply(ctx: &mut AppEventContext<'_>) {
    while let Some(request) = ctx.state.settings_pending().take_clip_edit() {
        match update_registered_clip(ctx.state, ctx.menu, &ctx.selectors.clip, &request) {
            Ok(()) => {
                refresh_settings_clips(ctx.state, ctx.settings);
                ctx.settings.notify_clip_edit_result(true, "");
            }
            Err(err) => ctx
                .settings
                .notify_clip_edit_result(false, err.user_message()),
        }
    }
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
