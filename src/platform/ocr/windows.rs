use std::ptr;
use std::slice;

use super::super::screen_capture::RgbaImage;

use anyhow::{Context, Result, bail};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::System::WinRT::IMemoryBufferByteAccess;
use windows::core::{HSTRING, Interface};

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
/// RGBA 画像からテキストを認識する (`Windows.Media.Ocr`)
pub(crate) fn recognize_text(image: &RgbaImage) -> Result<String> {
    if image.width == 0 || image.height == 0 {
        bail!("OCR 対象画像が空");
    }

    let prepared = prepare_ocr_image(image);
    let bitmap = create_ocr_bitmap(&prepared)?;

    let engine = create_ocr_engine()
        .context("OCR エンジンの初期化に失敗 (日本語の言語パックが未インストールの可能性)")?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .context("OCR の開始に失敗")?
        .join()
        .context("OCR の完了待ちに失敗")?;

    Ok(normalize_ocr_text(
        &result
            .Text()
            .context("OCR 結果テキストの取得に失敗")?
            .to_string(),
    ))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 日本語優先で OCR エンジンを生成する
fn create_ocr_engine() -> Result<OcrEngine> {
    for tag in ["ja-JP", "ja"] {
        let language = Language::CreateLanguage(&HSTRING::from(tag))
            .with_context(|| format!("言語タグ `{tag}` の作成に失敗"))?;
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
            return Ok(engine);
        }
    }

    OcrEngine::TryCreateFromUserProfileLanguages()
        .context("ユーザー言語プロファイルから OCR エンジンを作成できない")
}

/// OCR 向けに画像を拡大する
///
/// `Windows.Media.Ocr` は短辺が小さい画像で認識率が落ちやすい
fn prepare_ocr_image(image: &RgbaImage) -> RgbaImage {
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
fn upscale_nearest(image: &RgbaImage, scale: u32) -> RgbaImage {
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

/// `Windows.Media.Ocr` 向けの `Bgra8` ビットマップを生成する
fn create_ocr_bitmap(image: &RgbaImage) -> Result<SoftwareBitmap> {
    let width = i32::try_from(image.width).context("画像幅が大きすぎる")?;
    let height = i32::try_from(image.height).context("画像高さが大きすぎる")?;

    let bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Bgra8, width, height)
        .context("SoftwareBitmap の作成に失敗")?;
    write_rgba_as_bgra(&bitmap, &image.rgba)?;
    Ok(bitmap)
}

/// `SoftwareBitmap` (`Bgra8`) へ RGBA データを書き込む
fn write_rgba_as_bgra(bitmap: &SoftwareBitmap, rgba: &[u8]) -> Result<()> {
    let buffer = bitmap
        .LockBuffer(BitmapBufferAccessMode::Write)
        .context("ビットマップバッファのロックに失敗")?;
    let reference = buffer
        .CreateReference()
        .context("メモリバッファ参照の作成に失敗")?;
    let access: IMemoryBufferByteAccess = reference
        .cast()
        .context("IMemoryBufferByteAccess への変換に失敗")?;

    let mut data = ptr::null_mut();
    let mut capacity = 0u32;
    unsafe {
        access
            .GetBuffer(ptr::from_mut(&mut data), ptr::from_mut(&mut capacity))
            .context("ビットマップバッファの取得に失敗")?;
    }

    let capacity = usize::try_from(capacity).context("バッファ容量の変換に失敗")?;
    if capacity < rgba.len() {
        bail!("ビットマップバッファの容量が不足");
    }

    let slice = unsafe { slice::from_raw_parts_mut(data, capacity) };
    for (dest, src) in slice.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dest[0] = src[2];
        dest[1] = src[1];
        dest[2] = src[0];
        dest[3] = src[3];
    }
    Ok(())
}

/// `Windows.Media.Ocr` が日本語文字の間に挿入するスペースを除去する
fn normalize_ocr_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut normalized = String::with_capacity(text.len());

    for (index, ch) in chars.iter().enumerate() {
        if is_collapsible_ocr_space(*ch) {
            let prev = normalized.chars().last();
            let next = chars.get(index + 1).copied();
            if should_collapse_ocr_space(prev, next) {
                continue;
            }
        }
        normalized.push(*ch);
    }

    normalized
}

/// OCR 結果で除去候補となる空白文字かどうか
fn is_collapsible_ocr_space(ch: char) -> bool {
    ch == ' ' || ch == '\u{3000}'
}

/// 前後が日本語系文字ならスペースを詰める
fn should_collapse_ocr_space(prev: Option<char>, next: Option<char>) -> bool {
    matches!(
        (prev, next),
        (Some(left), Some(right)) if is_japanese_compact_char(left) && is_japanese_compact_char(right)
    )
}

/// スペースを詰めてよい日本語系文字かどうか
fn is_japanese_compact_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3001}'..='\u{303F}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9F}'
    )
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

    /// 漢字・かなの間に挿入されたスペースを除去する
    #[test]
    fn normalize_removes_spaces_between_japanese_chars() {
        assert_eq!(normalize_ocr_text("日 本 語 の テ ス ト"), "日本語のテスト");
    }

    /// 英単語間のスペースは維持する
    #[test]
    fn normalize_keeps_spaces_between_latin_words() {
        assert_eq!(normalize_ocr_text("hello world"), "hello world");
    }

    /// 英字と日本語の間のスペースは維持する
    #[test]
    fn normalize_keeps_space_between_latin_and_japanese() {
        assert_eq!(normalize_ocr_text("API の 説明"), "API の説明");
    }

    /// 改行は維持する
    #[test]
    fn normalize_keeps_line_breaks() {
        assert_eq!(normalize_ocr_text("あ い\nう え"), "あい\nうえ");
    }
}
