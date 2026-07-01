use crate::platform::{self, screen_capture::RgbaImage};
use crate::security::secret_from;
use crate::tray::dispatch;
use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

// ======================================================================
// OCR 実行
// ======================================================================
/// 画像から OCR を実行し、結果をクリップボードへ書き込む
///
/// 呼び出し元でバックグラウンドスレッドへ逃がすこと
pub(crate) fn run_ocr_on_image(image: &RgbaImage, worker: &ClipboardWorkerHandle) {
    match platform::ocr::recognize_text(image) {
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
