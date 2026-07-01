use crate::platform::{self, screen_capture::RgbaImage};
use crate::security::secret_from;
use crate::tray::dispatch;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

use anyhow::Result;

// ======================================================================
// OCR 実行
// ======================================================================
/// 画像から OCR を実行し、結果をクリップボードへ書き込む
///
/// 呼び出し元でバックグラウンドスレッドへ逃がすこと
pub(crate) fn run_ocr_on_image(image: &RgbaImage, worker: &ClipboardWorkerHandle) {
    dispatch_ocr_result(platform::ocr::recognize_text(image), worker);
}

/// OCR 認識結果をワーカーへ送るか、エラー・空結果を通知する
pub(crate) fn dispatch_ocr_result(result: Result<String>, worker: &ClipboardWorkerHandle) {
    match result {
        Ok(text) if text.trim().is_empty() => {
            platform::show_notification("OCR", "テキストを検出できませんでした");
        }
        Ok(text) => {
            dispatch::send_clipboard_command(
                worker,
                ClipboardCommand::SetOcrText(secret_from(text)),
            );
        }
        Err(err) => {
            crate::log_warn!("OCR に失敗: {err:#}");
            platform::show_notification("OCR エラー", &format!("{err:#}"));
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;

    use super::*;

    use crate::tray::state::test_app_state;
    use crate::tray::worker::ClipboardWorkerHandle;

    /// OCR 認識成功時は `SetOcrText` コマンドを送信すること
    #[test]
    fn dispatch_ocr_result_sends_set_ocr_text_command() {
        let state = Arc::new(test_app_state());
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(state, tx);

        dispatch_ocr_result(Ok("recognized".to_string()), &worker);

        match rx.recv().expect("SetOcrText が送信される") {
            ClipboardCommand::SetOcrText(text) => assert_eq!(text.to_string(), "recognized"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// 空白のみの OCR 結果はコマンドを送らないこと
    #[test]
    fn dispatch_ocr_result_empty_text_sends_no_command() {
        let state = Arc::new(test_app_state());
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(state, tx);

        dispatch_ocr_result(Ok("   \n".to_string()), &worker);

        assert!(rx.try_recv().is_err());
    }

    /// OCR 失敗時はコマンドを送らないこと
    #[test]
    fn dispatch_ocr_result_error_sends_no_command() {
        let state = Arc::new(test_app_state());
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(state, tx);

        dispatch_ocr_result(Err(anyhow::anyhow!("Tesseract の初期化に失敗")), &worker);

        assert!(rx.try_recv().is_err());
    }
}
