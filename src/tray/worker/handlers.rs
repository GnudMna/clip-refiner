use std::sync::Arc;

use super::super::dispatch;
use super::super::notify;
use super::super::state::{AppEvent, AppState};
use super::command::ClipboardCommand;

use crate::config::{AddRegisteredClipError, ResolvedClip};
use crate::platform;
use crate::refiner::{
    ClipboardProcessOutcome, ImageClipboard, RefineContext, TextClipboard,
    process_clipboard_pipeline_io,
};

/// コマンドを処理する
///
/// クリップボードの設定や加工を行い、成功時に通知を表示する
///
/// # Arguments
/// * `clipboard` - クリップボード操作用のインスタンス
/// * `state` - アプリケーションの共有状態
/// * `refine_ctx` - 加工コンテキスト (正規表現コンパイルキャッシュを保持)
/// * `cmd` - 受信したコマンド
pub(crate) fn handle_command<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    refine_ctx: &mut RefineContext,
    cmd: ClipboardCommand,
) {
    match cmd {
        ClipboardCommand::SetText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "クリップボードエラー",
                    "履歴からの復元処理に失敗しました。",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("履歴から復元", "クリップボードにコピーしました");
                }
            }
        }
        ClipboardCommand::CopyRegisteredClip(index) => {
            copy_registered_clip_to_clipboard(clipboard, state, index);
        }
        ClipboardCommand::SetOcrText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "OCR エラー",
                    "クリップボードへの書き込みに失敗しました",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("OCR", "クリップボードにコピーしました");
                }
            }
        }
        ClipboardCommand::ProcessMode(mode) => {
            let pre_text = clipboard.get_text().ok();
            refine_ctx.regex = state.with_config(|c| c.regex.clone());
            let pipeline = [mode];
            match process_clipboard_pipeline_io(clipboard, &pipeline, refine_ctx) {
                Ok(ClipboardProcessOutcome::Processed(processed)) => {
                    if let Some(ref pre) = pre_text {
                        state.record_undo_source(pre);
                    }
                    state.record_processing_success(&processed);
                    notify::show_process_notification(state, &pipeline, &processed);
                }
                Ok(ClipboardProcessOutcome::ImageProcessed { width, height }) => {
                    if let Some(ref pre) = pre_text {
                        state.record_undo_source(pre);
                        state.record_image_processing_success(pre);
                    }
                    notify::show_image_process_notification(state, mode, width, height);
                }
                Ok(ClipboardProcessOutcome::Unchanged) => {
                    if state.with_config(|c| c.notification_settings.enabled) {
                        platform::show_notification("加工結果", "テキストに変更はありませんでした");
                    }
                }
                Err(e) => {
                    crate::log_error!("加工エラー: {} ({:?})", e.user_message(), e);
                    platform::show_notification("加工エラー", e.user_message());
                }
            }
        }
        ClipboardCommand::Undo => {
            if let Some(text) = state.take_undo_source() {
                if let Err(e) = clipboard.set_text(text.to_string()) {
                    crate::log_error!("加工取り消しエラー: {:?}", e);
                    state.record_undo_source(&text);
                    platform::show_notification(
                        "クリップボードエラー",
                        "加工の取り消しに失敗しました",
                    );
                } else {
                    state.record_processing_success(&text);
                    if state.with_config(|c| c.notification_settings.enabled) {
                        platform::show_notification(
                            "加工の取り消し",
                            "クリップボードを加工前の内容に戻しました",
                        );
                    }
                }
            } else if state.with_config(|c| c.notification_settings.enabled) {
                platform::show_notification("加工の取り消し", "取り消せる加工がありません");
            }
        }
        ClipboardCommand::RegisterClipFromClipboard => {
            register_clip_from_clipboard(clipboard, state);
        }
    }
}

/// 登録済みクリップボード内容をクリップボードへコピーする
fn copy_registered_clip_to_clipboard<C: ImageClipboard + TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    index: usize,
) {
    let clip = state.with_config(|config| config.resolve_registered_clip(index));
    match clip {
        Some(ResolvedClip::Text(text)) => {
            if let Err(e) = clipboard.set_text(text.clone()) {
                crate::log_error!("クリップボード設定エラー: {:?}", e);
                platform::show_notification(
                    "クリップボードエラー",
                    "登録クリップのコピーに失敗しました。",
                );
            } else {
                state.record_clipboard_set(&text);
                if state.with_config(|c| c.notification_settings.enabled) {
                    platform::show_notification("登録クリップ", "クリップボードにコピーしました");
                }
            }
        }
        Some(ResolvedClip::Image {
            width,
            height,
            rgba,
        }) => {
            if let Err(e) = clipboard.set_image(width, height, rgba) {
                crate::log_error!("クリップボード画像設定エラー: {:?}", e);
                platform::show_notification(
                    "クリップボードエラー",
                    "登録クリップのコピーに失敗しました。",
                );
            } else if state.with_config(|c| c.notification_settings.enabled) {
                platform::show_notification("登録クリップ", "クリップボードにコピーしました");
            }
        }
        None => {}
    }
}

/// クリップボードの内容を登録クリップとして保存する
pub(super) fn register_clip_from_clipboard<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
) {
    if let Ok((width, height, rgba)) = clipboard.get_image() {
        let outcome = state.with_config_mut(|c| c.add_registered_image(width, height, &rgba));
        match outcome {
            Ok(()) => {
                state.save_config();
                dispatch::send_app_event(&state.proxy, AppEvent::RefreshClips);
                notify::show_when_enabled(
                    state,
                    "登録クリップ",
                    "クリップボードの画像を登録しました",
                );
            }
            Err(AddRegisteredClipError::ImageTooLarge) => {
                notify::show_when_enabled(
                    state,
                    "登録クリップ",
                    "画像が大きすぎるため登録できません",
                );
            }
            Err(AddRegisteredClipError::LimitReached) => {
                notify::show_when_enabled(state, "登録クリップ", "登録件数の上限に達しています");
            }
            Err(
                AddRegisteredClipError::ImageInvalid
                | AddRegisteredClipError::Empty
                | AddRegisteredClipError::TooLarge,
            ) => {
                notify::show_when_enabled(
                    state,
                    "登録クリップ",
                    "画像の形式が不正なため登録できません",
                );
            }
        }
        return;
    }

    let text = match clipboard.get_text() {
        Ok(text) => text,
        Err(e) => {
            crate::log_error!("クリップボード読み取りエラー: {:?}", e);
            platform::show_notification(
                "クリップボードエラー",
                "クリップボードの読み取りに失敗しました",
            );
            return;
        }
    };

    let outcome = state.with_config_mut(|c| c.add_registered_clip(text));
    match outcome {
        Ok(()) => {
            state.save_config();
            dispatch::send_app_event(&state.proxy, AppEvent::RefreshClips);
            notify::show_when_enabled(state, "登録クリップ", "クリップボードの内容を登録しました");
        }
        Err(AddRegisteredClipError::Empty) => {
            notify::show_when_enabled(
                state,
                "登録クリップ",
                "クリップボードが空のため登録できません",
            );
        }
        Err(AddRegisteredClipError::TooLarge) => {
            notify::show_when_enabled(
                state,
                "登録クリップ",
                "テキストが長すぎるため登録できません",
            );
        }
        Err(AddRegisteredClipError::LimitReached) => {
            notify::show_when_enabled(state, "登録クリップ", "登録件数の上限に達しています");
        }
        Err(AddRegisteredClipError::ImageTooLarge | AddRegisteredClipError::ImageInvalid) => {}
    }
}
