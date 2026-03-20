use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use super::notifier;
use super::state::AppState;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

use arboard::Clipboard;

/// UI イベントからバックグラウンドワーカーへ送るコマンド
#[derive(Clone)]
pub enum ClipboardCommand {
    /// 指定されたテキストをクリップボードに設定する
    SetText(String),
    /// 現在のクリップボードのテキストを指定されたモードで加工する
    ProcessMode(RefineMode),
}

impl std::fmt::Debug for ClipboardCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetText(_) => f.debug_tuple("SetText").field(&"...").finish(),
            Self::ProcessMode(mode) => f.debug_tuple("ProcessMode").field(mode).finish(),
        }
    }
}

/// クリップボード処理を非同期に行うワーカースレッドを開始する
///
/// # Arguments
/// * `state` - アプリケーションの共有状態
///
/// # Returns
/// * `Sender<ClipboardCommand>` - ワーカースレッドにコマンドを送信するためのチャネルの送信端
pub fn spawn_clipboard_worker(state: Arc<AppState>) -> Sender<ClipboardCommand> {
    let (tx, rx): (Sender<ClipboardCommand>, Receiver<ClipboardCommand>) = mpsc::channel();

    std::thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                crate::log_error!("クリップボード初期化エラー: {:?}", e);
                notification::show_notification(
                    "クリップボードエラー",
                    "クリップボードの初期化に失敗しました。監視処理は停止します。",
                );
                return;
            }
        };

        while let Ok(cmd) = rx.recv() {
            match cmd {
                ClipboardCommand::SetText(text) => {
                    if let Err(e) = clipboard.set_text(text.clone()) {
                        crate::log_error!("クリップボード設定エラー: {:?}", e);
                        notification::show_notification(
                            "クリップボードエラー",
                            "履歴からの復元処理に失敗しました。",
                        );
                    } else {
                        state.set_last_processed_text(text);
                        if state.show_success_notification() {
                            notification::show_notification(
                                "履歴から復元",
                                "クリップボードにコピーしました",
                            );
                        }
                    }
                }
                ClipboardCommand::ProcessMode(mode) => {
                    if let Some(processed) = process_clipboard(&mut clipboard, mode) {
                        state.set_last_processed_text(processed.clone());
                        notifier::show_process_notification(&state, mode, &processed);
                    }
                }
            }
        }
    });

    tx
}
