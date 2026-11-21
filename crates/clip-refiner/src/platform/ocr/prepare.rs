use super::super::screen_capture::RgbaImage;

// ======================================================================
// 定数
// ======================================================================
/// OCR が安定しやすい最短辺の目安 (物理ピクセル)
const MIN_OCR_DIMENSION: u32 = 96;

/// 小画像の最大拡大倍率
const MAX_UPSCALE: u32 = 4;

/// `OcrEngine::MaxImageDimension` に相当する、OCR に渡せる画像の一辺の上限 (物理ピクセル)
const MAX_OCR_IMAGE_DIMENSION: u32 = 10_000;

// ======================================================================
// パブリック関数
// ======================================================================
/// OCR 向けに画像を拡大する
///
/// 短辺が小さい画像は認識率向上のため拡大する。長辺が `MAX_OCR_IMAGE_DIMENSION` に
/// 収まるよう倍率を抑え、収まらない場合は拡大しない
pub(crate) fn prepare_ocr_image(image: &RgbaImage) -> RgbaImage {
    let min_dimension = image.width.min(image.height);
    if min_dimension >= MIN_OCR_DIMENSION {
        return image.clone();
    }

    let max_dimension = image.width.max(image.height);
    let scale = MIN_OCR_DIMENSION
        .saturating_add(min_dimension.saturating_sub(1))
        .checked_div(min_dimension)
        .unwrap_or(1)
        .clamp(2, MAX_UPSCALE)
        .min(
            MAX_OCR_IMAGE_DIMENSION
                .checked_div(max_dimension)
                .unwrap_or(1),
        );
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

    /// 短辺が小さい画像を拡大すること
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

    /// 十分なサイズの画像は拡大しないこと
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

    /// 最近傍拡大でピクセルを複製すること
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

    /// 倍率が 1 以下なら元画像を返すようにすること
    #[test]
    fn upscale_nearest_keeps_image_when_scale_is_one() {
        let image = RgbaImage {
            width: 1,
            height: 1,
            rgba: vec![9, 8, 7, 6],
        };
        let same = upscale_nearest(&image, 1);
        assert_eq!(same.width, 1);
        assert_eq!(same.height, 1);
        assert_eq!(same.rgba, image.rgba);
    }

    /// 極小画像の拡大倍率を上限でクランプすること
    #[test]
    fn prepare_ocr_image_clamps_upscale_to_max() {
        let image = RgbaImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        };
        let prepared = prepare_ocr_image(&image);
        assert_eq!(prepared.width, MAX_UPSCALE);
        assert_eq!(prepared.height, MAX_UPSCALE);
    }

    /// 横長で背の低い画像は長辺が上限に収まるよう倍率を抑えること
    #[test]
    fn prepare_ocr_image_limits_scale_by_long_side() {
        // 4 倍だと 12000px になるため 3 倍 (9000px) に抑える
        let image = RgbaImage {
            width: 3000,
            height: 24,
            rgba: vec![255; 3000 * 24 * 4],
        };
        let prepared = prepare_ocr_image(&image);
        assert_eq!(prepared.width, 9000);
        assert_eq!(prepared.height, 72);
        assert!(prepared.width <= MAX_OCR_IMAGE_DIMENSION);
    }

    /// 長辺が拡大すると上限を超える画像は拡大しないこと
    #[test]
    fn prepare_ocr_image_skips_upscale_when_long_side_too_large() {
        let image = RgbaImage {
            width: 6000,
            height: 20,
            rgba: vec![255; 6000 * 20 * 4],
        };
        let prepared = prepare_ocr_image(&image);
        assert_eq!(prepared.width, 6000);
        assert_eq!(prepared.height, 20);
    }
}
