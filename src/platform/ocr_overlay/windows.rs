use std::ptr;
use std::sync::Once;
use std::time::{Duration, Instant};

use crate::platform::screen_capture::{ScreenRect, virtual_screen_bounds};

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, ReleaseDC,
    SelectObject, ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect,
    GetWindowLongPtrW, HWND_TOPMOST, IDC_ARROW, IsWindowVisible, LoadCursorW, RegisterClassW,
    SW_HIDE, SW_SHOW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

// ======================================================================
// オーバーレイ型
// ======================================================================
/// オーバーレイ上の選択矩形 (クライアント座標)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionRect {
    /// 左上 X
    pub x: i32,
    /// 左上 Y
    pub y: i32,
    /// 幅
    pub width: i32,
    /// 高さ
    pub height: i32,
}

/// ドラッグ選択の状態
#[derive(Debug, Clone, Copy, Default)]
struct DragSelection {
    /// ドラッグ開始点 (クライアント座標)
    start: Option<(i32, i32)>,
    /// 現在のカーソル位置 (クライアント座標)
    cursor: Option<(i32, i32)>,
}

/// 選択確定時に呼ばれるコールバック
pub(crate) type OverlayCompleteFn = Box<dyn Fn(ScreenRect)>;

/// `UpdateLayeredWindow` で描画する半透明オーバーレイ
struct LayeredOverlay {
    hwnd: HWND,
    width: u32,
    height: u32,
    bgra: Vec<u8>,
    /// 暗転行のテンプレート (`width * 4` バイト)
    dim_row: Vec<u8>,
    presenter: LayeredPresenter,
    /// 直前に描画した選択範囲
    last_preview: Option<SelectionRect>,
}

/// `UpdateLayeredWindow` 用 GDI リソース (再利用)
struct LayeredPresenter {
    hdc_screen: HDC,
    hdc_mem: HDC,
    hbmp: HBITMAP,
    bits: *mut core::ffi::c_void,
    saved_bitmap: HBITMAP,
}

/// Win32 レイヤードウィンドウによる画面範囲選択オーバーレイ
pub(crate) struct OverlayWindow {
    hwnd: HWND,
    state: Box<OverlayState>,
}

/// ウィンドウに紐づく内部状態
struct OverlayState {
    layered: LayeredOverlay,
    selection: DragSelection,
    virt_x: i32,
    virt_y: i32,
    on_complete: OverlayCompleteFn,
    /// 直前の再描画時刻
    last_redraw_at: Option<Instant>,
}

// ======================================================================
// 定数
// ======================================================================
/// 画面全体の暗転アルファ (約 35%)
const DIM_ALPHA: u8 = 89;

/// 選択枠の線幅
const BORDER_WIDTH: i32 = 2;

/// 選択確定に必要な最小辺長
const MIN_SELECTION_SIZE: i32 = 5;

/// ドラッグ中の再描画間隔
const REDRAW_INTERVAL: Duration = Duration::from_millis(16);

/// ウィンドウクラス名
const OVERLAY_CLASS_NAME: &str = "ClipRefinerOcrOverlay";

/// `RegisterClassW` が返す「クラスは既に存在する」エラー
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;

static REGISTER_OVERLAY_CLASS: Once = Once::new();

// ======================================================================
// パブリック関数
// ======================================================================
impl OverlayWindow {
    /// 仮想デスクトップ全体を覆うオーバーレイウィンドウを生成する
    pub fn create(on_complete: OverlayCompleteFn) -> Result<Self> {
        register_overlay_class()?;

        let (virt_x, virt_y, width, height) = virtual_screen_bounds();
        if width == 0 || height == 0 {
            bail!("仮想デスクトップのサイズが取得できない");
        }

        let class_name = str_to_wide(OVERLAY_CLASS_NAME);
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
        self.state.selection = DragSelection::default();
        self.state.last_redraw_at = None;
        self.state.layered.last_preview = None;
        unsafe {
            ReleaseCapture();
            ShowWindow(self.hwnd, SW_HIDE);
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
            DestroyWindow(self.hwnd);
        }
    }
}

