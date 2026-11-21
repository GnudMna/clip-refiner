use std::sync::Arc;
use std::sync::mpsc::Sender;

use super::super::dispatch;
use super::super::notify::{
    NotificationCategory, show_category_notification, show_image_process_notification,
    show_process_notification,
};
use super::super::state::{AppEvent, AppState};
use super::command::ClipboardCommand;

use crate::config::{AddRegisteredClipError, ResolvedClip};
use crate::ipc::OneshotProcessResult;
use crate::refiner::{
    ClipboardProcessOutcome, ImageClipboard, RefineContext, RefineMode, TextClipboard,
    process_clipboard_pipeline_io,
};
use crate::security::SecretString;
use crate::user_error::{
    ClipboardWriteContext, UserNotification, registered_clip_register_failure, titles,
};

// ======================================================================
// コマンド処理
// ======================================================================
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
                ClipboardWriteContext::HistoryRestore
                    .notification()
                    .notify_with_debug(&e);
            } else {
                state.clipboard_session().record_clipboard_set(&text);
                show_category_notification(
                    state,
                    NotificationCategory::HistoryRestore,
                    "履歴から復元",
                    "クリップボードにコピーしました",
                );
            }
        }
        ClipboardCommand::CopyRegisteredClip(index) => {
            copy_registered_clip_to_clipboard(clipboard, state, index);
        }
        ClipboardCommand::SetOcrText(text) => {
            if let Err(e) = clipboard.set_text(text.to_string()) {
                ClipboardWriteContext::Ocr
                    .notification()
                    .notify_with_debug(&e);
            } else {
                state.clipboard_session().record_clipboard_set(&text);
                show_category_notification(
                    state,
                    NotificationCategory::Ocr,
                    "OCR",
                    "クリップボードにコピーしました",
                );
            }
        }
        ClipboardCommand::DischargeContinuousRecordAt(index) => {
            set_continuous_record_from_queue(clipboard, state, || {
                state.discharge_continuous_record_at(index)
            });
        }
        ClipboardCommand::PasteContinuousRecordFront => {
            // 書き込み成否に関わらず Ctrl+V を送る (空キュー時は現在のクリップボードを貼付)
            let _ = crate::tray::try_paste_continuous_record_front(state, clipboard);
            dispatch::send_app_event(&state.proxy, AppEvent::SendContinuousRecordPaste);
        }
        ClipboardCommand::ProcessMode(mode) => {
            handle_process_mode_command(clipboard, state, refine_ctx, mode);
        }
        ClipboardCommand::ProcessOneshotPipeline {
            pipeline,
            refine_ctx: oneshot_ctx,
            reply,
        } => {
            handle_process_oneshot_command(clipboard, state, &pipeline, &oneshot_ctx, &reply);
        }
        ClipboardCommand::Undo => {
            handle_undo_command(clipboard, state);
        }
        ClipboardCommand::RegisterClipFromClipboard => {
            register_clip_from_clipboard(clipboard, state);
        }
        // 監視ティックはワーカーループ側で実行する
        ClipboardCommand::ClipboardChanged | ClipboardCommand::Shutdown => {}
    }
}

/// 手動加工コマンドを処理する
fn handle_process_mode_command<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    refine_ctx: &mut RefineContext,
    mode: RefineMode,
) {
    let pre_text = clipboard.get_text().ok();
    refine_ctx.regex = state.with_config(|c| c.regex.clone());
    let pipeline = [mode];
    match process_clipboard_pipeline_io(clipboard, &pipeline, refine_ctx) {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            if let Some(ref pre) = pre_text {
                state.clipboard_session().record_undo_source(pre);
            }
            state
                .clipboard_session()
                .record_processing_success(&processed);
            show_process_notification(state, &pipeline, &processed);
        }
        Ok(ClipboardProcessOutcome::ImageProcessed { width, height }) => {
            if let Some(ref pre) = pre_text {
                state.clipboard_session().record_undo_source(pre);
                state
                    .clipboard_session()
                    .record_image_processing_success(pre);
            }
            show_image_process_notification(state, mode, width, height);
        }
        Ok(ClipboardProcessOutcome::Unchanged) => {
            show_category_notification(
                state,
                NotificationCategory::Process,
                "加工結果",
                "テキストに変更はありませんでした",
            );
        }
        Err(e) => {
            UserNotification::from(&e).notify_with_debug(&e);
        }
    }
}

