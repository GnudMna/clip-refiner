use std::ptr;
use std::slice;

use anyhow::{Context, Result, bail};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::System::WinRT::IMemoryBufferByteAccess;
use windows::core::{HSTRING, Interface};

use super::super::screen_capture::RgbaImage;
use super::normalize::normalize_ocr_text;
use super::prepare::prepare_ocr_image;

// ======================================================================
// パブリック関数
// ======================================================================
/// RGBA 画像からテキストを認識する (`Windows.Media.Ocr`)
pub(crate) fn recognize_text(image: &RgbaImage) -> Result<String> {
    validate_ocr_image(image)?;

    let prepared = prepare_ocr_image(image);
    let bitmap = create_ocr_bitmap(&prepared)?;

    let engine = create_ocr_engine()
        .context("OCR エンジンの初期化に失敗 (日本語の言語パックが未インストールの可能性)")?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .context("OCR の開始に失敗")?
        .join()
        .context("OCR の完了待ちに失敗")?;

    // `OcrResult::Text` は行を空白でつなぐため、改行が失われる。行ごとに取り出す
    let lines = result
        .Lines()
        .context("OCR 結果の行一覧の取得に失敗")?
        .into_iter()
        .map(|line| {
            line.Text()
                .map(|text| text.to_string())
                .context("OCR 行テキストの取得に失敗")
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(join_normalized_lines(&lines))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// 行ごとに正規化し、改行でつなぐ
///
/// 行をまたいで空白を詰めないよう、正規化は行単位で行う
fn join_normalized_lines<S: AsRef<str>>(lines: &[S]) -> String {
    lines
        .iter()
        .map(|line| normalize_ocr_text(line.as_ref()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// OCR 対象画像が空でないことを検証する
fn validate_ocr_image(image: &RgbaImage) -> Result<()> {
    if image.width == 0 || image.height == 0 {
        bail!("OCR 対象画像が空");
    }
    Ok(())
}

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
    // SAFETY: `access` は生存中の `reference` から得たインターフェースで、出力先はローカル変数
    unsafe {
        access
            .GetBuffer(ptr::from_mut(&mut data), ptr::from_mut(&mut capacity))
            .context("ビットマップバッファの取得に失敗")?;
    }

    let capacity = usize::try_from(capacity).context("バッファ容量の変換に失敗")?;
    if capacity < rgba.len() {
        bail!("ビットマップバッファの容量が不足");
    }

    // SAFETY: `data` は `GetBuffer` が返した `capacity` バイトの書き込み可能な領域
    // `buffer` / `reference` がロックを保持する間 (この関数の終わりまで) 有効で、他から参照されない
    let slice = unsafe { slice::from_raw_parts_mut(data, capacity) };
    copy_rgba_to_bgra(rgba, slice);
    Ok(())
}

/// RGBA ピクセル列を BGRA バッファへ書き込む
///
/// `dest` は `rgba` 以上の長さであること
fn copy_rgba_to_bgra(rgba: &[u8], dest: &mut [u8]) {
    for (pixel_dest, src) in dest.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        pixel_dest[0] = src[2];
        pixel_dest[1] = src[1];
        pixel_dest[2] = src[0];
        pixel_dest[3] = src[3];
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// 複数行は改行を保ったまま行ごとに正規化すること
    ///
    /// 行をまたいで日本語の間の空白とみなして詰めない
    #[test]
    fn join_normalized_lines_keeps_line_breaks_between_japanese_lines() {
        assert_eq!(
            join_normalized_lines(&["日 本 語 の 一 行 目", "二 行 目 の テ キ ス ト"]),
            "日本語の一行目\n二行目のテキスト"
        );
    }

    /// 1 行だけの場合は改行を付けないこと
    #[test]
    fn join_normalized_lines_single_line_has_no_line_break() {
        assert_eq!(join_normalized_lines(&["あ い"]), "あい");
    }

    /// 行がない場合は空文字列を返すこと
    #[test]
    fn join_normalized_lines_empty_returns_empty() {
        assert_eq!(join_normalized_lines::<&str>(&[]), "");
    }

    /// 幅または高さが 0 の画像は拒否すること
    #[test]
    fn validate_ocr_image_rejects_empty() {
        assert!(
            validate_ocr_image(&RgbaImage {
                width: 0,
                height: 10,
                rgba: vec![],
            })
            .is_err()
        );
        assert!(
            validate_ocr_image(&RgbaImage {
                width: 10,
                height: 0,
                rgba: vec![],
            })
            .is_err()
        );
    }

    /// 有効な画像は検証を通るようにすること
    #[test]
    fn validate_ocr_image_accepts_non_empty() {
        assert!(
            validate_ocr_image(&RgbaImage {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 255],
            })
            .is_ok()
        );
    }

    /// RGBA から BGRA へチャンネルを入れ替えるようにすること
    #[test]
    fn copy_rgba_to_bgra_swaps_red_and_blue() {
        let rgba = [10, 20, 30, 40, 50, 60, 70, 80];
        let mut dest = [0u8; 8];
        copy_rgba_to_bgra(&rgba, &mut dest);
        assert_eq!(dest, [30, 20, 10, 40, 70, 60, 50, 80]);
    }

    /// 公開 API も空画像を拒否すること
    #[test]
    fn recognize_text_rejects_empty_image() {
        let err = recognize_text(&RgbaImage {
            width: 0,
            height: 1,
            rgba: vec![],
        })
        .expect_err("空画像は失敗する");
        assert!(err.to_string().contains("空"));
    }

    /// 小さな RGBA 画像から `SoftwareBitmap` を生成できるようにすること
    #[test]
    fn create_ocr_bitmap_accepts_tiny_rgba_image() {
        let image = RgbaImage {
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, // R
                0, 255, 0, 255, // G
                0, 0, 255, 255, // B
                255, 255, 255, 255, // W
            ],
        };
        let bitmap = create_ocr_bitmap(&image).expect("SoftwareBitmap を生成できる");
        assert_eq!(bitmap.PixelWidth().expect("幅"), 2);
        assert_eq!(bitmap.PixelHeight().expect("高さ"), 2);
    }

    /// 単色画像でも OCR パイプラインが完走すること
    ///
    /// 言語パック未導入環境ではエンジン初期化失敗を許容する
    #[test]
    fn recognize_text_runs_on_solid_image_when_engine_available() {
        let image = RgbaImage {
            width: 64,
            height: 32,
            rgba: vec![255; 64 * 32 * 4],
        };

        match recognize_text(&image) {
            Ok(text) => {
                // 単色なので検出なしが通常。失敗せず完走すればよい
                let _ = text;
            }
            Err(err) => {
                let message = format!("{err:#}");
                assert!(
                    message.contains("OCR エンジン")
                        || message.contains("言語")
                        || message.contains("言語パック"),
                    "想定外の失敗: {message}"
                );
            }
        }
    }
}