// ======================================================================
// レイヤード描画
// ======================================================================
impl LayeredOverlay {
    /// 対象ウィンドウへ半透明レイヤーを割り当てる
    fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            bail!("オーバーレイ領域が空");
        }

        let pixel_count = usize::try_from(width)
            .context("幅の変換に失敗")?
            .saturating_mul(usize::try_from(height).context("高さの変換に失敗")?);
        let bgra = vec![0; pixel_count.saturating_mul(4)];
        let dim_row = build_dim_row(width);
        let presenter = LayeredPresenter::new(width, height)?;

        let mut overlay = Self {
            hwnd,
            width,
            height,
            bgra,
            dim_row,
            presenter,
            last_preview: None,
        };
        overlay.reset_to_dim()?;
        Ok(overlay)
    }

    /// 画面全体を暗転状態へ戻す
    fn reset_to_dim(&mut self) -> Result<()> {
        fill_dim_layer(&mut self.bgra, &self.dim_row, self.height);
        self.last_preview = None;
        self.present(self.hwnd)
    }

    /// 選択範囲だけ差分更新する
    fn update_selection(&mut self, selection: SelectionRect) -> Result<()> {
        if let Some(old) = self.last_preview.replace(selection) {
            restore_dim_rect(
                &mut self.bgra,
                self.width,
                self.height,
                &self.dim_row,
                expand_rect(old, BORDER_WIDTH),
            );
        }
        clear_rect(&mut self.bgra, self.width, self.height, selection);
        draw_border(&mut self.bgra, self.width, self.height, selection);
        self.present(self.hwnd)
    }

    /// バッファを `UpdateLayeredWindow` へ反映する
    fn present(&mut self, hwnd: HWND) -> Result<()> {
        self.presenter
            .present(hwnd, self.width, self.height, &self.bgra)
    }
}

impl Drop for LayeredPresenter {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc_mem, self.saved_bitmap);
            DeleteObject(self.hbmp);
            DeleteDC(self.hdc_mem);
            ReleaseDC(HWND::default(), self.hdc_screen);
        }
    }
}

impl LayeredPresenter {
    /// 再利用可能な DIB セクションを確保する
    fn new(width: u32, height: u32) -> Result<Self> {
        unsafe {
            let hdc_screen = GetDC(HWND::default());
            if hdc_screen.is_null() {
                bail!("画面 DC の取得に失敗");
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_null() {
                ReleaseDC(HWND::default(), hdc_screen);
                bail!("互換 DC の作成に失敗");
            }

            let bmi = bitmap_info(width, height)?;
            let mut bits: *mut core::ffi::c_void = ptr::null_mut();
            let hbmp = CreateDIBSection(
                hdc_mem,
                ptr::from_ref(&bmi),
                DIB_RGB_COLORS,
                ptr::from_mut(&mut bits),
                ptr::null_mut(),
                0,
            );
            if hbmp.is_null() || bits.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(HWND::default(), hdc_screen);
                bail!("DIB セクションの作成に失敗");
            }

            let saved_bitmap = SelectObject(hdc_mem, hbmp);
            Ok(Self {
                hdc_screen,
                hdc_mem,
                hbmp,
                bits,
                saved_bitmap,
            })
        }
    }

    /// ビットマップをウィンドウへ反映する
    fn present(&mut self, hwnd: HWND, width: u32, height: u32, bgra: &[u8]) -> Result<()> {
        unsafe {
            ptr::copy_nonoverlapping(bgra.as_ptr(), self.bits.cast(), bgra.len());

            let size = windows_sys::Win32::Foundation::SIZE {
                cx: i32::try_from(width).context("幅の変換に失敗")?,
                cy: i32::try_from(height).context("高さの変換に失敗")?,
            };
            let point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: 0,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: u8::try_from(AC_SRC_ALPHA).unwrap_or(1),
            };

            let ok = UpdateLayeredWindow(
                hwnd,
                self.hdc_screen,
                ptr::null(),
                ptr::from_ref(&size),
                self.hdc_mem,
                ptr::from_ref(&point),
                0,
                ptr::from_ref(&blend),
                ULW_ALPHA,
            );
            if ok == 0 {
                bail!("レイヤードウィンドウの更新に失敗");
            }
        }

        Ok(())
    }
}