/// CLI ワンショット要求を処理する (トレイ通知なし・監視エコー回避あり)
fn handle_process_oneshot_command<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    pipeline: &[RefineMode],
    refine_ctx: &RefineContext,
    reply: &Sender<OneshotProcessResult>,
) {
    let pre_text = clipboard.get_text().ok();
    let result = match process_clipboard_pipeline_io(clipboard, pipeline, refine_ctx) {
        Ok(outcome) => {
            match &outcome {
                ClipboardProcessOutcome::Processed(processed) => {
                    if let Some(ref pre) = pre_text {
                        state.clipboard_session().record_undo_source(pre);
                    }
                    state
                        .clipboard_session()
                        .record_processing_success(processed);
                }
                ClipboardProcessOutcome::ImageProcessed { .. } => {
                    if let Some(ref pre) = pre_text {
                        state.clipboard_session().record_undo_source(pre);
                        state
                            .clipboard_session()
                            .record_image_processing_success(pre);
                    }
                }
                ClipboardProcessOutcome::Unchanged => {}
            }
            OneshotProcessResult::from_outcome(&outcome)
        }
        Err(e) => OneshotProcessResult::from_process_error(&e),
    };

    if reply.send(result).is_err() {
        crate::log_warn!("ワンショット IPC の応答チャネルが切断されています");
    }
}

/// 加工の取り消しコマンドを処理する
fn handle_undo_command<C: TextClipboard>(clipboard: &mut C, state: &Arc<AppState>) {
    if let Some(text) = state.clipboard_session().take_undo_source() {
        if let Err(e) = clipboard.set_text(text.to_string()) {
            ClipboardWriteContext::Undo
                .notification()
                .notify_with_debug(&e);
            state.clipboard_session().record_undo_source(&text);
        } else {
            state.clipboard_session().record_processing_success(&text);
            show_category_notification(
                state,
                NotificationCategory::Process,
                "加工の取り消し",
                "クリップボードを加工前の内容に戻しました",
            );
        }
    } else {
        show_category_notification(
            state,
            NotificationCategory::Process,
            "加工の取り消し",
            "取り消せる加工がありません",
        );
    }
}

// ======================================================================
// 連続記録
// ======================================================================
/// 連続記録キューから取り出したテキストをクリップボードへセットする
fn set_continuous_record_from_queue<C: TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    take: impl FnOnce() -> Option<SecretString>,
) {
    let Some(text) = take() else {
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            "連続記録",
            "取り出せる記録がありません",
        );
        return;
    };

    if let Err(e) = clipboard.set_text(text.to_string()) {
        ClipboardWriteContext::ContinuousRecord
            .notification()
            .notify_with_debug(&e);
    } else {
        state.clipboard_session().record_clipboard_set(&text);
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            "連続記録",
            "クリップボードに取り出しました",
        );
    }
}

// ======================================================================
// 登録クリップ
// ======================================================================
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
                ClipboardWriteContext::RegisteredClip
                    .notification()
                    .notify_with_debug(&e);
            } else {
                state.clipboard_session().record_clipboard_set(&text);
                show_category_notification(
                    state,
                    NotificationCategory::RegisteredClip,
                    "登録クリップ",
                    "クリップボードにコピーしました",
                );
            }
        }
        Some(ResolvedClip::Image {
            width,
            height,
            rgba,
        }) => {
            if let Err(e) = clipboard.set_image(width, height, rgba) {
                ClipboardWriteContext::RegisteredClip
                    .notification()
                    .notify_with_debug(&e);
            } else {
                show_category_notification(
                    state,
                    NotificationCategory::RegisteredClip,
                    "登録クリップ",
                    "クリップボードにコピーしました",
                );
            }
        }
        None => {}
    }
}

/// 登録クリップ操作後に設定ウィンドウの pending 状態を解除する
fn refresh_settings_after_register(state: &Arc<AppState>) {
    dispatch::send_app_event(&state.proxy, AppEvent::RefreshSettingsWindow);
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
                show_category_notification(
                    state,
                    NotificationCategory::RegisteredClip,
                    "登録クリップ",
                    "クリップボードの画像を登録しました",
                );
            }
            Err(err) => {
                let notification = registered_clip_register_failure(err);
                show_category_notification(
                    state,
                    NotificationCategory::RegisteredClip,
                    notification.title(),
                    notification.body(),
                );
                refresh_settings_after_register(state);
            }
        }
        return;
    }

    let text = match clipboard.get_text() {
        Ok(text) => text,
        Err(e) => {
            UserNotification::static_message(
                titles::CLIPBOARD,
                "クリップボードの読み取りに失敗しました",
            )
            .notify_with_debug(&e);
            refresh_settings_after_register(state);
            return;
        }
    };

    let outcome = state.with_config_mut(|c| c.add_registered_clip(text));
    match outcome {
        Ok(()) => {
            state.save_config();
            dispatch::send_app_event(&state.proxy, AppEvent::RefreshClips);
            show_category_notification(
                state,
                NotificationCategory::RegisteredClip,
                "登録クリップ",
                "クリップボードの内容を登録しました",
            );
        }
        Err(
            err @ (AddRegisteredClipError::Empty
            | AddRegisteredClipError::TooLarge
            | AddRegisteredClipError::LimitReached),
        ) => {
            let notification = registered_clip_register_failure(err);
            show_category_notification(
                state,
                NotificationCategory::RegisteredClip,
                notification.title(),
                notification.body(),
            );
            refresh_settings_after_register(state);
        }
        Err(AddRegisteredClipError::ImageTooLarge | AddRegisteredClipError::ImageInvalid) => {
            refresh_settings_after_register(state);
        }
    }
}
