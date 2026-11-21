//! OCR / 連続記録のユーザーイベント処理と共有ヘルパー

use crate::tray::e2e;
use crate::tray::event::AppEventContext;
use crate::tray::state::{OcrEvent, RecordEvent};

use tao::event_loop::ControlFlow;

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
