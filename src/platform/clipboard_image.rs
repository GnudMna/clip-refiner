//! Excel コピー時にクリップボードへ載る描画済みビットマップを取得する

// ======================================================================
// 画像バッファ
// ======================================================================
/// クリップボードから取得した RGBA 画像
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImage {
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
/// クリップボード上の Excel 描画ビットマップ (`CF_DIB` / `CF_DIBV5`) を RGBA へ変換する
///
/// `arboard::Clipboard::get_image` が取得できない場合の Windows 向けフォールバック
#[cfg(windows)]
pub(crate) fn read_dib_image() -> Option<ClipboardImage> {
    use clipboard_win::{Clipboard, formats, is_format_avail, raw};

    let _clipboard = Clipboard::new().ok()?;

    let format = if is_format_avail(formats::CF_DIBV5) {
        formats::CF_DIBV5
    } else if is_format_avail(formats::CF_DIB) {
        formats::CF_DIB
    } else {
        return None;
    };

    let mut data = Vec::new();
    raw::get_vec(format, &mut data).ok()?;
    decode_dib_to_rgba(&mut data)
}

#[cfg(not(windows))]
pub(crate) fn read_dib_image() -> Option<ClipboardImage> {
    None
}

// ======================================================================
// プライベート関数
// ======================================================================
#[cfg(windows)]
fn decode_dib_to_rgba(data: &mut [u8]) -> Option<ClipboardImage> {
    use std::io::Cursor;

    use image::codecs::bmp::BmpDecoder;
    use image::{DynamicImage, ImageDecoder};

    tweak_dibv5_header(data);

    let decoder = BmpDecoder::new_without_file_header(Cursor::new(&*data)).ok()?;
    let (width, height) = decoder.dimensions();
    let rgba = DynamicImage::from_decoder(decoder)
        .ok()?
        .into_rgba8()
        .into_raw();

    Some(ClipboardImage {
        width,
        height,
        rgba,
    })
}

/// 32bit `BI_RGB` ヘッダーを `BmpDecoder` が解釈できるよう調整する
///
/// Chrome / Excel 等が alpha 付き 32bit DIB を `BI_RGB` で載せるケースへの対応
/// (`arboard` の Windows 実装と同様)
#[cfg(windows)]
fn tweak_dibv5_header(dib: &mut [u8]) {
    const BI_RGB: u32 = 0;
    const BI_BITFIELDS: u32 = 3;
    const BIT_COUNT_OFFSET: usize = 14;
    const COMPRESSION_OFFSET: usize = 16;
    const RED_MASK_OFFSET: usize = 40;
    const GREEN_MASK_OFFSET: usize = 44;
    const BLUE_MASK_OFFSET: usize = 48;
    const ALPHA_MASK_OFFSET: usize = 52;
    const HEADER_MIN_LEN: usize = ALPHA_MASK_OFFSET + 4;

    if dib.len() < HEADER_MIN_LEN {
        return;
    }

    let bit_count = u16::from_le_bytes([dib[BIT_COUNT_OFFSET], dib[BIT_COUNT_OFFSET + 1]]);
    let compression = u32::from_le_bytes([
        dib[COMPRESSION_OFFSET],
        dib[COMPRESSION_OFFSET + 1],
        dib[COMPRESSION_OFFSET + 2],
        dib[COMPRESSION_OFFSET + 3],
    ]);
    let alpha_mask = u32::from_le_bytes([
        dib[ALPHA_MASK_OFFSET],
        dib[ALPHA_MASK_OFFSET + 1],
        dib[ALPHA_MASK_OFFSET + 2],
        dib[ALPHA_MASK_OFFSET + 3],
    ]);

    if bit_count != 32 || compression != BI_RGB || alpha_mask != 0xff00_0000 {
        return;
    }

    dib[COMPRESSION_OFFSET..COMPRESSION_OFFSET + 4].copy_from_slice(&BI_BITFIELDS.to_le_bytes());

    let red_mask = u32::from_le_bytes([
        dib[RED_MASK_OFFSET],
        dib[RED_MASK_OFFSET + 1],
        dib[RED_MASK_OFFSET + 2],
        dib[RED_MASK_OFFSET + 3],
    ]);
    let green_mask = u32::from_le_bytes([
        dib[GREEN_MASK_OFFSET],
        dib[GREEN_MASK_OFFSET + 1],
        dib[GREEN_MASK_OFFSET + 2],
        dib[GREEN_MASK_OFFSET + 3],
    ]);
    let blue_mask = u32::from_le_bytes([
        dib[BLUE_MASK_OFFSET],
        dib[BLUE_MASK_OFFSET + 1],
        dib[BLUE_MASK_OFFSET + 2],
        dib[BLUE_MASK_OFFSET + 3],
    ]);

    if red_mask == 0 && green_mask == 0 && blue_mask == 0 {
        dib[RED_MASK_OFFSET..RED_MASK_OFFSET + 4].copy_from_slice(&0x00ff_0000_u32.to_le_bytes());
        dib[GREEN_MASK_OFFSET..GREEN_MASK_OFFSET + 4]
            .copy_from_slice(&0x0000_ff00_u32.to_le_bytes());
        dib[BLUE_MASK_OFFSET..BLUE_MASK_OFFSET + 4].copy_from_slice(&0x0000_00ff_u32.to_le_bytes());
    }
}
