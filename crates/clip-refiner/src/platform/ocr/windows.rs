use std::ptr;
use std::slice;

use super::super::screen_capture::RgbaImage;
use super::normalize::normalize_ocr_text;
use super::prepare::prepare_ocr_image;

use anyhow::{Context, Result, bail};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::System::WinRT::IMemoryBufferByteAccess;
use windows::core::{HSTRING, Interface};

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
