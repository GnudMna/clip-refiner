//! 画面範囲選択と OCR キャプチャ用オーバーレイ

use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use crate::platform::ocr_overlay::OverlayWindow;
use crate::platform::screen_capture::{ScreenRect, capture_screen_region};
use crate::tray::event::run_ocr_on_image;
use crate::tray::state::AppEvent;
use crate::tray::worker::ClipboardWorkerHandle;

use anyhow::Result;
use tao::event_loop::EventLoopProxy;
use tao::event_loop::EventLoopWindowTarget;

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
/// 画面範囲選択オーバーレイを初期化して生成する
pub fn init_ocr_capture(
    event_loop: &EventLoopWindowTarget<AppEvent>,
    proxy: &EventLoopProxy<AppEvent>,
    worker: Arc<ClipboardWorkerHandle>,
) -> Result<OcrCaptureWindow> {
    let _ = (event_loop, proxy);
    let overlay = OverlayWindow::create(Box::new(move |screen_rect| {
        let worker = Arc::clone(&worker);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            run_capture_and_ocr(screen_rect, &worker);
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

// ======================================================================
// プライベート関数
// ======================================================================
/// 選択範囲をキャプチャして OCR を実行する
fn run_capture_and_ocr(screen_rect: ScreenRect, worker: &ClipboardWorkerHandle) {
    match capture_screen_region(screen_rect) {
        Ok(image) => run_ocr_on_image(&image, worker),
        Err(err) => {
            crate::log_warn!("OCR 用の領域キャプチャに失敗: {err:#}");
            crate::platform::show_notification("OCR エラー", "選択範囲のキャプチャに失敗しました");
        }
    }
}
