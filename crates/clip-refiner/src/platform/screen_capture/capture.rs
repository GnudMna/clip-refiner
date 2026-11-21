use super::types::{RgbaImage, ScreenRect};

use anyhow::{Context, Result, bail};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

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
    validate_capture_rect(rect)?;

    let width = i32::try_from(rect.width).context("キャプチャ幅が大きすぎる")?;
    let height = i32::try_from(rect.height).context("キャプチャ高さが大きすぎる")?;

    unsafe {
        let hdc_screen = GetDC(Some(HWND::default()));
        if hdc_screen.is_invalid() {
            bail!("画面 DC の取得に失敗");
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            let _ = ReleaseDC(Some(HWND::default()), hdc_screen);
            bail!("互換 DC の作成に失敗");
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(Some(HWND::default()), hdc_screen);
            bail!("ビットマップの作成に失敗");
        }

        let old_bitmap = SelectObject(hdc_mem, hbitmap.into());
        let blt_ok = BitBlt(
            hdc_mem,
            0,
            0,
            width,
            height,
            Some(hdc_screen),
            rect.x,
            rect.y,
            SRCCOPY,
        );
        SelectObject(hdc_mem, old_bitmap);

        if blt_ok.is_err() {
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(Some(HWND::default()), hdc_screen);
            bail!("画面の BitBlt に失敗");
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(u32::MAX),
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
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
            Some(bgra.as_mut_ptr().cast()),
            &raw mut bmi,
            DIB_RGB_COLORS,
        );
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(Some(HWND::default()), hdc_screen);

        if lines == 0 {
            bail!("DIB データの取得に失敗");
        }

        Ok(RgbaImage {
            width: rect.width,
            height: rect.height,
            rgba: bgra_to_rgba(&bgra),
        })
    }
}

// ======================================================================
// プライベート関数
// ======================================================================
/// キャプチャ矩形が空でないことを検証する
fn validate_capture_rect(rect: ScreenRect) -> Result<()> {
    if rect.width == 0 || rect.height == 0 {
        bail!("キャプチャ領域が空");
    }
    Ok(())
}

/// GDI の BGRA ピクセル列を RGBA へ変換する
fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    bgra.chunks_exact(4)
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
        .collect()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 幅または高さが 0 の矩形は拒否すること
    #[test]
    fn validate_capture_rect_rejects_empty_region() {
        assert!(
            validate_capture_rect(ScreenRect {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            })
            .is_err()
        );
        assert!(
            validate_capture_rect(ScreenRect {
                x: 0,
                y: 0,
                width: 10,
                height: 0,
            })
            .is_err()
        );
    }

    /// 有効な矩形は検証を通るようにすること
    #[test]
    fn validate_capture_rect_accepts_non_empty_region() {
        assert!(
            validate_capture_rect(ScreenRect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            })
            .is_ok()
        );
    }

    /// BGRA から RGBA へチャンネルを入れ替えるようにすること
    #[test]
    fn bgra_to_rgba_swaps_red_and_blue() {
        let bgra = [10, 20, 30, 40, 50, 60, 70, 80];
        assert_eq!(bgra_to_rgba(&bgra), vec![30, 20, 10, 40, 70, 60, 50, 80]);
    }

    /// 公開 API も空領域を拒否すること
    #[test]
    fn capture_screen_region_rejects_empty_rect() {
        let err = capture_screen_region(ScreenRect {
            x: 0,
            y: 0,
            width: 0,
            height: 1,
        })
        .expect_err("空領域は失敗する");
        assert!(err.to_string().contains("空"));
    }

    /// 仮想デスクトップ境界が正のサイズを返すようにすること
    #[test]
    fn virtual_screen_bounds_returns_positive_size() {
        let (_x, _y, width, height) = virtual_screen_bounds();
        assert!(width > 0, "仮想画面幅が 0");
        assert!(height > 0, "仮想画面高さが 0");
    }

    /// 仮想画面左上の小領域をキャプチャできるようにすること
    #[test]
    fn capture_screen_region_captures_tiny_virtual_origin() {
        let (x, y, width, height) = virtual_screen_bounds();
        assert!(width > 0 && height > 0);

        let rect = ScreenRect {
            x,
            y,
            width: 2.min(width),
            height: 2.min(height),
        };
        let image = capture_screen_region(rect).expect("小領域キャプチャに成功する");
        assert_eq!(image.width, rect.width);
        assert_eq!(image.height, rect.height);
        assert_eq!(
            image.rgba.len(),
            usize::try_from(rect.width * rect.height * 4).expect("サイズ変換")
        );
    }
}
