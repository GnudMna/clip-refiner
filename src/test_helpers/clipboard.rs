#![allow(clippy::missing_panics_doc)]

use crate::refiner::text_clipboard::{ImageClipboard, TextClipboard};

// ======================================================================
// インメモリクリップボード
// ======================================================================
/// 単体・統合テスト用のインメモリクリップボード
pub struct InMemoryTextClipboard {
    text: String,
    source_image: Option<(u32, u32, Vec<u8>)>,
    written_image: Option<(u32, u32, Vec<u8>)>,
    fail_on_read: bool,
    fail_on_write: bool,
}

impl InMemoryTextClipboard {
    /// 指定テキストを保持するクリップボードを生成する
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            source_image: None,
            written_image: None,
            fail_on_read: false,
            fail_on_write: false,
        }
    }

    /// Excel 描画ビットマップを同時に保持するクリップボードを生成する
    #[must_use]
    pub fn with_source_image(self, width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            source_image: Some((width, height, rgba)),
            ..self
        }
    }

    /// 次の `get_text` を失敗させる
    #[must_use]
    pub fn fail_on_read(mut self) -> Self {
        self.fail_on_read = true;
        self
    }

    /// 次の `set_text` を失敗させる
    #[must_use]
    pub fn fail_on_write(mut self) -> Self {
        self.fail_on_write = true;
        self
    }

    /// 保持中のテキストを返す
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 書き込み後の画像サイズを返す
    pub fn written_image_size(&self) -> Option<(u32, u32)> {
        self.written_image
            .as_ref()
            .map(|(width, height, _)| (*width, *height))
    }
}

impl TextClipboard for InMemoryTextClipboard {
    fn get_text(&mut self) -> Result<String, String> {
        if self.fail_on_read {
            return Err("read failed".to_string());
        }
        Ok(self.text.clone())
    }

    fn set_text(&mut self, text: String) -> Result<(), String> {
        if self.fail_on_write {
            return Err("write failed".to_string());
        }
        self.text = text;
        Ok(())
    }
}

impl ImageClipboard for InMemoryTextClipboard {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), String> {
        self.source_image
            .clone()
            .ok_or_else(|| "no image".to_string())
    }

    fn set_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
        self.written_image = Some((width, height, rgba));
        Ok(())
    }
}
