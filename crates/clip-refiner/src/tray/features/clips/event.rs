use std::sync::Arc;

use crate::config::{FavoriteMoveDirection, ResolvedClip, UpdateRegisteredClipError};
use crate::security::SecretString;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::notify::{NotificationCategory, show_category_notification};
use crate::tray::selector::{ClipSelectorWindow, clip_hover_text, clip_selector_json};
use crate::tray::state::{AppState, ClipEditRequest, LockExt};
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// メニューイベント処理
// ======================================================================
/// 登録クリップメニューイベントを処理する
pub(crate) fn handle_clips_event(
    id: &tray_icon::menu::MenuId,
    menu: &TrayMenu,
    _state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
) -> bool {
    if id == menu.clips.register_item.id() {
        dispatch::send_clipboard_command(
            clipboard_worker,
            ClipboardCommand::RegisterClipFromClipboard,
        );
        return true;
    }

    let menu_records = menu.clips.records.lock_ignore_poison();

    if let Some((_, index)) = menu_records.iter().find(|(rec_id, _)| *rec_id == id) {
        dispatch::send_clipboard_command(
            clipboard_worker,
            ClipboardCommand::CopyRegisteredClip(*index),
        );
        return true;
    }

    false
}

// ======================================================================
// 登録クリップ操作
// ======================================================================
/// 登録クリップのクリップボードコピーを実行する
pub(crate) fn copy_registered_clip(
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
    index: usize,
) {
    let exists = state.with_config(|config| config.clips.get(index).is_some());
    if !exists {
        return;
    }

    dispatch::send_clipboard_command(
        clipboard_worker,
        ClipboardCommand::CopyRegisteredClip(index),
    );
}

/// 登録クリップ本文の全文をセレクターのホバー表示へ渡す
///
/// 画像・機密情報らしい本文・復号失敗の場合は何も返さない
pub(crate) fn show_registered_clip_full_text(
    state: &Arc<AppState>,
    clip_selector: &ClipSelectorWindow,
    index: usize,
) {
    let Some(ResolvedClip::Text(text)) = state.with_config(|c| c.resolve_registered_clip(index))
    else {
        return;
    };
    if let Some(hover_text) = clip_hover_text(&text) {
        clip_selector.show_full_text(index, &hover_text);
    }
}

/// 設定ウィンドウの編集用に、登録クリップのラベルと本文の全文を取り出す
///
/// ホバー表示と違い、機密情報らしい本文も省略しない (利用者が自分で開いた編集操作のため)
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
/// * `index` - 登録クリップのインデックス
///
/// # Returns
/// * `Result<(String, SecretString), &'static str>` - ラベルと本文、または利用者向けの失敗理由
pub(crate) fn registered_clip_for_edit(
    state: &Arc<AppState>,
    index: usize,
) -> Result<(String, SecretString), &'static str> {
    state.with_config(|c| {
        let clip = c.clips.get(index).ok_or("登録クリップが見つかりません")?;
        if clip.is_image() {
            return Err("画像の登録クリップは編集できません");
        }
        match c.resolve_registered_clip(index) {
            Some(ResolvedClip::Text(text)) => Ok((clip.label.clone(), text)),
            _ => Err("登録クリップ本文の読み込みに失敗しました"),
        }
    })
}

/// 登録クリップのラベルと本文を更新し、メニューとセレクターを更新する
pub(crate) fn update_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    request: &ClipEditRequest,
) -> Result<(), UpdateRegisteredClipError> {
    state.with_config_mut(|c| {
        c.update_registered_clip(request.index, &request.label, request.text.as_str())
    })?;

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    show_category_notification(
        state,
        NotificationCategory::RegisteredClip,
        "登録クリップ",
        "登録クリップを更新しました",
    );
    Ok(())
}

/// 登録クリップを削除し、メニューとセレクターを更新する
pub(crate) fn delete_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
) {
    let removed = state.with_config_mut(|c| c.remove_registered_clip(index));
    if !removed {
        return;
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    show_category_notification(
        state,
        NotificationCategory::RegisteredClip,
        "登録クリップ",
        "登録クリップを削除しました",
    );
}

/// 登録クリップの表示順を変更し、メニューとセレクターを更新する
pub(crate) fn move_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
    direction: FavoriteMoveDirection,
) {
    let moved = state.with_config_mut(|c| c.move_registered_clip(index, direction));
    if !moved {
        return;
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
}

/// 登録クリップのテンプレート展開を切り替え、メニューとセレクターを更新する
pub(crate) fn set_registered_clip_template(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
    enabled: bool,
) {
    let changed = state.with_config_mut(|c| c.set_registered_clip_template(index, enabled));
    if !changed {
        return;
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
}

/// 登録クリップの連番を最初に戻し、メニューとセレクターを更新する
///
/// 次のコピーで `{counter}` が 1 から始まる。連番が既に未使用のときは何もしない
pub(crate) fn reset_registered_clip_counter(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    index: usize,
) {
    let changed = state.with_config_mut(|c| c.set_registered_clip_counter(index, 0));
    if !changed {
        return;
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    show_category_notification(
        state,
        NotificationCategory::RegisteredClip,
        "登録クリップ",
        "連番をリセットしました",
    );
}

// ======================================================================
// UI 更新
// ======================================================================
/// 登録クリップメニューとセレクター表示を設定内容に合わせて更新する
pub(crate) fn refresh_clips_views(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
) {
    if let Err(err) = menu.refresh_clips(state) {
        dispatch::log_menu_operation_error("登録クリップメニューの更新", err);
    }
    if clip_selector.is_visible() {
        let clips_json = state.with_config(clip_selector_json);
        clip_selector.refresh_items(&clips_json);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::state::test_app_state;

    /// 編集用の取り出しがテキスト登録のラベルと全文を返し、機密らしい本文も省略しないこと
    #[test]
    fn registered_clip_for_edit_returns_label_and_full_text() {
        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
            state.with_config_mut(|c| {
                c.add_registered_clip("line1\nline2").expect("add");
                c.add_registered_clip(jwt).expect("add");
                c.update_registered_clip(0, "見出し", "line1\nline2")
                    .expect("label");
            });

            let (label, text) = registered_clip_for_edit(&state, 0).expect("text clip");
            assert_eq!(label, "見出し");
            assert_eq!(text.as_str(), "line1\nline2");
            let (_, secret) = registered_clip_for_edit(&state, 1).expect("sensitive clip");
            assert_eq!(secret.as_str(), jwt);
        });
    }

    /// 範囲外と画像登録は編集用に取り出せないこと
    #[test]
    fn registered_clip_for_edit_rejects_missing_and_image() {
        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            state.with_config_mut(|c| {
                c.add_registered_image(1, 1, &[0, 0, 0, 255])
                    .expect("image");
            });

            assert!(registered_clip_for_edit(&state, 0).is_err());
            assert!(registered_clip_for_edit(&state, 1).is_err());
        });
    }
}
