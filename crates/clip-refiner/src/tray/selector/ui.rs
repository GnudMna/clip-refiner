//! セレクター系 UI ウィンドウ

use std::sync::Arc;

use super::clip::{ClipSelectorWindow, init_clip_selector};
use super::quick::{QuickSelectorWindow, init_quick_selector};
use crate::tray::ocr::{OcrCaptureWindow, init_ocr_capture};
use crate::tray::state::AppEvent;
use crate::tray::worker::ClipboardWorkerHandle;

use tao::event_loop::EventLoopProxy;

// ======================================================================
// セレクター UI
// ======================================================================
/// セレクター系 UI ウィンドウの束
pub struct SelectorUi {
    /// 変換モード選択用クイックセレクター
    pub quick: QuickSelectorWindow,
    /// 登録クリップ選択用セレクター
    pub clip: ClipSelectorWindow,
    /// 画面範囲選択 OCR 用オーバーレイ
    pub ocr: OcrCaptureWindow,
}

impl SelectorUi {
    /// セレクター系 UI の空シェルを生成する (`WebView` / オーバーレイは初回表示まで作らない)
    ///
    /// # Arguments
    /// * `proxy` - イベント送信用プロキシ
    /// * `clipboard_worker` - クリップボード操作ワーカー
    ///
    /// # Returns
    /// * `Self` - 遅延生成用の `SelectorUi`
    pub fn new(
        proxy: &EventLoopProxy<AppEvent>,
        clipboard_worker: Arc<ClipboardWorkerHandle>,
    ) -> Self {
        let quick = init_quick_selector(proxy);
        let clip = init_clip_selector(proxy);
        let ocr = init_ocr_capture(proxy, clipboard_worker);
        Self { quick, clip, ocr }
    }
}
