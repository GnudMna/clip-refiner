//! Excel コピー時にクリップボードへ載る描画済みビットマップを取得する
//!
//! `OpenClipboard` を伴う本文 I/O のうち、`CF_DIB` / `CF_DIBV5` 専用の短命経路
//! 通常のテキスト / 画像 I/O は `OwnedClipboard` (`arboard`) 側に集約し、
//! 変更検知 (`seq_num` / リスナ) とは役割を分けている

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
/// `CF_DIB` / `CF_DIBV5` がクリップボード上にあるか判定する
///
/// `OpenClipboard` は不要 (`IsClipboardFormatAvailable`)
pub(crate) fn dib_format_available() -> bool {
    use clipboard_win::{formats, is_format_avail};

    is_format_avail(formats::CF_DIBV5) || is_format_avail(formats::CF_DIB)
}

/// クリップボード上の Excel 描画ビットマップ (`CF_DIB` / `CF_DIBV5`) を RGBA へ変換する
///
/// 呼び出し中だけ `clipboard_win` で短命に `OpenClipboard` する
/// `OwnedClipboard` / `arboard` 側の操作と同時には呼ばないこと
pub(crate) fn read_dib_image() -> Option<ClipboardImage> {
    use clipboard_win::{Clipboard, formats, is_format_avail, raw};

    // 短命 RAII。Drop で CloseClipboard する
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

// ======================================================================
// プライベート関数
// ======================================================================
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

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::{decode_dib_to_rgba, tweak_dibv5_header};

    /// alpha 付き 32bit `BI_RGB` を `BI_BITFIELDS` へ補正すること
    #[test]
    fn tweak_converts_alpha_bi_rgb_to_bitfields() {
        let mut dib = vec![0u8; 56];
        // biSize = 56 (BITMAPV5HEADER)
        dib[0..4].copy_from_slice(&56u32.to_le_bytes());
        // biBitCount = 32
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        // biCompression = BI_RGB
        dib[16..20].copy_from_slice(&0u32.to_le_bytes());
        // bV5AlphaMask
        dib[52..56].copy_from_slice(&0xff00_0000_u32.to_le_bytes());

        tweak_dibv5_header(&mut dib);

        let compression = u32::from_le_bytes(dib[16..20].try_into().expect("compression"));
        assert_eq!(compression, 3); // BI_BITFIELDS
        assert_eq!(
            u32::from_le_bytes(dib[40..44].try_into().expect("red")),
            0x00ff_0000
        );
        assert_eq!(
            u32::from_le_bytes(dib[44..48].try_into().expect("green")),
            0x0000_ff00
        );
        assert_eq!(
            u32::from_le_bytes(dib[48..52].try_into().expect("blue")),
            0x0000_00ff
        );
    }

    /// 条件を満たさないヘッダーは変更しないこと
    #[test]
    fn tweak_skips_non_alpha_headers() {
        let mut dib = vec![0u8; 56];
        dib[14..16].copy_from_slice(&24u16.to_le_bytes());
        let before = dib.clone();

        tweak_dibv5_header(&mut dib);

        assert_eq!(dib, before);
    }

    /// 1x1 の 24bit DIB を RGBA へデコードできること
    #[test]
    fn decode_minimal_24bit_dib() {
        // BITMAPINFOHEADER (40) + 1 ピクセル BGR + パディング 1 バイト = 44
        let mut dib = vec![0u8; 44];
        dib[0..4].copy_from_slice(&40u32.to_le_bytes()); // biSize
        dib[4..8].copy_from_slice(&1i32.to_le_bytes()); // width
        dib[8..12].copy_from_slice(&1i32.to_le_bytes()); // height
        dib[12..14].copy_from_slice(&1u16.to_le_bytes()); // planes
        dib[14..16].copy_from_slice(&24u16.to_le_bytes()); // bit count
        // pixel BGR = blue, green, red
        dib[40] = 10;
        dib[41] = 20;
        dib[42] = 30;

        let image = decode_dib_to_rgba(&mut dib).expect("decode");
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.rgba, vec![30, 20, 10, 255]);
    }
}
