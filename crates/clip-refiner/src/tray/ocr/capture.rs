//! 画面範囲選択と OCR キャプチャ用オーバーレイ

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::platform::ocr;
use crate::platform::ocr_overlay::OverlayWindow;
use crate::platform::screen_capture::{ScreenRect, capture_screen_region};
use crate::tray::dispatch;
use crate::tray::event::dispatch_ocr_result;
use crate::tray::state::{AppEvent, LockExt};
use crate::tray::worker::ClipboardWorkerHandle;

use anyhow::{Context, Result};
use tao::event_loop::EventLoopProxy;

// ======================================================================
// OCR ジョブ制御
// ======================================================================
/// OCR キャプチャジョブの同時実行制御
///
/// 新しい要求は世代を進め、古いジョブの結果適用と通知を無効化する
/// キャプチャと認識は `slot` で 1 本に直列化する
struct OcrJobGate {
    generation: AtomicU64,
    slot: Mutex<()>,
}

impl OcrJobGate {
    /// 空のゲートを生成する
    ///
    /// # Returns
    /// * `Self` - 世代 0・スロット空きの初期状態
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            slot: Mutex::new(()),
        }
    }

    /// 新しいジョブを開始し、その世代番号を返す
    ///
    /// # Returns
    /// * `u64` - 開始したジョブの世代番号 (1 始まり)
    fn begin_job(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 指定世代が最新かどうか判定する
    ///
    /// # Arguments
    /// * `generation` - 判定対象のジョブ世代
    ///
    /// # Returns
    /// * `bool` - 最新世代なら `true`
    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
}

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
    /// OCR ジョブの世代と同時実行スロット
    job_gate: Arc<OcrJobGate>,
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
        job_gate: Arc::new(OcrJobGate::new()),
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
        let job_gate = Arc::clone(&self.job_gate);
        let overlay = OverlayWindow::create(Box::new(move |screen_rect| {
            if let Some(screen_rect) = screen_rect {
                let worker = Arc::clone(&worker);
                let job_gate = Arc::clone(&job_gate);
                let generation = job_gate.begin_job();
                std::thread::spawn(move || {
                    run_capture_and_ocr(screen_rect, &worker, &job_gate, generation);
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
///
/// オーバーレイ破棄待ちのあと、スロットを取得してキャプチャと認識を直列実行する
/// 世代が古い場合は途中でも打ち切り、結果の書き込み・通知を行わない
fn run_capture_and_ocr(
    screen_rect: ScreenRect,
    worker: &ClipboardWorkerHandle,
    gate: &OcrJobGate,
    generation: u64,
) {
    std::thread::sleep(Duration::from_millis(50));
    if !gate.is_current(generation) {
        crate::log_info!("新しい OCR 要求によりキャプチャ前のジョブを破棄");
        return;
    }

    let _slot = gate.slot.lock_ignore_poison();
    if !gate.is_current(generation) {
        crate::log_info!("新しい OCR 要求によりスロット待ち後のジョブを破棄");
        return;
    }

    match capture_screen_region(screen_rect) {
        Ok(image) => {
            if !gate.is_current(generation) {
                crate::log_info!("新しい OCR 要求により認識前のジョブを破棄");
                return;
            }
            let result = ocr::recognize_text(&image);
            apply_ocr_job_result(gate, generation, result, worker);
        }
        Err(err) => {
            if !gate.is_current(generation) {
                return;
            }
            crate::log_warn!("OCR 用の領域キャプチャに失敗: {err:#}");
            crate::platform::show_notification("OCR エラー", "選択範囲のキャプチャに失敗しました");
        }
    }
}

/// 最新世代のジョブだけ OCR 結果をワーカーへ送る
///
/// # Arguments
/// * `gate` - OCR ジョブ制御
/// * `generation` - このジョブの世代番号
/// * `result` - OCR 認識結果
/// * `worker` - クリップボード操作ワーカー
///
/// # Returns
/// * `bool` - 結果を適用した場合は `true`、世代失効で破棄した場合は `false`
fn apply_ocr_job_result(
    gate: &OcrJobGate,
    generation: u64,
    result: Result<String>,
    worker: &ClipboardWorkerHandle,
) -> bool {
    if !gate.is_current(generation) {
        crate::log_info!("新しい OCR 要求により OCR 結果を破棄");
        return false;
    }
    dispatch_ocr_result(result, worker);
    true
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    use crate::tray::state::test_app_state;
    use crate::tray::worker::{ClipboardCommand, ClipboardWorkerHandle};

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

    /// 新しいジョブ開始で旧世代が無効になること
    #[test]
    fn begin_job_invalidates_previous_generation() {
        let gate = OcrJobGate::new();
        let first = gate.begin_job();
        assert!(gate.is_current(first));

        let second = gate.begin_job();
        assert!(!gate.is_current(first));
        assert!(gate.is_current(second));
    }

    /// 最新世代の OCR 結果だけワーカーへ送ること
    #[test]
    fn apply_ocr_job_result_dispatches_only_current_generation() {
        let state = Arc::new(test_app_state());
        let (tx, rx) = mpsc::channel();
        let worker = ClipboardWorkerHandle::for_test(state, tx);
        let gate = OcrJobGate::new();
        let first = gate.begin_job();
        let second = gate.begin_job();

        assert!(!apply_ocr_job_result(
            &gate,
            first,
            Ok("stale".to_string()),
            &worker
        ));
        assert!(rx.try_recv().is_err());

        assert!(apply_ocr_job_result(
            &gate,
            second,
            Ok("fresh".to_string()),
            &worker
        ));
        match rx.recv().expect("最新世代の SetOcrText が送信される") {
            ClipboardCommand::SetOcrText(text) => assert_eq!(text.to_string(), "fresh"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// スロット保持中に次世代が始まると待ち後の旧ジョブを破棄すること
    #[test]
    fn superseded_job_exits_after_waiting_for_slot() {
        let gate = Arc::new(OcrJobGate::new());
        let first = gate.begin_job();
        let slot_guard = gate.slot.lock_ignore_poison();

        let gate_thread = Arc::clone(&gate);
        let handle = thread::spawn(move || {
            let _slot = gate_thread.slot.lock_ignore_poison();
            gate_thread.is_current(first)
        });

        // スロット待ちに入ったあと世代を進め、旧ジョブを無効化する
        thread::sleep(Duration::from_millis(30));
        let _second = gate.begin_job();
        drop(slot_guard);

        assert!(!handle.join().expect("スロット待ちスレッドが完了する"));
    }
}
