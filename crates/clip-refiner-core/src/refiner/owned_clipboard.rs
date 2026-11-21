//! OS クリップボード本文 I/O の単一所有者
//!
//! 常駐時はワーカースレッドが 1 インスタンスだけ保持する。
//! 変更検知 (`clipboard_win::raw::seq_num` / `AddClipboardFormatListener`) は
//! `OpenClipboard` を伴わない別経路であり、本文読み書きはこの型に集約する

use super::text_clipboard::{ClipboardIoError, ImageClipboard, TextClipboard};

// ======================================================================
// 所有クリップボード
// ======================================================================
/// ワーカー (またはワンショット) が保持する OS クリップボード接続
///
/// `arboard::Clipboard` を包み、プロセス内で同時に複数生成しない運用を型で示す。
/// 画像取得では `CF_DIB` 系と通常画像で `OpenClipboard` 経路を排他する
pub struct OwnedClipboard {
    inner: arboard::Clipboard,
}

impl OwnedClipboard {
    /// OS クリップボードへの接続を 1 本開く
    ///
    /// # Returns
    /// * `Result<Self, arboard::Error>` - 初期化に成功した所有者
    ///
    /// # Errors
    /// クリップボード初期化に失敗した場合
    pub fn new() -> Result<Self, arboard::Error> {
        Ok(Self {
            inner: arboard::Clipboard::new()?,
        })
    }
}

impl TextClipboard for OwnedClipboard {
    fn get_text(&mut self) -> Result<String, ClipboardIoError> {
        TextClipboard::get_text(&mut self.inner)
    }

    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        TextClipboard::set_text(&mut self.inner, text)
    }
}

impl ImageClipboard for OwnedClipboard {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), ClipboardIoError> {
        ImageClipboard::get_image(&mut self.inner)
    }

    fn set_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        ImageClipboard::set_image(&mut self.inner, width, height, rgba)
    }
}
