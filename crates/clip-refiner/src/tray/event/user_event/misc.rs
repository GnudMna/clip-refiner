//! OCR / 連続記録 / `WebView2` プロセスのユーザーイベント処理と共有ヘルパー

use tao::event_loop::ControlFlow;
use tao::window::WindowId;

use crate::tray::e2e;
use crate::tray::event::AppEventContext;
use crate::tray::state::{OcrEvent, RecordEvent, WebViewEvent};
use crate::user_error::webview_process_failure;

// ======================================================================
// OCR
// ======================================================================
/// OCR 関連イベントを処理する
pub(super) fn handle_ocr(ctx: &mut AppEventContext<'_>, event: OcrEvent) {
    match event {
        OcrEvent::DisposeOverlay => ctx.selectors.ocr.hide(),
    }
}

// ======================================================================
// 連続記録
// ======================================================================
/// 連続記録関連イベントを処理する
pub(super) fn handle_record(_ctx: &mut AppEventContext<'_>, event: RecordEvent) {
    match event {
        RecordEvent::SendPaste => {
            crate::tray::send_continuous_record_ctrl_v();
        }
    }
}

// ======================================================================
// WebView
// ======================================================================
/// `WebView2` のプロセス関連イベントを処理する
pub(super) fn handle_webview(ctx: &mut AppEventContext<'_>, event: WebViewEvent) {
    match event {
        WebViewEvent::ProcessFailed(window_id) => dispose_failed_webview(ctx, window_id),
    }
}

/// 異常終了を通知したウィンドウの `WebView` を破棄し、表示中だった場合は利用者へ知らせる
///
/// 作り直しは次の表示に任せる。破棄済みで ID が一致しない通知は無視する
fn dispose_failed_webview(ctx: &AppEventContext<'_>, window_id: WindowId) {
    let disposed = [
        (
            "クイックセレクター",
            ctx.selectors.quick.dispose_failed_webview(window_id),
        ),
        (
            "登録クリップ",
            ctx.selectors.clip.dispose_failed_webview(window_id),
        ),
        ("設定", ctx.settings.dispose_failed_webview(window_id)),
    ]
    .into_iter()
    .find_map(|(label, was_visible)| was_visible.map(|visible| (label, visible)));

    if let Some((label, true)) = disposed {
        webview_process_failure(label).notify();
    }
}

// ======================================================================
// 共有ヘルパー
// ======================================================================
/// E2E 中にセレクターを閉じた場合はプロセスを終了する
///
/// Playwright は `WebView` 破棄による page close を待機するため、
/// 非表示保持でも CDP 切断が起きるよう終了する
pub(super) fn exit_if_e2e_selector_closed(ctx: &mut AppEventContext<'_>) {
    if e2e::is_active() {
        *ctx.control_flow = ControlFlow::Exit;
    }
}
