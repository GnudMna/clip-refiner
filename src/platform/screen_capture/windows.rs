use std::ptr;

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

// ======================================================================
// 画面矩形
// ======================================================================
/// 画面上の矩形領域 (物理ピクセル座標)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    /// 画面左上からの X 座標
    pub x: i32,
    /// 画面左上からの Y 座標
    pub y: i32,
    /// 幅
    pub width: u32,
    /// 高さ
    pub height: u32,
}

/// キャプチャした RGBA 画像
#[derive(Debug, Clone)]
pub(crate) struct RgbaImage {
    /// 画像幅 (ピクセル)
    pub width: u32,
    /// 画像高さ (ピクセル)
    pub height: u32,
    /// RGBA ピクセル列 (左上から行優先)
    pub rgba: Vec<u8>,
}

// ======================================================================
// パブリック関数
// ======================================================================
/// 仮想デスクトップ全体の境界 (物理ピクセル) を返す
pub(crate) fn virtual_screen_bounds() -> (i32, i32, u32, u32) {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = u32::try_from(GetSystemMetrics(SM_CXVIRTUALSCREEN)).unwrap_or(0);
        let height = u32::try_from(GetSystemMetrics(SM_CYVIRTUALSCREEN)).unwrap_or(0);
        (x, y, width, height)
    }
}

/// 指定矩形の画面領域を RGBA 画像として取得する
pub(crate) fn capture_screen_region(rect: ScreenRect) -> Result<RgbaImage> {
    if rect.width == 0 || rect.height == 0 {
        bail!("キャプチャ領域が空");
    }

    let width = i32::try_from(rect.width).context("キャプチャ幅が大きすぎる")?;
    let height = i32::try_from(rect.height).context("キャプチャ高さが大きすぎる")?;

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

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_null() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND::default(), hdc_screen);
            bail!("ビットマップの作成に失敗");
        }

        let old_bitmap = SelectObject(hdc_mem, hbitmap);
        let blt_ok = BitBlt(
            hdc_mem, 0, 0, width, height, hdc_screen, rect.x, rect.y, SRCCOPY,
        );
        SelectObject(hdc_mem, old_bitmap);

        if blt_ok == 0 {
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND::default(), hdc_screen);
            bail!("画面の BitBlt に失敗");
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(u32::MAX),
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [std::mem::zeroed(); 1],
        };

        let pixel_count = rect
            .width
            .checked_mul(rect.height)
            .context("ピクセル数の計算に失敗")?;
        let mut bgra = vec![0u8; pixel_count as usize * 4];

        let lines = GetDIBits(
            hdc_mem,
            hbitmap,
            0,
            rect.height,
            bgra.as_mut_ptr().cast(),
            ptr::from_mut(&mut bmi),
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(hbitmap);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(HWND::default(), hdc_screen);

        if lines == 0 {
            bail!("DIB データの取得に失敗");
        }

        let rgba = bgra
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
            .collect();

        Ok(RgbaImage {
            width: rect.width,
            height: rect.height,
            rgba,
        })
    }
}