// ======================================================================
// ウィンドウプロシージャ
// ======================================================================
/// オーバーレイ用ウィンドウクラスを登録する
fn register_overlay_class() -> Result<()> {
    let mut failed = false;
    REGISTER_OVERLAY_CLASS.call_once(|| {
        let class_name = str_to_wide(OVERLAY_CLASS_NAME);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: unsafe { GetModuleHandleW(ptr::null()) },
            hIcon: ptr::null_mut(),
            hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        if unsafe { RegisterClassW(ptr::from_ref(&wc)) } == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_CLASS_ALREADY_EXISTS {
                failed = true;
            }
        }
    });
    if failed {
        bail!("オーバーレイウィンドウクラスの登録に失敗");
    }
    Ok(())
}

/// オーバーレイのウィンドウプロシージャ
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
        if state.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        match msg {
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                let mut rect = std::mem::zeroed();
                if GetClientRect(hwnd, ptr::from_mut(&mut rect)) != 0 {
                    let _ = ValidateRect(hwnd, ptr::from_ref(&rect));
                }
                return 0;
            }
            WM_LBUTTONDOWN => {
                let cursor = (lparam_x(lparam), lparam_y(lparam));
                SetCapture(hwnd);
                (*state).selection.start = Some(cursor);
                (*state).selection.cursor = Some(cursor);
                redraw_overlay(state);
                return 0;
            }
            WM_MOUSEMOVE => {
                if (*state).selection.start.is_none() {
                    return 0;
                }
                (*state).selection.cursor = Some((lparam_x(lparam), lparam_y(lparam)));
                let now = Instant::now();
                if let Some(last) = (*state).last_redraw_at
                    && now.duration_since(last) < REDRAW_INTERVAL
                {
                    return 0;
                }
                (*state).last_redraw_at = Some(now);
                redraw_overlay(state);
                return 0;
            }
            WM_LBUTTONUP => {
                ReleaseCapture();
                if (*state).selection.start.is_none() {
                    return 0;
                }
                (*state).selection.cursor = Some((lparam_x(lparam), lparam_y(lparam)));
                finish_selection(hwnd, state);
                return 0;
            }
            WM_KEYDOWN => {
                if wparam == usize::from(VK_ESCAPE) {
                    hide_overlay(hwnd, state);
                }
                return 0;
            }
            WM_DESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                return 0;
            }
            _ => {}
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

/// オーバーレイを再描画する
unsafe fn redraw_overlay(state: *mut OverlayState) {
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
unsafe fn finish_selection(hwnd: HWND, state: *mut OverlayState) {
    unsafe {
        let selection = &(*state).selection;
        let (Some(start), Some(end)) = (selection.start, selection.cursor) else {
            hide_overlay(hwnd, state);
            return;
        };
        let rect = normalize_rect(start, end);
        if rect.width < MIN_SELECTION_SIZE || rect.height < MIN_SELECTION_SIZE {
            hide_overlay(hwnd, state);
            return;
        }

        let screen_rect = map_client_rect_to_screen((*state).virt_x, (*state).virt_y, rect);
        hide_overlay(hwnd, state);
        ((*state).on_complete)(screen_rect);
    }
}

/// オーバーレイを非表示にする
unsafe fn hide_overlay(hwnd: HWND, state: *mut OverlayState) {
    unsafe {
        (*state).selection = DragSelection::default();
        (*state).last_redraw_at = None;
        (*state).layered.last_preview = None;
        ReleaseCapture();
        ShowWindow(hwnd, SW_HIDE);
        let mut rect = std::mem::zeroed();
        if GetClientRect(hwnd, ptr::from_mut(&mut rect)) != 0 {
            let _ = ValidateRect(hwnd, ptr::from_ref(&rect));
        }
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// UTF-8 文字列を null 終端付き UTF-16 へ変換する
fn str_to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `LPARAM` からクライアント X 座標を取り出す
fn lparam_x(lparam: LPARAM) -> i32 {
    i32::from(u16::try_from(lparam & 0xFFFF).unwrap_or(0))
}

/// `LPARAM` からクライアント Y 座標を取り出す
fn lparam_y(lparam: LPARAM) -> i32 {
    let high = u32::try_from(lparam).unwrap_or(0) >> 16;
    i32::from(u16::try_from(high).unwrap_or(0))
}

/// 2点から正規化した選択矩形を作る
fn normalize_rect(start: (i32, i32), end: (i32, i32)) -> SelectionRect {
    let x1 = start.0.min(end.0);
    let y1 = start.1.min(end.1);
    let x2 = start.0.max(end.0);
    let y2 = start.1.max(end.1);
    SelectionRect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1).max(1),
        height: y2.saturating_sub(y1).max(1),
    }
}

/// クライアント座標の矩形を仮想デスクトップ座標へ変換する
fn map_client_rect_to_screen(virt_x: i32, virt_y: i32, rect: SelectionRect) -> ScreenRect {
    ScreenRect {
        x: virt_x.saturating_add(rect.x),
        y: virt_y.saturating_add(rect.y),
        width: u32::try_from(rect.width.max(1)).unwrap_or(1),
        height: u32::try_from(rect.height.max(1)).unwrap_or(1),
    }
}

/// 暗転行テンプレートを生成する
fn build_dim_row(width: u32) -> Vec<u8> {
    let row_bytes = usize::try_from(width).unwrap_or(0).saturating_mul(4);
    let mut dim_row = Vec::with_capacity(row_bytes);
    for _ in 0..width {
        dim_row.extend_from_slice(&[0, 0, 0, DIM_ALPHA]);
    }
    dim_row
}

/// `BITMAPINFO` を組み立てる
fn bitmap_info(width: u32, height: u32) -> Result<BITMAPINFO> {
    Ok(BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(u32::MAX),
            biWidth: i32::try_from(width).context("幅の変換に失敗")?,
            biHeight: -i32::try_from(height).context("高さの変換に失敗")?,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }; 1],
    })
}

