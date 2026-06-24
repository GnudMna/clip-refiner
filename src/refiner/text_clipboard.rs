//! クリップボード I/O の抽象化
//!
//! テストでは `InMemoryTextClipboard` を使い、システムクリップボードに依存しない

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

impl TextClipboard for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, String> {
        arboard::Clipboard::get_text(self).map_err(|e| e.to_string())
    }

    fn set_text(&mut self, text: String) -> Result<(), String> {
        arboard::Clipboard::set_text(self, text).map_err(|e| e.to_string())
    }
}

// ======================================================================
// インメモリ実装
// ======================================================================
/// テスト用のインメモリクリップボード
#[cfg(test)]
pub(crate) struct InMemoryTextClipboard {
    text: String,
    fail_on_read: bool,
    fail_on_write: bool,
}

#[cfg(test)]
impl InMemoryTextClipboard {
    /// 指定テキストを保持するクリップボードを生成する
    pub(crate) fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fail_on_read: false,
            fail_on_write: false,
        }
    }

    /// 次の `get_text` を失敗させる
    pub(crate) fn fail_on_read(mut self) -> Self {
        self.fail_on_read = true;
        self
    }

    /// 次の `set_text` を失敗させる
    pub(crate) fn fail_on_write(mut self) -> Self {
        self.fail_on_write = true;
        self
    }

    /// 保持中のテキストを返す
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
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
