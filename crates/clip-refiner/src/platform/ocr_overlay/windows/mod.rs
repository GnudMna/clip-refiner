//! Win32 レイヤードウィンドウによる画面範囲選択オーバーレイ

mod geometry;
mod layered;
mod types;
mod wndproc;

pub(crate) use types::OverlayCompleteFn;

use std::ptr;

use layered::LayeredOverlay;
use types::{DragSelection, OVERLAY_CLASS_NAME};
use wndproc::{OverlayState, hide_overlay, register_overlay_class};

use crate::platform::screen_capture::virtual_screen_bounds;

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GWLP_USERDATA, HWND_TOPMOST, IsWindowVisible, SW_SHOW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_LAYERED,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

// ======================================================================
// オーバーレイウィンドウ
// ======================================================================
/// Win32 レイヤードウィンドウによる画面範囲選択オーバーレイ
pub(crate) struct OverlayWindow {
    hwnd: HWND,
    state: Box<OverlayState>,
}

impl OverlayWindow {
    /// 仮想デスクトップ全体を覆うオーバーレイウィンドウを生成する
    pub fn create(on_complete: OverlayCompleteFn) -> Result<Self> {
        register_overlay_class()?;

        let (virt_x, virt_y, width, height) = virtual_screen_bounds();
        if width == 0 || height == 0 {
            bail!("仮想デスクトップのサイズが取得できない");
        }

        let class_name = geometry::str_to_wide(OVERLAY_CLASS_NAME);
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                ptr::null(),
                WS_POPUP,
                virt_x,
                virt_y,
                i32::try_from(width).context("幅の変換に失敗")?,
                i32::try_from(height).context("高さの変換に失敗")?,
                HWND::default(),
                HWND::default(),
                GetModuleHandleW(ptr::null()),
                ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            bail!("オーバーレイウィンドウの作成に失敗");
        }

        let layered = LayeredOverlay::new(hwnd, width, height)?;
        let mut state = Box::new(OverlayState {
            layered,
            selection: DragSelection::default(),
            virt_x,
            virt_y,
            on_complete,
            last_redraw_at: None,
        });

        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                ptr::from_mut(state.as_mut()).cast::<()>() as isize,
            );
        }

        Ok(Self { hwnd, state })
    }

    /// オーバーレイを表示する
    pub fn show(&mut self) {
        self.state.selection = DragSelection::default();
        self.state.last_redraw_at = None;
        if let Err(err) = self.state.layered.reset_to_dim() {
            crate::log_warn!("オーバーレイの初期描画に失敗: {err:#}");
        }
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                    | windows_sys::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            SetForegroundWindow(self.hwnd);
        }
    }

    /// オーバーレイを非表示にする
    pub fn hide(&mut self) {
        unsafe {
            hide_overlay(self.hwnd, self.state.as_mut());
        }
    }

    /// オーバーレイが表示中かどうか
    pub fn is_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            ReleaseCapture();
            DestroyWindow(self.hwnd);
        }
    }
}
