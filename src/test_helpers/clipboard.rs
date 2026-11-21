#![allow(clippy::missing_panics_doc)]

use crate::refiner::text_clipboard::TextClipboard;

// ======================================================================
// インメモリクリップボード
// ======================================================================
/// 単体・統合テスト用のインメモリクリップボード
pub struct InMemoryTextClipboard {
    text: String,
    fail_on_read: bool,
    fail_on_write: bool,
}

impl InMemoryTextClipboard {
    /// 指定テキストを保持するクリップボードを生成する
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fail_on_read: false,
            fail_on_write: false,
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
