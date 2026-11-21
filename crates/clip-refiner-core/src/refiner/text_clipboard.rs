//! クリップボード I/O の抽象化
//!
//! 本番の所有型は [`super::OwnedClipboard`]。テスト用のインメモリ実装は
//! `test_helpers::InMemoryTextClipboard` を参照

// ======================================================================
// エラー型
// ======================================================================
/// クリップボード I/O の失敗理由
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClipboardIoError {
    /// テキストまたは画像の読み取りに失敗
    #[error("クリップボードの読み取りに失敗しました: {0}")]
    ReadFailed(String),
    /// テキストまたは画像の書き込みに失敗
    #[error("クリップボードの書き込みに失敗しました: {0}")]
    WriteFailed(String),
    /// クリップボードに画像が存在しない
    #[error("クリップボードに画像がありません")]
    ImageUnavailable,
}

// ======================================================================
// トレイト
// ======================================================================
/// テキスト形式のクリップボード読み書き
pub trait TextClipboard {
    /// クリップボードからテキストを取得する
    ///
    /// # Errors
    /// クリップボードの読み取りに失敗した場合
    fn get_text(&mut self) -> Result<String, ClipboardIoError>;

    /// クリップボードへテキストを書き込む
    ///
    /// # Errors
    /// クリップボードへの書き込みに失敗した場合
    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError>;
}

/// 画像形式のクリップボード読み書き
pub trait ImageClipboard {
    /// クリップボードから RGBA 画像を取得する
    ///
    /// # Errors
    /// 画像が存在しない、または読み取りに失敗した場合
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), ClipboardIoError>;

    /// クリップボードへ RGBA 画像を書き込む
    ///
    /// # Errors
    /// クリップボードへの書き込みに失敗した場合
    fn set_image(&mut self, width: u32, height: u32, rgba: Vec<u8>)
    -> Result<(), ClipboardIoError>;
}

#[cfg(feature = "clipboard")]
impl TextClipboard for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, ClipboardIoError> {
        arboard::Clipboard::get_text(self).map_err(|e| ClipboardIoError::ReadFailed(e.to_string()))
    }

    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        arboard::Clipboard::set_text(self, text)
            .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))
    }
}

#[cfg(feature = "clipboard")]
impl ImageClipboard for arboard::Clipboard {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), ClipboardIoError> {
        read_rgba_image(self)
    }

    fn set_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        use std::borrow::Cow;

        use arboard::ImageData;

        arboard::Clipboard::set_image(
            self,
            ImageData {
                width: usize::try_from(width)
                    .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))?,
                height: usize::try_from(height)
                    .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))?,
                bytes: Cow::Owned(rgba),
            },
        )
        .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))
    }
}

/// 画像を 1 経路だけで読む (同一呼び出しで arboard と clipboard-win を重ねて開かない)
///
/// `CF_DIB` / `CF_DIBV5` がある場合は Excel 向け DIB デコードのみを試し、
/// 無い場合または DIB 読み失敗時のみ `arboard` を使う
#[cfg(feature = "clipboard")]
fn read_rgba_image(
    clipboard: &mut arboard::Clipboard,
) -> Result<(u32, u32, Vec<u8>), ClipboardIoError> {
    if crate::native::clipboard_image::dib_format_available()
        && let Some(image) = crate::native::clipboard_image::read_dib_image()
    {
        return Ok((image.width, image.height, image.rgba));
    }

    match arboard::Clipboard::get_image(clipboard) {
        Ok(image) => Ok((
            u32::try_from(image.width).map_err(|e| ClipboardIoError::ReadFailed(e.to_string()))?,
            u32::try_from(image.height).map_err(|e| ClipboardIoError::ReadFailed(e.to_string()))?,
            image.bytes.into_owned(),
        )),
        Err(_) => Err(ClipboardIoError::ImageUnavailable),
    }
}
