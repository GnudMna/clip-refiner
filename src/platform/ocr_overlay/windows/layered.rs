use std::ptr;

use super::geometry::{clamp_rect, expand_rect};
use super::types::{BORDER_PIXEL, BORDER_WIDTH, DIM_ALPHA, SelectionRect};

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow};

// ======================================================================
// レイヤード描画
// ======================================================================
/// `UpdateLayeredWindow` で描画する半透明オーバーレイ
pub(crate) struct LayeredOverlay {
    hwnd: HWND,
    width: u32,
    height: u32,
    bgra: Vec<u8>,
    /// 暗転行のテンプレート (`width * 4` バイト)
    dim_row: Vec<u8>,
    presenter: LayeredPresenter,
    /// 直前に描画した選択範囲
    pub last_preview: Option<SelectionRect>,
}

/// `UpdateLayeredWindow` 用 GDI リソース (再利用)
struct LayeredPresenter {
    hdc_screen: HDC,
    hdc_mem: HDC,
    hbmp: HBITMAP,
    bits: *mut core::ffi::c_void,
    saved_bitmap: HBITMAP,
}

impl LayeredOverlay {
    /// 対象ウィンドウへ半透明レイヤーを割り当てる
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
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
    pub fn reset_to_dim(&mut self) -> Result<()> {
        fill_dim_layer(&mut self.bgra, &self.dim_row, self.height);
        self.last_preview = None;
        self.present(self.hwnd)
    }

    /// 選択範囲だけ差分更新する
    pub fn update_selection(&mut self, selection: SelectionRect) -> Result<()> {
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
// ピクセル操作
// ======================================================================
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
}
