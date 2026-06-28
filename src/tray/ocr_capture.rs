//! 画面範囲選択と OCR キャプチャ用オーバーレイ

use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::time::Duration;

use super::event::run_ocr_on_image;
use super::worker::ClipboardCommand;

use crate::platform::ocr_overlay::OverlayWindow;
use crate::platform::screen_capture::capture_screen_region;

use anyhow::Result;

// ======================================================================
// OCR キャプチャウィンドウ構造体
// ======================================================================
/// 画面範囲選択用の全画面オーバーレイを管理する構造体
pub struct OcrCaptureWindow {
    overlay: RefCell<OverlayWindow>,
}

// ======================================================================
// 初期化
// ======================================================================
/// Win32 レイヤードオーバーレイを初期化して生成する
pub fn init_ocr_capture(clipboard_tx: Sender<ClipboardCommand>) -> Result<OcrCaptureWindow> {
    let overlay = OverlayWindow::create(Box::new(move |screen_rect| {
        let clipboard = clipboard_tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            match capture_screen_region(screen_rect) {
                Ok(image) => run_ocr_on_image(&image, &clipboard),
                Err(err) => {
                    crate::log_warn!("OCR 用の領域キャプチャに失敗: {err:#}");
                    crate::platform::show_notification(
                        "OCR エラー",
                        "選択範囲のキャプチャに失敗しました",
                    );
                }
            }
        });
    }))?;

    Ok(OcrCaptureWindow {
        overlay: RefCell::new(overlay),
    })
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl OcrCaptureWindow {
    /// 半透明オーバーレイを即座に表示する
    pub fn show(&self) {
        if let Ok(mut overlay) = self.overlay.try_borrow_mut() {
            overlay.show();
        }
    }

    /// オーバーレイを非表示にする
    pub fn hide(&self) {
        if let Ok(mut overlay) = self.overlay.try_borrow_mut() {
            overlay.hide();
        }
    }

    /// オーバーレイが表示中かどうか
    pub fn is_visible(&self) -> bool {
        self.overlay.borrow().is_visible()
    }
}
