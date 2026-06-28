//! クリップボード I/O の抽象化
//!
//! テスト用のインメモリ実装は `test_helpers::InMemoryTextClipboard` を参照

// ======================================================================
// トレイト
// ======================================================================
/// テキスト形式のクリップボード読み書き
pub(crate) trait TextClipboard {
    /// クリップボードからテキストを取得する
    fn get_text(&mut self) -> Result<String, String>;

    /// クリップボードへテキストを書き込む
    fn set_text(&mut self, text: String) -> Result<(), String>;
}

/// 画像形式のクリップボード読み書き
pub(crate) trait ImageClipboard {
    /// クリップボードから RGBA 画像を取得する
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), String>;

    /// クリップボードへ RGBA 画像を書き込む
    fn set_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String>;
}

impl TextClipboard for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, String> {
        arboard::Clipboard::get_text(self).map_err(|e| e.to_string())
    }

    fn set_text(&mut self, text: String) -> Result<(), String> {
        arboard::Clipboard::set_text(self, text).map_err(|e| e.to_string())
    }
}

impl ImageClipboard for arboard::Clipboard {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), String> {
        if let Ok(image) = arboard::Clipboard::get_image(self) {
            return Ok((
                u32::try_from(image.width).map_err(|e| e.to_string())?,
                u32::try_from(image.height).map_err(|e| e.to_string())?,
                image.bytes.into_owned(),
            ));
        }

        if let Some(image) = crate::platform::clipboard_image::read_dib_image() {
            return Ok((image.width, image.height, image.rgba));
        }

        Err("clipboard image not available".to_string())
    }

    fn set_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
        use arboard::ImageData;
        use std::borrow::Cow;

        arboard::Clipboard::set_image(
            self,
            ImageData {
                width: usize::try_from(width).map_err(|e| e.to_string())?,
                height: usize::try_from(height).map_err(|e| e.to_string())?,
                bytes: Cow::Owned(rgba),
            },
        )
        .map_err(|e| e.to_string())
    }
}
