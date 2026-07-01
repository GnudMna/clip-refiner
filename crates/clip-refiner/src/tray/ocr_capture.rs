//! 画面範囲選択と OCR キャプチャ用オーバーレイ

use std::sync::Arc;
use std::time::Duration;

use super::event::run_ocr_on_image;
use super::worker::ClipboardWorkerHandle;

use crate::platform::screen_capture::{ScreenRect, capture_screen_region};

use anyhow::Result;
use tao::event_loop::EventLoopProxy;
use tao::event_loop::EventLoopWindowTarget;

use super::state::AppEvent;

#[cfg(windows)]
use crate::platform::ocr_overlay::OverlayWindow;

#[cfg(windows)]
use std::cell::RefCell;

#[cfg(not(windows))]
use super::dispatch;
#[cfg(not(windows))]
use super::selector_window::WebSelectorWindow;
#[cfg(not(windows))]
use crate::platform::screen_capture::virtual_screen_bounds;
#[cfg(not(windows))]
use serde::Deserialize;
#[cfg(not(windows))]
use tao::dpi::{LogicalSize, PhysicalPosition};
#[cfg(not(windows))]
use tao::window::{Fullscreen, WindowBuilder, WindowId};

// ======================================================================
// OCR キャプチャウィンドウ構造体
// ======================================================================
/// 画面範囲選択用の全画面オーバーレイを管理する構造体
pub struct OcrCaptureWindow {
    #[cfg(windows)]
    overlay: RefCell<OverlayWindow>,
    #[cfg(not(windows))]
    overlay: WebOcrOverlay,
}

#[cfg(not(windows))]
struct WebOcrOverlay {
    selector: WebSelectorWindow,
}

// ======================================================================
// IPC
// ======================================================================
#[cfg(not(windows))]
#[derive(Debug, Deserialize)]
struct OcrSelectionMessage {
    action: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(not(windows))]
enum OcrOverlayIpcAction {
    Select(ScreenRect),
    Cancel,
}

#[cfg(not(windows))]
fn parse_ocr_overlay_ipc(msg: &str) -> Option<OcrOverlayIpcAction> {
    if msg == "cancel" {
        return Some(OcrOverlayIpcAction::Cancel);
    }

    let payload: OcrSelectionMessage = serde_json::from_str(msg).ok()?;
    if payload.action != "select" {
        return None;
    }

    Some(OcrOverlayIpcAction::Select(ScreenRect {
        x: payload.x,
        y: payload.y,
        width: payload.width,
        height: payload.height,
    }))
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
    #[cfg(windows)]
    {
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

    #[cfg(not(windows))]
    {
        let html = include_str!("../ui/ocr_overlay.html").to_string();
        let worker_for_ipc = Arc::clone(&worker);
        let proxy_for_ipc = proxy.clone();
        let window = build_ocr_overlay_window(event_loop)?;
        let selector =
            WebSelectorWindow::build(window, "clip-refiner-ocr-webview", html, move |request| {
                let msg = request.body();
                let Some(action) = parse_ocr_overlay_ipc(msg) else {
                    return;
                };

                dispatch::send_app_event(&proxy_for_ipc, AppEvent::HideOcrCapture);
                if let OcrOverlayIpcAction::Select(screen_rect) = action {
                    let worker = Arc::clone(&worker_for_ipc);
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(50));
                        run_capture_and_ocr(screen_rect, &worker);
                    });
                }
            })?;

        Ok(OcrCaptureWindow {
            overlay: WebOcrOverlay { selector },
        })
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl OcrCaptureWindow {
    /// 半透明オーバーレイを即座に表示する
    pub fn show(&self) {
        #[cfg(windows)]
        {
            if let Ok(mut overlay) = self.overlay.try_borrow_mut() {
                overlay.show();
            }
        }
        #[cfg(not(windows))]
        {
            self.overlay.selector.show_with_script("");
        }
    }

    /// オーバーレイを非表示にする
    pub fn hide(&self) {
        #[cfg(windows)]
        {
            if let Ok(mut overlay) = self.overlay.try_borrow_mut() {
                overlay.hide();
            }
        }
        #[cfg(not(windows))]
        {
            self.overlay.selector.hide();
        }
    }

    /// オーバーレイが表示中かどうか
    pub fn is_visible(&self) -> bool {
        #[cfg(windows)]
        {
            self.overlay.borrow().is_visible()
        }
        #[cfg(not(windows))]
        {
            self.overlay.selector.is_visible()
        }
    }

    /// ウィンドウ ID を返す (WebView オーバーレイのイベントルーティング用)
    #[cfg(not(windows))]
    pub fn id(&self) -> WindowId {
        self.overlay.selector.id()
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

#[cfg(not(windows))]
fn build_ocr_overlay_window(
    event_loop: &EventLoopWindowTarget<AppEvent>,
) -> Result<tao::window::Window> {
    let (origin_x, origin_y, width, height) = virtual_screen_bounds();
    let width = width.max(800);
    let height = height.max(600);

    let mut builder = WindowBuilder::new()
        .with_title("ClipRefiner OCR")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_visible(false)
        .with_resizable(false)
        .with_inner_size(LogicalSize::new(f64::from(width), f64::from(height)));

    if width > 0 && height > 0 {
        builder = builder.with_position(PhysicalPosition::new(origin_x, origin_y));
    }

    let window = builder.build(event_loop)?;

    if origin_x == 0 && origin_y == 0 && width > 0 && height > 0 {
        let _ = window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    }

    Ok(window)
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(all(test, not(windows)))]
mod tests {
    use super::{OcrOverlayIpcAction, parse_ocr_overlay_ipc};

    /// OCR オーバーレイ IPC: 範囲選択 JSON を解釈すること
    #[test]
    fn parse_ocr_overlay_ipc_select() {
        let action =
            parse_ocr_overlay_ipc(r#"{"action":"select","x":10,"y":20,"width":100,"height":50}"#)
                .expect("select IPC を解釈できる");
        match action {
            OcrOverlayIpcAction::Select(rect) => {
                assert_eq!(rect.x, 10);
                assert_eq!(rect.y, 20);
                assert_eq!(rect.width, 100);
                assert_eq!(rect.height, 50);
            }
            OcrOverlayIpcAction::Cancel => panic!("cancel ではない"),
        }
    }

    /// OCR オーバーレイ IPC: cancel を解釈すること
    #[test]
    fn parse_ocr_overlay_ipc_cancel() {
        assert!(matches!(
            parse_ocr_overlay_ipc("cancel"),
            Some(OcrOverlayIpcAction::Cancel)
        ));
    }
}
