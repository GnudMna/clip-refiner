//! セレクター系 UI ウィンドウ

use std::sync::Arc;

use super::clip::{ClipSelectorWindow, init_clip_selector};
use super::quick::{QuickSelectorWindow, init_quick_selector};
use crate::tray::ocr::{OcrCaptureWindow, init_ocr_capture};
use crate::tray::state::AppEvent;
use crate::tray::worker::ClipboardWorkerHandle;

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
    /// # Arguments
    /// * `event_loop` - ウィンドウ作成用のイベントループ
    /// * `proxy` - イベント送信用プロキシ
    /// * `clipboard_worker` - クリップボード操作ワーカー
    ///
    /// # Returns
    /// * `Result<Self>` - 成功時は `SelectorUi`、いずれかのウィンドウ初期化に失敗した場合はエラー
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
