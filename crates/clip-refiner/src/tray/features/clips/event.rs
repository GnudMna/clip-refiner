use std::sync::Arc;

use crate::config::{FavoriteMoveDirection, ResolvedClip, UpdateRegisteredClipError};
use crate::security::SecretString;
use crate::tray::dispatch;
use crate::tray::menu::TrayMenu;
use crate::tray::notify::{NotificationCategory, show_category_notification};
use crate::tray::selector::{ClipSelectorWindow, clip_hover_text, clip_selector_json};
use crate::tray::state::{AppState, ClipEditRequest, ClipRef, LockExt};
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
/// 操作対象が見つからない (表示後に一覧が変わった) 場合の利用者向けメッセージ
pub(crate) const STALE_CLIP_MESSAGE: &str =
    "登録クリップの一覧が更新されたため、操作を中止しました。もう一度操作してください";

/// 操作対象の登録クリップが現在の一覧に存在しない (表示後に一覧が変わった)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaleClip;

/// 登録クリップのクリップボードコピーを実行する
///
/// # Returns
/// * `Result<(), StaleClip>` - 対象が見つからない場合は `Err`
pub(crate) fn copy_registered_clip(
    state: &Arc<AppState>,
    clipboard_worker: &ClipboardWorkerHandle,
    target: ClipRef,
) -> Result<(), StaleClip> {
    let index = state
        .with_config(|config| target.resolve(config))
        .ok_or(StaleClip)?;

    dispatch::send_clipboard_command(
        clipboard_worker,
        ClipboardCommand::CopyRegisteredClip(index),
    );
    Ok(())
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
/// * `target` - 編集対象の登録クリップ
///
/// # Returns
/// * `Result<(String, SecretString, u64), &'static str>` - ラベル・本文・識別トークン、
///   または利用者向けの失敗理由
pub(crate) fn registered_clip_for_edit(
    state: &Arc<AppState>,
    target: ClipRef,
) -> Result<(String, SecretString, u64), &'static str> {
    state.with_config(|c| {
        let index = target.resolve(c).ok_or(STALE_CLIP_MESSAGE)?;
        let clip = &c.clips[index];
        if clip.is_image() {
            return Err("画像の登録クリップは編集できません");
        }
        match c.resolve_registered_clip(index) {
            Some(ResolvedClip::Text(text)) => Ok((clip.label.clone(), text, clip.identity_token())),
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
        let index = request
            .target
            .resolve(c)
            .ok_or(UpdateRegisteredClipError::NotFound)?;
        c.update_registered_clip(index, &request.label, request.text.as_str())
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
    target: ClipRef,
) -> Result<(), StaleClip> {
    state
        .with_config_mut(|c| {
            let index = target.resolve(c)?;
            Some(c.remove_registered_clip(index))
        })
        .ok_or(StaleClip)?;

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    show_category_notification(
        state,
        NotificationCategory::RegisteredClip,
        "登録クリップ",
        "登録クリップを削除しました",
    );
    Ok(())
}

/// 登録クリップの表示順を変更し、メニューとセレクターを更新する
pub(crate) fn move_registered_clip(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    target: ClipRef,
    direction: FavoriteMoveDirection,
) -> Result<(), StaleClip> {
    let moved = state
        .with_config_mut(|c| {
            let index = target.resolve(c)?;
            Some(c.move_registered_clip(index, direction))
        })
        .ok_or(StaleClip)?;
    if !moved {
        return Ok(());
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    Ok(())
}

/// 登録クリップのテンプレート展開を切り替え、メニューとセレクターを更新する
pub(crate) fn set_registered_clip_template(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    target: ClipRef,
    enabled: bool,
) -> Result<(), StaleClip> {
    let changed = state
        .with_config_mut(|c| {
            let index = target.resolve(c)?;
            Some(c.set_registered_clip_template(index, enabled))
        })
        .ok_or(StaleClip)?;
    if !changed {
        return Ok(());
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    Ok(())
}

/// 登録クリップの連番を最初に戻し、メニューとセレクターを更新する
///
/// 次のコピーで `{counter}` が 1 から始まる。連番が既に未使用のときは何もしない
pub(crate) fn reset_registered_clip_counter(
    state: &Arc<AppState>,
    menu: &TrayMenu,
    clip_selector: &ClipSelectorWindow,
    target: ClipRef,
) -> Result<(), StaleClip> {
    let changed = state
        .with_config_mut(|c| {
            let index = target.resolve(c)?;
            Some(c.set_registered_clip_counter(index, 0))
        })
        .ok_or(StaleClip)?;
    if !changed {
        return Ok(());
    }

    state.save_config();
    refresh_clips_views(state, menu, clip_selector);
    show_category_notification(
        state,
        NotificationCategory::RegisteredClip,
        "登録クリップ",
        "連番をリセットしました",
    );
    Ok(())
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

            let (label, text, _) =
                registered_clip_for_edit(&state, ClipRef::unchecked(0)).expect("text clip");
            assert_eq!(label, "見出し");
            assert_eq!(text.as_str(), "line1\nline2");
            let (_, secret, _) =
                registered_clip_for_edit(&state, ClipRef::unchecked(1)).expect("sensitive clip");
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

            assert!(registered_clip_for_edit(&state, ClipRef::unchecked(0)).is_err());
            assert!(registered_clip_for_edit(&state, ClipRef::unchecked(1)).is_err());
        });
    }

    /// 表示後に並びが変わった場合、トークンが一致しない指定では編集用の取り出しが失敗すること
    #[test]
    fn registered_clip_for_edit_rejects_stale_token() {
        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            state.with_config_mut(|c| {
                c.add_registered_clip("first").expect("add");
                c.add_registered_clip("second").expect("add");
            });
            let token_of_first = state.with_config(|c| c.clips[0].identity_token());

            // 表示時点では 0 番目が "first"
            let (_, text, _) =
                registered_clip_for_edit(&state, ClipRef::checked(0, token_of_first)).expect("ok");
            assert_eq!(text.as_str(), "first");

            // 並びが入れ替わった後は同じ指定を拒否し、移動先の位置なら取り出せる
            state.with_config_mut(|c| {
                assert!(c.move_registered_clip(0, FavoriteMoveDirection::Down));
            });
            assert_eq!(
                registered_clip_for_edit(&state, ClipRef::checked(0, token_of_first)).err(),
                Some(STALE_CLIP_MESSAGE)
            );
            let (_, text, _) =
                registered_clip_for_edit(&state, ClipRef::checked(1, token_of_first)).expect("ok");
            assert_eq!(text.as_str(), "first");
        });
    }

    /// 編集内容の保存で、トークンが一致しない指定は別のクリップを書き換えず `NotFound` となること
    #[test]
    fn stale_edit_request_does_not_update_other_clip() {
        crate::test_helpers::with_temp_config_dir(|| {
            let state = Arc::new(test_app_state());
            state.with_config_mut(|c| {
                c.add_registered_clip("first").expect("add");
                c.add_registered_clip("second").expect("add");
            });
            let token_of_first = state.with_config(|c| c.clips[0].identity_token());
            // "first" が削除され、0 番目が "second" になった状況
            state.with_config_mut(|c| {
                assert!(c.remove_registered_clip(0));
            });

            let result = state.with_config_mut(|c| {
                let index = ClipRef::checked(0, token_of_first)
                    .resolve(c)
                    .ok_or(UpdateRegisteredClipError::NotFound)?;
                c.update_registered_clip(index, "x", "overwritten")
            });
            assert_eq!(result, Err(UpdateRegisteredClipError::NotFound));
            let (_, text, _) =
                registered_clip_for_edit(&state, ClipRef::unchecked(0)).expect("text");
            assert_eq!(text.as_str(), "second");
        });
    }
}