/// 矩形を外側へ拡張する
fn expand_rect(rect: SelectionRect, margin: i32) -> SelectionRect {
    SelectionRect {
        x: rect.x.saturating_sub(margin),
        y: rect.y.saturating_sub(margin),
        width: rect.width.saturating_add(margin.saturating_mul(2)),
        height: rect.height.saturating_add(margin.saturating_mul(2)),
    }
}

/// 画面全体を半透明の黒で塗りつぶす
fn fill_dim_layer(bgra: &mut [u8], dim_row: &[u8], height: u32) {
    let row_bytes = dim_row.len();
    for y in 0..usize::try_from(height).unwrap_or(0) {
        let start = y.saturating_mul(row_bytes);
        let end = start.saturating_add(row_bytes);
        if end <= bgra.len() {
            bgra[start..end].copy_from_slice(dim_row);
        }
    }
}

/// 指定範囲を暗転状態へ戻す
fn restore_dim_rect(
    bgra: &mut [u8],
    frame_width: u32,
    frame_height: u32,
    dim_row: &[u8],
    rect: SelectionRect,
) {
    let Some(clamped) = clamp_rect(rect, frame_width, frame_height) else {
        return;
    };

    let row_bytes = usize::try_from(frame_width).unwrap_or(0).saturating_mul(4);
    for y in clamped.y..clamped.y.saturating_add(clamped.height) {
        let row_start = usize::try_from(y).unwrap_or(0).saturating_mul(row_bytes);
        let x_start = usize::try_from(clamped.x).unwrap_or(0).saturating_mul(4);
        let patch_bytes = usize::try_from(clamped.width)
            .unwrap_or(0)
            .saturating_mul(4);
        let start = row_start.saturating_add(x_start);
        let end = start.saturating_add(patch_bytes);
        let src_start = x_start;
        let src_end = src_start.saturating_add(patch_bytes);
        if end <= bgra.len() && src_end <= dim_row.len() {
            bgra[start..end].copy_from_slice(&dim_row[src_start..src_end]);
        }
    }
}

/// 選択範囲内を完全透明にする
fn clear_rect(bgra: &mut [u8], frame_width: u32, frame_height: u32, rect: SelectionRect) {
    let Some(clamped) = clamp_rect(rect, frame_width, frame_height) else {
        return;
    };

    for y in clamped.y..clamped.y.saturating_add(clamped.height) {
        let row_start = usize::try_from(y)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(frame_width).unwrap_or(0))
            .saturating_mul(4);
        let x_start = usize::try_from(clamped.x).unwrap_or(0).saturating_mul(4);
        let row_bytes = usize::try_from(clamped.width)
            .unwrap_or(0)
            .saturating_mul(4);
        let start = row_start.saturating_add(x_start);
        let end = start.saturating_add(row_bytes);
        if end <= bgra.len() {
            bgra[start..end].fill(0);
        }
    }
}

/// 選択枠の BGRA 色
const BORDER_PIXEL: [u8; 4] = [0xFF, 0xA8, 0x5A, 0xFF];

