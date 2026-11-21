//! 画面範囲選択と OCR キャプチャ用オーバーレイ

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use crate::platform::ocr_overlay::OverlayWindow;
use crate::platform::screen_capture::{ScreenRect, capture_screen_region};
use crate::tray::dispatch;
use crate::tray::event::run_ocr_on_image;
use crate::tray::state::AppEvent;
use crate::tray::worker::ClipboardWorkerHandle;

use anyhow::{Context, Result};
use tao::event_loop::EventLoopProxy;

// ======================================================================
// OCR キャプチャウィンドウ構造体
// ======================================================================
/// 画面範囲選択用の全画面オーバーレイを管理する構造体
///
/// オーバーレイは初回表示時に生成し、非表示時に破棄してアイドル RSS を抑える
pub struct OcrCaptureWindow {
    /// イベント送信用プロキシ
    proxy: EventLoopProxy<AppEvent>,
    /// クリップボード操作ワーカー
    worker: Arc<ClipboardWorkerHandle>,
    /// 遅延生成されたオーバーレイ (非表示時は `None`)
    overlay: RefCell<Option<OverlayWindow>>,
    /// テスト用の擬似表示フラグ (実 HWND なしで相互排他を検証する)
    #[cfg(test)]
    force_visible: Cell<bool>,
    /// テスト時に `show` で実オーバーレイを作らず擬似表示する
    #[cfg(test)]
    simulate_show: Cell<bool>,
}

// ======================================================================
// 初期化
// ======================================================================
/// 画面範囲選択オーバーレイの空シェルを生成する
///
/// # Arguments
/// * `proxy` - イベント送信用プロキシ
/// * `worker` - クリップボード操作ワーカー
///
/// # Returns
/// * `OcrCaptureWindow` - 遅延生成用シェル
pub fn init_ocr_capture(
    proxy: &EventLoopProxy<AppEvent>,
    worker: Arc<ClipboardWorkerHandle>,
) -> OcrCaptureWindow {
    OcrCaptureWindow {
        proxy: proxy.clone(),
        worker,
        overlay: RefCell::new(None),
        #[cfg(test)]
        force_visible: Cell::new(false),
        #[cfg(test)]
        simulate_show: Cell::new(false),
    }
}

// ======================================================================
// ウィンドウ操作
// ======================================================================
impl OcrCaptureWindow {
    /// 未生成ならオーバーレイを生成する
    fn ensure_created(&self) -> Result<()> {
        if self.overlay.borrow().is_some() {
            return Ok(());
        }
        let worker = Arc::clone(&self.worker);
        let proxy = self.proxy.clone();
        let overlay = OverlayWindow::create(Box::new(move |screen_rect| {
            if let Some(screen_rect) = screen_rect {
                let worker = Arc::clone(&worker);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(50));
                    run_capture_and_ocr(screen_rect, &worker);
                });
            }
            // wndproc から戻ったあとに破棄する (同一コール内の DestroyWindow を避ける)
            dispatch::send_app_event(&proxy, AppEvent::DisposeOcrOverlay);
        }))
        .context("OCR オーバーレイの生成に失敗")?;
        *self.overlay.borrow_mut() = Some(overlay);
        Ok(())
    }

    /// 半透明オーバーレイを即座に表示する
    ///
    /// 未生成ならオーバーレイを遅延生成する
    ///
    /// # Arguments
    /// * `self` - この `OcrCaptureWindow`
    pub fn show(&self) {
        #[cfg(test)]
        if self.simulate_show.get() {
            self.force_visible.set(true);
            return;
        }
        if let Err(err) = self.ensure_created() {
            crate::log_warn!("OCR オーバーレイの初期化に失敗: {err:#}");
            crate::platform::show_notification("OCR", "オーバーレイの初期化に失敗しました");
            return;
        }
        if let Ok(mut overlay) = self.overlay.try_borrow_mut()
            && let Some(overlay) = overlay.as_mut()
        {
            overlay.show();
        }
    }

    /// オーバーレイを非表示にし、ウィンドウを破棄する
    ///
    /// # Arguments
    /// * `self` - この `OcrCaptureWindow`
    pub fn hide(&self) {
        #[cfg(test)]
        self.force_visible.set(false);
        if let Ok(mut overlay) = self.overlay.try_borrow_mut()
            && let Some(mut overlay) = overlay.take()
        {
            overlay.hide();
        }
    }

    /// オーバーレイが表示中かどうか
    ///
    /// # Arguments
    /// * `self` - この `OcrCaptureWindow`
    ///
    /// # Returns
    /// * `bool` - 表示中なら `true`
    pub fn is_visible(&self) -> bool {
        #[cfg(test)]
        if self.force_visible.get() {
            return true;
        }
        self.overlay
            .borrow()
            .as_ref()
            .is_some_and(OverlayWindow::is_visible)
    }

    /// テスト用に表示中として扱う (実 HWND は作らない)
    ///
    /// # Arguments
    /// * `visible` - 擬似表示するか
    #[cfg(test)]
    pub(crate) fn force_visible_for_test(&self, visible: bool) {
        self.force_visible.set(visible);
    }

    /// テスト時の `show` を擬似表示のみにし、実オーバーレイ生成を避ける
    #[cfg(test)]
    pub(crate) fn enable_show_simulation_for_test(&self) {
        self.simulate_show.set(true);
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

    /// 空シェルは非表示で、`hide` が no-op であること
    #[test]
    fn empty_shell_is_not_visible_and_hide_is_noop() {
        let state = Arc::new(test_app_state());
        let (tx, _) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(Arc::clone(&state), tx);
        let ocr = init_ocr_capture(&state.proxy, worker);

        assert!(!ocr.is_visible());
        ocr.hide();
        assert!(!ocr.is_visible());
    }

    /// 擬似表示の `show` / `hide` が実 HWND なしで切り替わること
    #[test]
    fn simulated_show_and_hide_toggle_visibility() {
        let state = Arc::new(test_app_state());
        let (tx, _) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(Arc::clone(&state), tx);
        let ocr = init_ocr_capture(&state.proxy, worker);
        ocr.enable_show_simulation_for_test();

        ocr.show();
        assert!(ocr.is_visible());
        ocr.hide();
        assert!(!ocr.is_visible());
    }
}
