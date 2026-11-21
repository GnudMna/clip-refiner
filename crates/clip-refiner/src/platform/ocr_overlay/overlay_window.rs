//! Win32 レイヤードウィンドウによる画面範囲選択オーバーレイ

use std::ptr::NonNull;

use anyhow::{Context, Result, bail};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GWLP_USERDATA, HWND_TOPMOST, IsWindowVisible, SW_SHOW,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use super::geometry;
use super::layered::LayeredOverlay;
use super::types::{DragSelection, OVERLAY_CLASS_NAME, OverlayCompleteFn};
use super::wndproc::{OverlayState, hide_overlay, register_overlay_class};
use crate::platform::screen_capture::virtual_screen_bounds;

// ======================================================================
// オーバーレイウィンドウ
// ======================================================================
/// Win32 レイヤードウィンドウによる画面範囲選択オーバーレイ
pub(crate) struct OverlayWindow {
    hwnd: HWND,
    /// `Box::into_raw` で確保した状態 (`GWLP_USERDATA` と共有し、`Drop` で解放)
    ///
    /// ウィンドウプロシージャも同じポインタ経由で触るため、`Box` のまま保持せず生ポインタで持つ
    state: NonNull<OverlayState>,
}

impl OverlayWindow {
    /// 仮想デスクトップ全体を覆うオーバーレイウィンドウを生成する
    ///
    /// # Arguments
    /// * `on_complete` - 範囲選択完了時 (`Some`) またはキャンセル時 (`None`) のコールバック
    ///
    /// # Returns
    /// * `Result<Self>` - 生成した `OverlayWindow`。ウィンドウ生成やレイヤー初期化失敗時は `Err`
    pub fn create(on_complete: OverlayCompleteFn) -> Result<Self> {
        register_overlay_class()?;

        let (virt_x, virt_y, width, height) = virtual_screen_bounds();
        if width == 0 || height == 0 {
            bail!("仮想デスクトップのサイズが取得できない");
        }

        let class_name = geometry::str_to_wide(OVERLAY_CLASS_NAME);
        // SAFETY: `class_name` は NUL 終端の UTF-16 で、呼び出し中は生存する
        // `GWLP_USERDATA` は未設定 (0) のため、生成中のメッセージは `DefWindowProcW` へ流れる
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR::null(),
                WS_POPUP,
                virt_x,
                virt_y,
                i32::try_from(width).context("幅の変換に失敗")?,
                i32::try_from(height).context("高さの変換に失敗")?,
                Some(HWND::default()),
                None,
                GetModuleHandleW(None).ok().map(Into::into),
                None,
            )
        }?;

        let layered = match LayeredOverlay::new(hwnd, width, height) {
            Ok(layered) => layered,
            Err(err) => {
                // SAFETY: 直前に作成した未破棄のウィンドウで、`GWLP_USERDATA` は未設定
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                return Err(err);
            }
        };
        let state = NonNull::from(Box::leak(Box::new(OverlayState {
            layered,
            selection: DragSelection::default(),
            virt_x,
            virt_y,
            on_complete,
            last_redraw_at: None,
        })));

        // SAFETY: `state` はヒープ上に固定され、`Drop` で `GWLP_USERDATA` を 0 に戻してから解放する
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state.as_ptr() as isize);
        }

        Ok(Self { hwnd, state })
    }

    /// 状態への可変参照を返す
    ///
    /// 戻り値の参照を保持したまま Win32 API を呼ばないこと (ウィンドウプロシージャが同じ状態を触るため)
    ///
    /// # Arguments
    /// * `self` - この `OverlayWindow`
    ///
    /// # Returns
    /// * `&mut OverlayState` - 状態
    fn state_mut(&mut self) -> &mut OverlayState {
        // SAFETY: `state` は `Drop` まで解放されない
        // ウィンドウプロシージャはこのスレッドのメッセージ処理中にのみ動き、参照の生存中には呼ばれない
        unsafe { self.state.as_mut() }
    }

    /// オーバーレイを表示する
    ///
    /// # Arguments
    /// * `self` - この `OverlayWindow`
    pub fn show(&mut self) {
        let state = self.state_mut();
        state.selection = DragSelection::default();
        state.last_redraw_at = None;
        if let Err(err) = state.layered.reset_to_dim() {
            crate::log_warn!("オーバーレイの初期描画に失敗: {err:#}");
        }
        // SAFETY: `hwnd` は `self` が所有する生存中のウィンドウ
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let _ = SetForegroundWindow(self.hwnd);
        }
    }

    /// オーバーレイを非表示にする
    ///
    /// # Arguments
    /// * `self` - この `OverlayWindow`
    pub fn hide(&mut self) {
        // SAFETY: `state` は `self` が所有する生存中の状態で、他に参照は生存していない
        unsafe {
            hide_overlay(self.hwnd, self.state.as_ptr());
        }
    }

    /// オーバーレイが表示中かどうか
    ///
    /// # Arguments
    /// * `self` - この `OverlayWindow`
    ///
    /// # Returns
    /// * `bool` - ウィンドウが表示中の場合は `true`
    pub fn is_visible(&self) -> bool {
        // SAFETY: `hwnd` は `self` が所有する生存中のウィンドウ
        unsafe { IsWindowVisible(self.hwnd).as_bool() }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        // SAFETY: `state` の解放前に `GWLP_USERDATA` を 0 に戻し、ウィンドウも破棄して以降は参照させない
        // `state` は `create` の `Box::leak` で確保したもので、ここで 1 度だけ `Box` へ戻して解放する
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            let _ = ReleaseCapture();
            let _ = DestroyWindow(self.hwnd);
            drop(Box::from_raw(self.state.as_ptr()));
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageW, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };

    use super::*;
    use crate::platform::ocr_overlay::geometry::make_lparam;
    use crate::platform::ocr_overlay::types::{BORDER_PIXEL, SelectionRect};
    use crate::platform::screen_capture::ScreenRect;

    impl OverlayWindow {
        /// テスト用に HWND を公開する
        fn hwnd_for_test(&self) -> HWND {
            self.hwnd
        }

        /// テスト用に直前の選択プレビュー矩形を返す
        fn last_preview_for_test(&mut self) -> Option<SelectionRect> {
            self.state_mut().layered.last_preview
        }

        /// テスト用に選択枠の角ピクセル (BGRA) を返す
        fn border_corner_pixel_for_test(&mut self) -> Option<[u8; 4]> {
            let layered = &self.state_mut().layered;
            let preview = layered.last_preview?;
            let width = layered.width_for_test();
            let bgra = layered.bgra_for_test();
            let x = usize::try_from(preview.x).ok()?;
            let y = usize::try_from(preview.y).ok()?;
            let offset = y
                .saturating_mul(usize::try_from(width).ok()?)
                .saturating_add(x)
                .saturating_mul(4);
            bgra.get(offset..offset.saturating_add(4))
                .map(|p| [p[0], p[1], p[2], p[3]])
        }
    }

    /// オーバーレイを生成・表示・非表示・破棄できるようにすること
    #[test]
    fn overlay_lifecycle_create_show_hide() {
        let mut overlay =
            OverlayWindow::create(Box::new(|_| {})).expect("オーバーレイを生成できる");
        overlay.show();
        assert!(overlay.is_visible(), "表示後は可視であること");
        overlay.hide();
        assert!(!overlay.is_visible(), "非表示後は不可視であること");
    }

    /// 完了コールバックの結果を記録するオーバーレイを生成して表示する
    ///
    /// 外側の `Option` はコールバック呼び出しの有無、内側は選択確定 / キャンセルを表す
    #[allow(
        clippy::option_option,
        reason = "コールバック未呼び出しと選択キャンセル (`None`) を区別して検証する"
    )]
    fn create_recording_overlay() -> (OverlayWindow, Arc<Mutex<Option<Option<ScreenRect>>>>) {
        let completed: Arc<Mutex<Option<Option<ScreenRect>>>> = Arc::new(Mutex::new(None));
        let completed_cb = Arc::clone(&completed);
        let mut overlay = OverlayWindow::create(Box::new(move |rect| {
            *completed_cb.lock().expect("lock") = Some(rect);
        }))
        .expect("オーバーレイを生成できる");
        overlay.show();
        (overlay, completed)
    }

    /// オーバーレイへウィンドウメッセージを注入する
    fn send_message(hwnd: HWND, message: u32, wparam: usize, lparam: LPARAM) {
        // SAFETY: `hwnd` はこのスレッドで作成した生存中のオーバーレイで、`lparam` はポインタを含まない
        unsafe {
            let _ = SendMessageW(hwnd, message, Some(WPARAM(wparam)), Some(lparam));
        }
    }

    /// マウスメッセージ注入で範囲選択が確定し枠色が描画されるようにすること
    #[test]
    fn overlay_drag_completes_with_screen_rect_and_border() {
        let (mut overlay, completed) = create_recording_overlay();

        let hwnd = overlay.hwnd_for_test();
        send_message(hwnd, WM_LBUTTONDOWN, 0, make_lparam(20, 30));
        send_message(hwnd, WM_MOUSEMOVE, 0, make_lparam(120, 130));

        assert_eq!(
            overlay.last_preview_for_test(),
            Some(SelectionRect {
                x: 20,
                y: 30,
                width: 100,
                height: 100,
            })
        );
        assert_eq!(overlay.border_corner_pixel_for_test(), Some(BORDER_PIXEL));

        send_message(hwnd, WM_LBUTTONUP, 0, make_lparam(120, 130));

        assert!(!overlay.is_visible(), "確定後は非表示であること");
        let result = *completed.lock().expect("lock");
        let screen = result
            .expect("on_complete が呼ばれる")
            .expect("選択が確定する");
        assert!(screen.width >= 5);
        assert!(screen.height >= 5);
    }

    /// Escape で選択をキャンセルできるようにすること
    #[test]
    fn overlay_escape_cancels_selection() {
        let (overlay, completed) = create_recording_overlay();

        let hwnd = overlay.hwnd_for_test();
        send_message(hwnd, WM_LBUTTONDOWN, 0, make_lparam(10, 10));
        send_message(hwnd, WM_KEYDOWN, usize::from(VK_ESCAPE.0), LPARAM(0));

        assert!(!overlay.is_visible(), "キャンセル後は非表示であること");
        let result = *completed.lock().expect("lock");
        assert!(
            result.expect("on_complete が呼ばれる").is_none(),
            "キャンセルは None"
        );
    }
}
