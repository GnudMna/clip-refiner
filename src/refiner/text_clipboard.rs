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

impl TextClipboard for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, String> {
        arboard::Clipboard::get_text(self).map_err(|e| e.to_string())
    }

    fn set_text(&mut self, text: String) -> Result<(), String> {
        arboard::Clipboard::set_text(self, text).map_err(|e| e.to_string())
    }
}
