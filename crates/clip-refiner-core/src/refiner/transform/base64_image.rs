use std::io::Cursor;

use image::{ImageBuffer, ImageFormat, ImageReader, Limits, RgbaImage};

use super::base64::{base64_decode_bytes, base64_encode_bytes};
use crate::consts;

// ======================================================================
// 型
// ======================================================================
/// Base64 と相互変換する画像の失敗理由
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Base64ImageError {
    /// 画像として扱えない (Base64 や画像形式として不正)
    Invalid,
    /// 画像の辺長またはピクセル数が上限を超えている
    TooLarge,
}

/// Base64 から復元した RGBA 画像
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedImage {
    /// 幅 (ピクセル)
    pub width: u32,
    /// 高さ (ピクセル)
    pub height: u32,
    /// RGBA ピクセル列
    pub rgba: Vec<u8>,
}

/// デコード時に確保できる最大バイト数
const MAX_DECODE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

// ======================================================================
// Base64 と画像の相互変換
// ======================================================================
/// Base64 文字列を画像 (PNG / JPEG / BMP) として復元する
///
/// 画像のデコードを始める前に、ヘッダーから読み取った寸法を上限と照合する。
///
/// # Arguments
/// * `input` - Base64 文字列 (data URI も可)
///
/// # Returns
/// * `Result<DecodedImage, Base64ImageError>` - 復元した RGBA 画像
///
/// # Errors
/// 画像として扱えない場合は `Invalid`、辺長またはピクセル数が上限を超える場合は `TooLarge`
pub(crate) fn decode_base64_image(input: &str) -> Result<DecodedImage, Base64ImageError> {
    let bytes = base64_decode_bytes(input).ok_or(Base64ImageError::Invalid)?;
    let format = image::guess_format(&bytes)
        .ok()
        .filter(|format| {
            matches!(
                format,
                ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Bmp
            )
        })
        .ok_or(Base64ImageError::Invalid)?;

    let (width, height) = ImageReader::with_format(Cursor::new(&bytes), format)
        .into_dimensions()
        .map_err(|_| Base64ImageError::Invalid)?;
    check_dimensions(width, height)?;

    let mut limits = Limits::default();
    limits.max_image_width = Some(consts::MAX_BASE64_IMAGE_DIMENSION);
    limits.max_image_height = Some(consts::MAX_BASE64_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    reader.limits(limits);
    let rgba = reader
        .decode()
        .map_err(|_| Base64ImageError::Invalid)?
        .into_rgba8();

    Ok(DecodedImage {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// RGBA 画像を PNG にして Base64 文字列へエンコードする
///
/// # Arguments
/// * `width` - 幅 (ピクセル)
/// * `height` - 高さ (ピクセル)
/// * `rgba` - RGBA ピクセル列
///
/// # Returns
/// * `Result<String, Base64ImageError>` - PNG の Base64 文字列 (標準アルファベット・パディングあり)
///
/// # Errors
/// バッファ長が寸法と合わない場合は `Invalid`、辺長またはピクセル数が上限を超える場合は `TooLarge`
pub(crate) fn encode_base64_png(
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<String, Base64ImageError> {
    check_dimensions(width, height)?;

    let image: RgbaImage =
        ImageBuffer::from_raw(width, height, rgba).ok_or(Base64ImageError::Invalid)?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|_| Base64ImageError::Invalid)?;

    Ok(base64_encode_bytes(&png))
}

/// 画像の寸法が 0 でなく、上限以内か検証する
fn check_dimensions(width: u32, height: u32) -> Result<(), Base64ImageError> {
    if width == 0 || height == 0 {
        return Err(Base64ImageError::Invalid);
    }
    if width > consts::MAX_BASE64_IMAGE_DIMENSION
        || height > consts::MAX_BASE64_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > consts::MAX_BASE64_IMAGE_PIXELS
    {
        return Err(Base64ImageError::TooLarge);
    }
    Ok(())
}

// ======================================================================
// テスト
// ======================================================================
/// 幅を差し替えた 2x2 の PNG を生成する (IHDR の CRC も更新する)
#[cfg(test)]
pub(crate) fn png_with_width(width: u32) -> Vec<u8> {
    let mut png = Vec::new();
    RgbaImage::new(2, 2)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("png");
    // シグネチャ 8 バイトの後ろに、長さ 4・種別 `IHDR` 4・本体 13・CRC 4 が続く
    png[16..20].copy_from_slice(&width.to_be_bytes());
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in &png[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    png[29..33].copy_from_slice(&(!crc).to_be_bytes());
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rgba() -> Vec<u8> {
        vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
    }

    /// 画像を PNG の Base64 へエンコードし、元の RGBA に復元できること
    #[test]
    fn test_png_base64_roundtrip() {
        let encoded = encode_base64_png(2, 2, sample_rgba()).expect("encode");
        // PNG シグネチャ `\x89PNG` の Base64 は `iVBORw0KGgo` で始まる
        assert!(encoded.starts_with("iVBORw0KGgo"));

        let decoded = decode_base64_image(&encoded).expect("decode");
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, sample_rgba());
    }

    /// data URI と折り返し付きの Base64 から画像を復元できること
    #[test]
    fn test_decode_data_uri_and_wrapped() {
        let encoded = encode_base64_png(2, 2, sample_rgba()).expect("encode");
        let wrapped = encoded
            .as_bytes()
            .chunks(16)
            .map(|chunk| std::str::from_utf8(chunk).expect("ascii"))
            .collect::<Vec<_>>()
            .join("\n");
        let uri = format!("data:image/png;base64,{wrapped}");

        let decoded = decode_base64_image(&uri).expect("decode");
        assert_eq!(decoded.rgba, sample_rgba());
    }

    /// 画像以外の Base64 は `Invalid` になること
    #[test]
    fn test_decode_non_image_is_invalid() {
        // "hello" と、PNG シグネチャだけで本体のないデータ
        assert_eq!(
            decode_base64_image("aGVsbG8="),
            Err(Base64ImageError::Invalid)
        );
        assert_eq!(
            decode_base64_image("iVBORw0KGgo="),
            Err(Base64ImageError::Invalid)
        );
        assert_eq!(decode_base64_image(""), Err(Base64ImageError::Invalid));
        // `BM` で始まるだけの文字列は BMP として扱わない
        assert_eq!(
            decode_base64_image(&base64_encode_bytes(b"BM is a text")),
            Err(Base64ImageError::Invalid)
        );
    }

    /// 上限を超える寸法は `TooLarge` になること
    #[test]
    fn test_oversized_dimensions_are_too_large() {
        let side = consts::MAX_BASE64_IMAGE_DIMENSION + 1;
        assert_eq!(
            encode_base64_png(side, 1, Vec::new()),
            Err(Base64ImageError::TooLarge)
        );

        // 4097x4097 は辺長こそ上限内だがピクセル数が上限を超える
        assert_eq!(
            check_dimensions(4097, 4097),
            Err(Base64ImageError::TooLarge)
        );
        assert_eq!(check_dimensions(4096, 4096), Ok(()));
    }

    /// バッファ長が寸法と合わない場合は `Invalid` になること
    #[test]
    fn test_encode_rejects_mismatched_buffer() {
        assert_eq!(
            encode_base64_png(2, 2, vec![0; 4]),
            Err(Base64ImageError::Invalid)
        );
        assert_eq!(
            encode_base64_png(0, 2, Vec::new()),
            Err(Base64ImageError::Invalid)
        );
    }

    /// 寸法を上限超過に書き換えた PNG は、デコードせずに `TooLarge` になること
    #[test]
    fn test_decode_rejects_oversized_png_header() {
        let png = png_with_width(consts::MAX_BASE64_IMAGE_DIMENSION + 1);

        assert_eq!(
            decode_base64_image(&base64_encode_bytes(&png)),
            Err(Base64ImageError::TooLarge)
        );
    }
}
