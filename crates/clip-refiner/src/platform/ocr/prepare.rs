use super::super::screen_capture::RgbaImage;

// ======================================================================
// 定数
// ======================================================================
/// OCR が安定しやすい最短辺の目安 (物理ピクセル)
const MIN_OCR_DIMENSION: u32 = 96;

/// 小画像の最大拡大倍率
const MAX_UPSCALE: u32 = 4;

// ======================================================================
// パブリック関数
// ======================================================================
/// OCR 向けに画像を拡大する
///
/// 短辺が小さい画像は認識率向上のため拡大する
pub(crate) fn prepare_ocr_image(image: &RgbaImage) -> RgbaImage {
    let min_dimension = image.width.min(image.height);
    if min_dimension >= MIN_OCR_DIMENSION {
        return image.clone();
    }

    let scale = MIN_OCR_DIMENSION
        .saturating_add(min_dimension.saturating_sub(1))
        .checked_div(min_dimension)
        .unwrap_or(1)
        .clamp(2, MAX_UPSCALE);
    upscale_nearest(image, scale)
}

/// 最近傍補間で RGBA 画像を拡大する
pub(crate) fn upscale_nearest(image: &RgbaImage, scale: u32) -> RgbaImage {
    if scale <= 1 {
        return image.clone();
    }

    let width = image.width.saturating_mul(scale);
    let height = image.height.saturating_mul(scale);
    let mut rgba = Vec::with_capacity(usize::try_from(width * height * 4).unwrap_or(0));

    for y in 0..height {
        let source_y = y / scale;
        let row_start = usize::try_from(source_y * image.width * 4).unwrap_or(0);
        let row_end = row_start.saturating_add(usize::try_from(image.width * 4).unwrap_or(0));
        let source_row = &image.rgba[row_start..row_end];

        for x in 0..width {
            let source_x = usize::try_from((x / scale) * 4).unwrap_or(0);
            rgba.extend_from_slice(&source_row[source_x..source_x.saturating_add(4)]);
        }
    }

    RgbaImage {
        width,
        height,
        rgba,
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 短辺が小さい画像を拡大する
    #[test]
    fn prepare_ocr_image_upscales_small_selection() {
        let image = RgbaImage {
            width: 40,
            height: 20,
            rgba: vec![255; 40 * 20 * 4],
        };
        let prepared = prepare_ocr_image(&image);
        assert_eq!(prepared.width, 160);
        assert_eq!(prepared.height, 80);
    }

    /// 十分なサイズの画像は拡大しない
    #[test]
    fn prepare_ocr_image_keeps_large_selection() {
        let image = RgbaImage {
            width: 200,
            height: 120,
            rgba: vec![255; 200 * 120 * 4],
        };
        let prepared = prepare_ocr_image(&image);
        assert_eq!(prepared.width, 200);
        assert_eq!(prepared.height, 120);
    }

    /// 最近傍拡大でピクセルが複製される
    #[test]
    fn upscale_nearest_duplicates_pixels() {
        let image = RgbaImage {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let upscaled = upscale_nearest(&image, 2);
        assert_eq!(upscaled.width, 4);
        assert_eq!(upscaled.height, 2);
        assert_eq!(
            upscaled.rgba,
            vec![
                1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8, //
                1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8, 5, 6, 7, 8,
            ]
        );
    }
}
