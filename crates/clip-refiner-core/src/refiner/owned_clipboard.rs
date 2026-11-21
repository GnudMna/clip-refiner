//! OS クリップボード本文 I/O の単一所有者
//!
//! 常駐時はワーカースレッドが 1 インスタンスだけ保持する
//! 変更検知 (`clipboard_win::raw::seq_num` / `AddClipboardFormatListener`) は
//! `OpenClipboard` を伴わない別経路であり、本文読み書きはこの型に集約する

use super::text_clipboard::{ClipboardIoError, HtmlClipboard, ImageClipboard, TextClipboard};

// ======================================================================
// 画像ファイルの掃除
// ======================================================================
/// 貼り付け用に書き出した画像ファイルのうち、保持時間を過ぎたものを削除する
///
/// 画像をエクスプローラへ貼り付けられるよう `set_image_as_file` が一時フォルダへ書き出した PNG が対象。
/// 保持時間内のファイルは、クリップボードが参照している可能性があるため残す。
/// 常駐の起動時に呼ぶ想定で、フォルダが無い場合や削除できない場合は何もしない
pub fn remove_stale_image_files() {
    crate::native::clipboard_file::remove_stale_image_files();
}

// ======================================================================
// 所有クリップボード
// ======================================================================
/// ワーカー (またはワンショット) が保持する OS クリップボード接続
///
/// `arboard::Clipboard` を包み、プロセス内で同時に複数生成しない運用を型で示す
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

    fn is_excluded_from_monitoring(&mut self) -> bool {
        crate::native::clipboard_exclusion::is_excluded_from_monitoring()
    }

    fn change_token(&mut self) -> Option<u64> {
        clipboard_win::raw::seq_num().map(|s| u64::from(s.get()))
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

    fn set_image_as_file(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        ImageClipboard::set_image_as_file(&mut self.inner, width, height, rgba)
    }
}

impl HtmlClipboard for OwnedClipboard {
    fn get_html(&mut self) -> Result<Option<String>, ClipboardIoError> {
        HtmlClipboard::get_html(&mut self.inner)
    }

    fn set_html(&mut self, html: String, alt_text: String) -> Result<(), ClipboardIoError> {
        HtmlClipboard::set_html(&mut self.inner, html, alt_text)
    }
}
