//! `AddClipboardFormatListener` によるクリップボード変更通知
//!
//! 本文は読まない。起床だけをワーカーへ伝え、`OpenClipboard` は `OwnedClipboard` 側に任せる

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use clipboard_win::monitor::{Monitor, Shutdown};
use thiserror::Error;

use super::command::ClipboardCommand;

// ======================================================================
// エラー
// ======================================================================
/// クリップボード変更リスナの起動失敗
#[derive(Debug, Error)]
pub(super) enum ClipboardListenerError {
    /// 専用スレッドの spawn 失敗
    #[error("リスナスレッドの起動に失敗: {0}")]
    Spawn(String),
    /// メッセージウィンドウ / リスナ登録の初期化失敗
    #[error("クリップボードリスナの初期化に失敗: {0}")]
    Init(String),
    /// 準備完了通知前にスレッドが終了した
    #[error("リスナスレッドが準備完了前に終了した")]
    ReadyChannelClosed,
}

// ======================================================================
// 連打抑制
// ======================================================================
/// 未処理の変更通知が無ければ `ClipboardChanged` を送る
///
/// 既に未処理フラグが立っている場合は送信を省略する
///
/// # Arguments
/// * `pending` - 未処理の `ClipboardChanged` があるかを示すフラグ
/// * `tx` - ワーカーへのコマンド送信チャネル
///
/// # Returns
/// * `bool` - 新たにコマンドを送った場合は `true`
pub(crate) fn try_notify_clipboard_changed(
    pending: &AtomicBool,
    tx: &Sender<ClipboardCommand>,
) -> bool {
    if pending.swap(true, Ordering::AcqRel) {
        return false;
    }

    if tx.send(ClipboardCommand::ClipboardChanged).is_err() {
        pending.store(false, Ordering::Release);
        return false;
    }
    true
}

/// 未処理フラグを下げ、次の変更通知を受けられるようにする
///
/// # Arguments
/// * `pending` - 未処理の `ClipboardChanged` があるかを示すフラグ
pub(crate) fn clear_clipboard_changed_pending(pending: &AtomicBool) {
    pending.store(false, Ordering::Release);
}

// ======================================================================
// リスナハンドル
// ======================================================================
/// クリップボード変更を専用スレッドで待ち、ワーカーへ起床通知する
pub(super) struct ClipboardFormatListener {
    shutdown: Mutex<Option<Shutdown>>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl ClipboardFormatListener {
    /// リスナスレッドを起動する
    ///
    /// # Arguments
    /// * `tx` - ワーカーへのコマンド送信チャネル
    /// * `pending` - `ClipboardChanged` の連打抑制フラグ
    ///
    /// # Returns
    /// * `Result<Self, ClipboardListenerError>` - 起動成功時はハンドル
    pub fn start(
        tx: Sender<ClipboardCommand>,
        pending: Arc<AtomicBool>,
    ) -> Result<Self, ClipboardListenerError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("clip-refiner-clipboard-listener".into())
            .spawn(move || run_listener_loop(&tx, &pending, &ready_tx))
            .map_err(|err| ClipboardListenerError::Spawn(err.to_string()))?;

        let shutdown = match ready_rx.recv() {
            Ok(Ok(shutdown)) => shutdown,
            Ok(Err(err)) => return Err(err),
            Err(_) => return Err(ClipboardListenerError::ReadyChannelClosed),
        };

        Ok(Self {
            shutdown: Mutex::new(Some(shutdown)),
            join: Mutex::new(Some(join)),
        })
    }

    /// リスナを停止し、スレッド終了を待つ
    ///
    /// # Arguments
    /// * `self` - 自身
    pub fn stop(&self) {
        if let Ok(mut guard) = self.shutdown.lock() {
            guard.take();
        }
        if let Ok(mut guard) = self.join.lock()
            && let Some(join) = guard.take()
        {
            let _ = join.join();
        }
    }
}

impl Drop for ClipboardFormatListener {
    fn drop(&mut self) {
        self.stop();
    }
}

/// メッセージ専用ウィンドウでクリップボード変更を待ち、ワーカーへ通知する
fn run_listener_loop(
    tx: &Sender<ClipboardCommand>,
    pending: &Arc<AtomicBool>,
    ready_tx: &Sender<Result<Shutdown, ClipboardListenerError>>,
) {
    let mut monitor = match Monitor::new() {
        Ok(monitor) => monitor,
        Err(err) => {
            let _ = ready_tx.send(Err(ClipboardListenerError::Init(err.to_string())));
            return;
        }
    };

    let shutdown = monitor.shutdown_channel();
    if ready_tx.send(Ok(shutdown)).is_err() {
        return;
    }

    loop {
        match monitor.recv() {
            Ok(true) => {
                let _ = try_notify_clipboard_changed(pending, tx);
            }
            Ok(false) => break,
            Err(err) => {
                crate::log_warn!("クリップボードリスナがエラーで終了: {err}");
                break;
            }
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    use super::*;

    /// 未処理が無いときは `ClipboardChanged` を送るようにすること
    #[test]
    fn try_notify_sends_when_not_pending() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_clipboard_changed(&pending, &tx));
        assert!(pending.load(Ordering::Acquire));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ClipboardChanged)
        ));
    }

    /// 未処理中は追加の `ClipboardChanged` を送らないようにすること
    #[test]
    fn try_notify_coalesces_while_pending() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_clipboard_changed(&pending, &tx));
        assert!(!try_notify_clipboard_changed(&pending, &tx));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ClipboardChanged)
        ));
        assert!(rx.try_recv().is_err());
    }

    /// フラグを下げた後は再び通知できるようにすること
    #[test]
    fn clear_pending_allows_next_notify() {
        let (tx, rx) = mpsc::channel();
        let pending = AtomicBool::new(false);

        assert!(try_notify_clipboard_changed(&pending, &tx));
        let _ = rx.try_recv();
        clear_clipboard_changed_pending(&pending);

        assert!(try_notify_clipboard_changed(&pending, &tx));
        assert!(matches!(
            rx.try_recv(),
            Ok(ClipboardCommand::ClipboardChanged)
        ));
    }

    /// 送信失敗時は未処理フラグを戻すようにすること
    #[test]
    fn try_notify_clears_pending_when_send_fails() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let pending = AtomicBool::new(false);

        assert!(!try_notify_clipboard_changed(&pending, &tx));
        assert!(!pending.load(Ordering::Acquire));
    }

    /// 起動失敗の `Display` がユーザー向け文言になるようにすること
    #[test]
    fn clipboard_listener_error_display() {
        let spawn = ClipboardListenerError::Spawn("os error".into());
        assert!(spawn.to_string().contains("リスナスレッドの起動に失敗"));

        let init = ClipboardListenerError::Init("monitor".into());
        assert!(
            init.to_string()
                .contains("クリップボードリスナの初期化に失敗")
        );

        assert_eq!(
            ClipboardListenerError::ReadyChannelClosed.to_string(),
            "リスナスレッドが準備完了前に終了した"
        );
    }
}
