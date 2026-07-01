//! イベントループ・ワーカーへの非同期送信と失敗時のログ記録

use super::state::AppEvent;
use super::worker::{ClipboardCommand, ClipboardWorkerHandle};

use tao::event_loop::EventLoopProxy;

// ======================================================================
// 送信ヘルパー
// ======================================================================
/// クリップボードワーカーへコマンドを送信する
pub(crate) fn send_clipboard_command(worker: &ClipboardWorkerHandle, command: ClipboardCommand) {
    worker.send(command);
}

/// UI イベントループへカスタムイベントを送信する
///
/// アプリ終了中などで送信できない場合は警告ログを記録する
pub(crate) fn send_app_event(proxy: &EventLoopProxy<AppEvent>, event: AppEvent) {
    if let Err(err) = proxy.send_event(event) {
        crate::log_warn!("UI イベントの送信に失敗 (アプリ終了中の可能性): {:?}", err);
    }
}

/// メニュー再構築などの失敗を警告ログに記録する
pub(crate) fn log_menu_operation_error(context: &str, err: impl std::fmt::Display) {
    crate::log_warn!("{context}: {err}");
}
