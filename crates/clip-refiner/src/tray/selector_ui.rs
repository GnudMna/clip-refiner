//! セレクター系 UI ウィンドウ

use std::sync::Arc;

use super::clip_selector::{ClipSelectorWindow, init_clip_selector};
use super::ocr::{OcrCaptureWindow, init_ocr_capture};
use super::quick_selector::{QuickSelectorWindow, init_quick_selector};
use super::state::AppEvent;
use super::worker::ClipboardWorkerHandle;

use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopProxy};

// ======================================================================
// セレクター UI
// ======================================================================
/// セレクター系 UI ウィンドウの束
pub struct SelectorUi {
    /// 加工モード選択用クイックセレクター
    pub quick: QuickSelectorWindow,
    /// 登録クリップ選択用セレクター
    pub clip: ClipSelectorWindow,
    /// 画面範囲選択 OCR 用オーバーレイ
    pub ocr: OcrCaptureWindow,
}

impl SelectorUi {
    /// イベントループ上にセレクター系ウィンドウを生成する
    ///
    /// # Errors
    /// いずれかのウィンドウ初期化に失敗した場合
    pub fn new(
        event_loop: &EventLoop<AppEvent>,
        proxy: &EventLoopProxy<AppEvent>,
        clipboard_worker: Arc<ClipboardWorkerHandle>,
    ) -> Result<Self> {
        let quick = init_quick_selector(event_loop, proxy)?;
        let clip = init_clip_selector(event_loop, proxy)?;
        let ocr = init_ocr_capture(event_loop, proxy, clipboard_worker)?;
        Ok(Self { quick, clip, ocr })
    }
}
