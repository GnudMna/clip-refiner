use std::io::Cursor;

use super::super::screen_capture::RgbaImage;
use super::normalize::normalize_ocr_text;
use super::prepare::prepare_ocr_image;

use anyhow::{Context, Result, bail};
use image::{ImageBuffer, Rgba};
use objc2::Alloc;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRecognizedText, VNRecognizedTextObservation,
    VNRequestTextRecognitionLevelAccurate,
};

// ======================================================================
// パブリック関数
// ======================================================================
/// RGBA 画像からテキストを認識する (Apple Vision)
pub(crate) fn recognize_text(image: &RgbaImage) -> Result<String> {
    if image.width == 0 || image.height == 0 {
        bail!("OCR 対象画像が空");
    }

    let prepared = prepare_ocr_image(image);
    let png_data = rgba_to_png(&prepared)?;
    let ns_data = NSData::with_bytes(&png_data);
    let options = NSDictionary::new();

    let handler = unsafe {
        VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &ns_data,
            &options,
        )
    };

    let request = unsafe {
        let request = VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc());
        request.setRecognitionLevel(VNRequestTextRecognitionLevelAccurate);
        request.setUsesLanguageCorrection(true);
        request.setAutomaticallyDetectsLanguage(true);
        let languages = NSArray::from_retained_slice(&[
            NSString::from_str("ja-JP"),
            NSString::from_str("en-US"),
        ]);
        request.setRecognitionLanguages(&languages);
        request
    };

    unsafe {
        handler
            .performRequests_error(&NSArray::from_retained_slice(&[request.clone()]), None)
            .map_err(|err| anyhow::anyhow!("Vision OCR の実行に失敗: {err:?}"))?;
    }

    let observations = request.results().context("Vision OCR の結果が空")?;

    let mut lines = Vec::new();
    let count = observations.count();
    for index in 0..count {
        let observation = observations.objectAtIndex(index);
        let observation = observation.downcast_ref::<VNRecognizedTextObservation>();
        let Some(observation) = observation else {
            continue;
        };
        let candidates = unsafe { observation.topCandidates(1) };
        if candidates.count() == 0 {
            continue;
        }
        let candidate = candidates.objectAtIndex(0);
        let candidate = candidate.downcast_ref::<VNRecognizedText>();
        let Some(recognized) = candidate else {
            continue;
        };
        let text = recognized.string().to_string();
        if !text.is_empty() {
            lines.push(text);
        }
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    Ok(normalize_ocr_text(&lines.join("\n")))
}

// ======================================================================
// プライベート関数
// ======================================================================
/// RGBA 画像を PNG バイト列へエンコードする
fn rgba_to_png(image: &RgbaImage) -> Result<Vec<u8>> {
    let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(image.width, image.height, image.rgba.clone())
            .context("RGBA バッファの構築に失敗")?;
    let mut png = Vec::new();
    buffer
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .context("PNG エンコードに失敗")?;
    Ok(png)
}
