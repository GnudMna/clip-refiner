use std::sync::Once;
use std::time::Instant;

use anyhow::Result;
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WIN32_ERROR, WPARAM};
use windows::Win32::Graphics::Gdi::{HBRUSH, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, HICON, IDC_ARROW, LoadCursorW,
    RegisterClassW, SW_HIDE, SetWindowLongPtrW, ShowWindow, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WNDCLASS_STYLES, WNDCLASSW,
};
use windows::core::PCWSTR;

use super::geometry::{lparam_x, lparam_y, map_client_rect_to_screen, normalize_rect, str_to_wide};
use super::layered::LayeredOverlay;
use super::types::{
    DragSelection, ERROR_CLASS_ALREADY_EXISTS, MIN_SELECTION_SIZE, OVERLAY_CLASS_NAME,
    REDRAW_INTERVAL,
};

// ======================================================================
// ウィンドウ状態
// ======================================================================
/// ウィンドウに紐づく内部状態
pub(crate) struct OverlayState {
    pub layered: LayeredOverlay,
    pub selection: DragSelection,
    pub virt_x: i32,
    pub virt_y: i32,
    pub on_complete: super::types::OverlayCompleteFn,
    /// 直前の再描画時刻
    pub last_redraw_at: Option<Instant>,
}

static REGISTER_OVERLAY_CLASS: Once = Once::new();

// ======================================================================
// ウィンドウプロシージャ
// ======================================================================
/// オーバーレイ用ウィンドウクラスを登録する
pub(crate) fn register_overlay_class() -> Result<()> {
    let mut failed = false;
    REGISTER_OVERLAY_CLASS.call_once(|| {
        let class_name = str_to_wide(OVERLAY_CLASS_NAME);
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            // SAFETY: `None` は自プロセスのモジュールハンドルを返すだけで、ポインタを受け取らない
            hInstance: unsafe { GetModuleHandleW(None).map(Into::into).unwrap_or_default() },
            hIcon: HICON::default(),
            // SAFETY: システム定義カーソルの読み込みで、インスタンスハンドルを渡さない
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW).unwrap_or_default() },
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
        };
        // SAFETY: `wc` と `class_name` は呼び出し中に生存し、クラス名は NUL 終端の UTF-16
        if unsafe { RegisterClassW(&raw const wc) } == 0 {
            // SAFETY: 引数を取らず、同じスレッドの直前の失敗コードを読むだけ
            let err = unsafe { GetLastError() };
            if err != WIN32_ERROR(ERROR_CLASS_ALREADY_EXISTS) {
                failed = true;
            }
        }
    });
    if failed {
        anyhow::bail!("オーバーレイウィンドウクラスの登録に失敗");
    }
    Ok(())
}

/// オーバーレイのウィンドウプロシージャ
///
/// # Safety
/// OS からウィンドウメッセージとして呼ばれることを前提とする
/// `GWLP_USERDATA` は 0 か、生存中の `OverlayState` を指していること
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: `GWLP_USERDATA` は `OverlayWindow::create` が `Box<OverlayState>` のアドレスを設定し、
    // `OverlayWindow` の破棄と `WM_DESTROY` で 0 に戻す。0 以外のときは生存中の状態を指す
    // ウィンドウはそれを作成したスレッドでのみメッセージを受けるため、状態へ並行アクセスしない
    // `on_complete` はオーバーレイ自体を破棄しないため、呼び出し後に `state` がダングリングにならない
    unsafe {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
        if state.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        match msg {
            WM_ERASEBKGND => return LRESULT(1),
            WM_PAINT => {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &raw mut rect).is_ok() {
                    let _ = ValidateRect(Some(hwnd), Some(&raw const rect));
                }
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                let cursor = (lparam_x(lparam), lparam_y(lparam));
                let _ = SetCapture(hwnd);
                (*state).selection.start = Some(cursor);
                (*state).selection.cursor = Some(cursor);
                redraw_overlay(state);
                return LRESULT(0);
            }
            WM_MOUSEMOVE => {
                if (*state).selection.start.is_none() {
                    return LRESULT(0);
                }
                (*state).selection.cursor = Some((lparam_x(lparam), lparam_y(lparam)));
                let now = Instant::now();
                if let Some(last) = (*state).last_redraw_at
                    && now.duration_since(last) < REDRAW_INTERVAL
                {
                    return LRESULT(0);
                }
                (*state).last_redraw_at = Some(now);
                redraw_overlay(state);
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                if (*state).selection.start.is_none() {
                    return LRESULT(0);
                }
                (*state).selection.cursor = Some((lparam_x(lparam), lparam_y(lparam)));
                finish_selection(hwnd, state);
                return LRESULT(0);
            }
            WM_KEYDOWN => {
                if wparam.0 == usize::from(VK_ESCAPE.0) {
                    hide_overlay(hwnd, state);
                    ((*state).on_complete)(None);
                }
                return LRESULT(0);
            }
            WM_DESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                return LRESULT(0);
            }
            _ => {}
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

/// オーバーレイを再描画する
///
/// # Safety
/// `state` は生存中の `OverlayState` を指し、呼び出し中に他から参照されないこと
unsafe fn redraw_overlay(state: *mut OverlayState) {
    // SAFETY: 呼び出し元が `state` の有効性と排他性を保証する
    unsafe {
        let Some(rect) = (*state)
            .selection
            .start
            .zip((*state).selection.cursor)
            .map(|(start, end)| normalize_rect(start, end))
        else {
            return;
        };
        if let Err(err) = (*state).layered.update_selection(rect) {
            crate::log_warn!("オーバーレイの再描画に失敗: {err:#}");
        }
    }
}

/// 選択範囲を確定してオーバーレイを閉じる
///
/// # Safety
/// `state` は生存中の `OverlayState` を指し、呼び出し中に他から参照されないこと
/// `on_complete` はオーバーレイを破棄しないこと
unsafe fn finish_selection(hwnd: HWND, state: *mut OverlayState) {
    // SAFETY: 呼び出し元が `state` の有効性と排他性を保証する
    // `selection` の参照は `start` / `cursor` のコピー後に使わず、`hide_overlay` の書き込みと重ならない
    unsafe {
        let selection = &(*state).selection;
        let (Some(start), Some(end)) = (selection.start, selection.cursor) else {
            hide_overlay(hwnd, state);
            ((*state).on_complete)(None);
            return;
        };
        let rect = normalize_rect(start, end);
        if rect.width < MIN_SELECTION_SIZE || rect.height < MIN_SELECTION_SIZE {
            hide_overlay(hwnd, state);
            ((*state).on_complete)(None);
            return;
        }

        let screen_rect = map_client_rect_to_screen((*state).virt_x, (*state).virt_y, rect);
        hide_overlay(hwnd, state);
        ((*state).on_complete)(Some(screen_rect));
    }
}

/// オーバーレイを非表示にする
///
/// # Safety
/// `state` は生存中の `OverlayState` を指し、呼び出し中に他から参照されないこと
pub(crate) unsafe fn hide_overlay(hwnd: HWND, state: *mut OverlayState) {
    // SAFETY: 呼び出し元が `state` の有効性と排他性を保証する
    unsafe {
        (*state).selection = DragSelection::default();
        (*state).last_redraw_at = None;
        (*state).layered.last_preview = None;
        let _ = ReleaseCapture();
        let _ = ShowWindow(hwnd, SW_HIDE);
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &raw mut rect).is_ok() {
            let _ = ValidateRect(Some(hwnd), Some(&raw const rect));
        }
    }
}
