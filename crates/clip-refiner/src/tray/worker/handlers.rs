use std::sync::Arc;
use std::sync::mpsc::Sender;

use chrono::Local;

use super::super::clipboard_monitor::ObservedClipboard;
use super::super::dispatch;
use super::super::notify::{
    NotificationCategory, show_category_notification, show_image_process_notification,
    show_process_notification,
};
use super::super::state::{AppState, ClipsEvent, RecordEvent, SettingsEvent, UndoLookup};
use super::command::ClipboardCommand;
use crate::clip_template::{
    ClipTemplateContext, clip_template_uses_clipboard, clip_template_uses_counter,
    expand_clip_template,
};
use crate::config::{AddRegisteredClipError, ResolvedClip};
use crate::ipc::OneshotProcessResult;
use crate::refiner::{
    ClipboardProcessOutcome, HtmlClipboard, ImageClipboard, RefineContext, RefineMode,
    TextClipboard, process_clipboard_pipeline_io,
};
use crate::security::{SecretString, secret_from};
use crate::user_error::{
    ClipboardWriteContext, UserNotification, registered_clip_register_failure,
    registered_clip_template_failure, titles,
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
pub(crate) fn handle_command<C: TextClipboard + ImageClipboard + HtmlClipboard>(
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
            set_continuous_record_from_queue(clipboard, state, index);
        }
        ClipboardCommand::PasteContinuousRecordFront => {
            // 空キュー時は現在のクリップボードを貼付。書き込み失敗時は古い内容を貼らないよう送らない
            let outcome = crate::tray::try_paste_continuous_record_front(state, clipboard);
            if outcome.should_send_paste() {
                dispatch::send_app_event(&state.proxy, RecordEvent::SendPaste.into());
            } else {
                ClipboardWriteContext::ContinuousRecord
                    .notification()
                    .notify();
            }
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
        // 監視ティック / 設定再読込はワーカーループ側で実行する
        ClipboardCommand::ClipboardChanged
        | ClipboardCommand::ConfigFileChanged
        | ClipboardCommand::Shutdown => {}
    }
}