/// 選択枠を描画する
fn draw_border(bgra: &mut [u8], frame_width: u32, frame_height: u32, rect: SelectionRect) {
    let Some(clamped) = clamp_rect(rect, frame_width, frame_height) else {
        return;
    };

    let left = clamped.x;
    let top = clamped.y;
    let right = clamped.x.saturating_add(clamped.width).saturating_sub(1);
    let bottom = clamped.y.saturating_add(clamped.height).saturating_sub(1);

    for offset in 0..BORDER_WIDTH {
        let y_top = top.saturating_add(offset);
        let y_bottom = bottom.saturating_sub(offset);
        for x in left..=right {
            set_pixel(bgra, frame_width, x, y_top, BORDER_PIXEL);
            set_pixel(bgra, frame_width, x, y_bottom, BORDER_PIXEL);
        }

        let x_left = left.saturating_add(offset);
        let x_right = right.saturating_sub(offset);
        for y in top..=bottom {
            set_pixel(bgra, frame_width, x_left, y, BORDER_PIXEL);
            set_pixel(bgra, frame_width, x_right, y, BORDER_PIXEL);
        }
    }
}

/// 矩形をフレーム内に収める
fn clamp_rect(rect: SelectionRect, frame_width: u32, frame_height: u32) -> Option<SelectionRect> {
    let width_i = i32::try_from(frame_width).ok()?;
    let height_i = i32::try_from(frame_height).ok()?;
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    let x1 = rect.x.clamp(0, width_i.saturating_sub(1));
    let y1 = rect.y.clamp(0, height_i.saturating_sub(1));
    let x2 = rect
        .x
        .saturating_add(rect.width)
        .clamp(0, width_i)
        .max(x1.saturating_add(1));
    let y2 = rect
        .y
        .saturating_add(rect.height)
        .clamp(0, height_i)
        .max(y1.saturating_add(1));

    Some(SelectionRect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    })
}

/// BGRA ピクセルを書き込む
fn set_pixel(bgra: &mut [u8], frame_width: u32, x: i32, y: i32, pixel: [u8; 4]) {
    if x < 0 || y < 0 {
        return;
    }
    let width = usize::try_from(frame_width).unwrap_or(0);
    let xu = usize::try_from(x).unwrap_or(0);
    let yu = usize::try_from(y).unwrap_or(0);
    let offset = yu
        .saturating_mul(width)
        .saturating_add(xu)
        .saturating_mul(4);
    if offset.saturating_add(3) < bgra.len() {
        bgra[offset..offset.saturating_add(4)].copy_from_slice(&pixel);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 選択範囲内のピクセルが透明になる
    #[test]
    fn clear_rect_makes_selection_transparent() {
        let dim_row = build_dim_row(4);
        let mut bgra = vec![0xFF; 4 * 4 * 4];
        fill_dim_layer(&mut bgra, &dim_row, 4);
        clear_rect(
            &mut bgra,
            4,
            4,
            SelectionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        );
        assert_eq!(bgra[4 * 4 + 4 + 3], 0);
        assert_eq!(bgra[3], DIM_ALPHA);
    }

    /// 差分更新で選択範囲を戻せる
    #[test]
    fn restore_dim_rect_refills_selection_area() {
        let dim_row = build_dim_row(4);
        let mut bgra = vec![0xFF; 4 * 4 * 4];
        fill_dim_layer(&mut bgra, &dim_row, 4);
        clear_rect(
            &mut bgra,
            4,
            4,
            SelectionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        );
        restore_dim_rect(
            &mut bgra,
            4,
            4,
            &dim_row,
            SelectionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        );
        assert_eq!(bgra[4 * 4 + 4 + 3], DIM_ALPHA);
    }

    /// 2点から正規化矩形を作る
    #[test]
    fn normalize_rect_orders_corners() {
        let rect = normalize_rect((30, 40), (10, 20));
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 20);
        assert_eq!(rect.height, 20);
    }

    /// クライアント座標から仮想デスクトップ座標への変換を検証する
    #[test]
    fn map_client_rect_to_screen_adds_virtual_origin() {
        let rect = map_client_rect_to_screen(
            -100,
            50,
            SelectionRect {
                x: 100,
                y: 200,
                width: 300,
                height: 400,
            },
        );
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 250);
        assert_eq!(rect.width, 300);
        assert_eq!(rect.height, 400);
    }
}
