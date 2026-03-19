use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use arboard::Clipboard;

use super::notifier;
use super::state::AppState;
use crate::notification;
use crate::refiner::{RefineMode, process_clipboard};

/// UI イベントからバックグラウンドワーカーへ送るコマンド
#[derive(Debug, Clone)]
pub enum ClipboardCommand {
    /// 指定されたテキストをクリップボードに設定する
    SetText(String),
    /// 現在のクリップボードのテキストを指定されたモードで加工する
    ProcessMode(RefineMode),
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
                notification::show_anyhow_error("クリップボード初期化エラー", &anyhow::anyhow!(e));
                return;
            }
        };

        while let Ok(cmd) = rx.recv() {
            match cmd {
                ClipboardCommand::SetText(text) => {
                    if let Err(e) = clipboard.set_text(text.clone()) {
                        notification::show_anyhow_error(
                            "クリップボード設定エラー",
                            &anyhow::anyhow!(e),
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