/// 手動加工コマンドを処理する
fn handle_process_mode_command<C: TextClipboard + ImageClipboard + HtmlClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    refine_ctx: &mut RefineContext,
    mode: RefineMode,
) {
    // 取り消し用テキストと加工対象を同じ読み取りにそろえ、書き戻し前に別のコピーが入れば上書きしない
    let token = clipboard.change_token();
    let pre_text = clipboard.get_text().ok();
    refine_ctx.regex = state.with_config(|c| c.regex.clone());
    let pipeline = [mode];
    let mut observed = ObservedClipboard::with_observed(clipboard, pre_text.as_deref(), token);
    let outcome = process_clipboard_pipeline_io(&mut observed, &pipeline, refine_ctx);
    if observed.is_superseded() {
        crate::log_debug!("加工中にクリップボードが更新されたため書き戻しを見送り");
        show_category_notification(
            state,
            NotificationCategory::Process,
            "加工結果",
            "加工中にクリップボードが更新されたため、書き戻しを中止しました",
        );
        return;
    }
    match outcome {
        Ok(ClipboardProcessOutcome::Processed(processed)) => {
            if let Some(ref pre) = pre_text {
                state
                    .clipboard_session()
                    .record_undo_source(pre, &processed);
            }
            state
                .clipboard_session()
                .record_processing_success(&processed);
            show_process_notification(state, &pipeline, &processed);
        }
        Ok(ClipboardProcessOutcome::ImageProcessed { width, height }) => {
            if let Some(ref pre) = pre_text {
                state.clipboard_session().record_image_undo_source(pre);
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
        Err(e) if e.is_skip() => {
            UserNotification::from(&e).notify_skip();
        }
        Err(e) => {
            UserNotification::from(&e).notify_with_debug(&e);
        }
    }
}

/// CLI ワンショット要求を処理する (トレイ通知なし・監視エコー回避あり)
fn handle_process_oneshot_command<C: TextClipboard + ImageClipboard + HtmlClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    pipeline: &[RefineMode],
    refine_ctx: &RefineContext,
    reply: &Sender<OneshotProcessResult>,
) {
    // 取り消し用テキストと加工対象を同じ読み取りにそろえ、書き戻し前に別のコピーが入れば上書きしない
    let token = clipboard.change_token();
    let pre_text = clipboard.get_text().ok();
    let mut observed = ObservedClipboard::with_observed(clipboard, pre_text.as_deref(), token);
    let result = match process_clipboard_pipeline_io(&mut observed, pipeline, refine_ctx) {
        Ok(outcome) => {
            match &outcome {
                ClipboardProcessOutcome::Processed(processed) => {
                    if let Some(ref pre) = pre_text {
                        state.clipboard_session().record_undo_source(pre, processed);
                    }
                    state
                        .clipboard_session()
                        .record_processing_success(processed);
                }
                ClipboardProcessOutcome::ImageProcessed { .. } => {
                    if let Some(ref pre) = pre_text {
                        state.clipboard_session().record_image_undo_source(pre);
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
    let current = clipboard.get_text().ok();
    match state
        .clipboard_session()
        .take_undo_entry_if_current(current.as_deref())
    {
        UndoLookup::Ready(entry) => {
            if let Err(e) = clipboard.set_text(entry.source.to_string()) {
                ClipboardWriteContext::Undo
                    .notification()
                    .notify_with_debug(&e);
                state.clipboard_session().store_undo_entry(entry);
            } else {
                state
                    .clipboard_session()
                    .record_processing_success(&entry.source);
                show_category_notification(
                    state,
                    NotificationCategory::Process,
                    "加工の取り消し",
                    "クリップボードを加工前の内容に戻しました",
                );
            }
        }
        UndoLookup::Stale => {
            show_category_notification(
                state,
                NotificationCategory::Process,
                "加工の取り消し",
                "加工後にクリップボードが変更されたため取り消しませんでした",
            );
        }
        UndoLookup::Empty => {
            show_category_notification(
                state,
                NotificationCategory::Process,
                "加工の取り消し",
                "取り消せる加工がありません",
            );
        }
    }
}

// ======================================================================
// 連続記録
// ======================================================================
/// 連続記録キューの指定位置から取り出したテキストをクリップボードへセットする
///
/// 書き込みに失敗した場合は元の位置へ戻す
fn set_continuous_record_from_queue<C: TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    index: usize,
) {
    let Some(text) = state.discharge_continuous_record_at(index) else {
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            "連続記録モード",
            "取り出せる記録がありません",
        );
        return;
    };

    if let Err(e) = clipboard.set_text(text.to_string()) {
        ClipboardWriteContext::ContinuousRecord
            .notification()
            .notify_with_debug(&e);
        state.restore_continuous_record_at(index, text.as_str());
    } else {
        state.clipboard_session().record_clipboard_set(&text);
        show_category_notification(
            state,
            NotificationCategory::ContinuousRecord,
            "連続記録モード",
            "クリップボードに取り出しました",
        );
    }
}

// ======================================================================
// 登録クリップ
// ======================================================================
/// 登録済みクリップボード内容をクリップボードへコピーする
///
/// テンプレート指定のテキストはプレースホルダを展開してからコピーする。
/// 展開に失敗した場合はクリップボードを変更せず通知する。
/// `{counter}` を含むテンプレートは、コピーに成功したときだけ連番を 1 進めて保存する
fn copy_registered_clip_to_clipboard<C: ImageClipboard + TextClipboard>(
    clipboard: &mut C,
    state: &Arc<AppState>,
    index: usize,
) {
    let (clip, template, counter) = state.with_config(|config| {
        let entry = config.clips.get(index);
        (
            config.resolve_registered_clip(index),
            entry.is_some_and(|entry| entry.template),
            entry.map_or(0, |entry| entry.counter),
        )
    });
    match clip {
        Some(ResolvedClip::Text(source)) => {
            let next_counter = counter.saturating_add(1);
            let (text, issued_counter) = if template {
                match expand_registered_clip_template(clipboard, &source, next_counter) {
                    Ok(text) => (
                        text,
                        clip_template_uses_counter(source.as_str()).then_some(next_counter),
                    ),
                    Err(notification) => {
                        notification.notify();
                        return;
                    }
                }
            } else {
                (source, None)
            };
            if let Err(e) = clipboard.set_text(text.to_string()) {
                ClipboardWriteContext::RegisteredClip
                    .notification()
                    .notify_with_debug(&e);
            } else {
                state
                    .clipboard_session()
                    .record_clipboard_set(text.as_str());
                if let Some(issued) = issued_counter {
                    record_issued_counter(state, index, issued);
                }
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

/// 払い出した連番を登録クリップへ記録し、保存して一覧を更新する
fn record_issued_counter(state: &Arc<AppState>, index: usize, counter: u64) {
    if state.with_config_mut(|c| c.set_registered_clip_counter(index, counter)) {
        state.save_config();
        dispatch::send_app_event(&state.proxy, ClipsEvent::Refresh.into());
    }
}

/// 登録クリップのテンプレートを展開する
///
/// `{clipboard}` を含む場合だけ現在のクリップボードのテキストを読む (テキストが無ければ空文字)
///
/// # Arguments
/// * `clipboard` - `{clipboard}` の読み取りに使うクリップボード
/// * `source` - 復号済みのテンプレート本文
/// * `counter` - `{counter}` に差し込む連番 (今回払い出す値)
///
/// # Returns
/// * `Result<SecretString, UserNotification>` - 展開後のテキスト。失敗時は案内用の通知
fn expand_registered_clip_template<C: TextClipboard>(
    clipboard: &mut C,
    source: &SecretString,
    counter: u64,
) -> Result<SecretString, UserNotification> {
    let current = if clip_template_uses_clipboard(source.as_str()) {
        clipboard.get_text().ok().map(secret_from)
    } else {
        None
    };
    let user_name = std::env::var("USERNAME").ok();
    let ctx = ClipTemplateContext {
        now: Local::now().fixed_offset(),
        clipboard: current.as_deref().map(String::as_str),
        user_name: user_name.as_deref(),
        counter,
    };
    expand_clip_template(source.as_str(), &ctx, random_uuid_bytes)
        .map(secret_from)
        .map_err(|err| registered_clip_template_failure(&err))
}

/// `{uuid}` 用の乱数 16 バイトを OS から取得する
fn random_uuid_bytes() -> Option<[u8; 16]> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes)
}

/// 登録クリップ操作後に設定ウィンドウの pending 状態を解除する
fn refresh_settings_after_register(state: &Arc<AppState>) {
    dispatch::send_app_event(&state.proxy, SettingsEvent::Refresh.into());
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
                dispatch::send_app_event(&state.proxy, ClipsEvent::Refresh.into());
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
            dispatch::send_app_event(&state.proxy, ClipsEvent::Refresh.into());
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
            | AddRegisteredClipError::LimitReached
            | AddRegisteredClipError::EncryptFailed),
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
