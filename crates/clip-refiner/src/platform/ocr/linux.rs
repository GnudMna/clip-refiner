use super::super::screen_capture::RgbaImage;
use super::normalize::normalize_ocr_text;
use super::prepare::prepare_ocr_image;

use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageBuffer, Rgba};
use tesseract::Tesseract;

// ======================================================================
// パブリック関数
// ======================================================================
/// RGBA 画像からテキストを認識する (Tesseract)
pub(crate) fn recognize_text(image: &RgbaImage) -> Result<String> {
    if image.width == 0 || image.height == 0 {
        bail!("OCR 対象画像が空");
    }

    let prepared = prepare_ocr_image(image);
    let gray = rgba_to_luma8(&prepared)?;
    let width = i32::try_from(prepared.width).context("画像幅が大きすぎる")?;
    let height = i32::try_from(prepared.height).context("画像高さが大きすぎる")?;
    let bytes_per_line = i32::try_from(prepared.width).context("行幅の変換に失敗")?;

    let mut tess = Tesseract::new(None, Some("jpn+eng"))
        .context("Tesseract の初期化に失敗 (tesseract-ocr と jpn 言語パックが必要)")?;
    tess.set_image(gray.as_raw(), width, height, 1, bytes_per_line)
        .context("Tesseract への画像設定に失敗")?;

    let text = tess.get_text().context("Tesseract OCR の実行に失敗")?;

    Ok(normalize_ocr_text(text.trim()))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// RGBA 画像をグレースケールへ変換する
fn rgba_to_luma8(image: &RgbaImage) -> Result<image::GrayImage> {
    let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(image.width, image.height, image.rgba.clone())
            .context("RGBA バッファの構築に失敗")?;
    Ok(DynamicImage::ImageRgba8(buffer).to_luma8())
}
