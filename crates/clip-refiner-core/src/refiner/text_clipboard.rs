//! クリップボード I/O の抽象化
//!
//! 本番の所有型は `OwnedClipboard` (feature `clipboard`)。テスト用のインメモリ実装は
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
    /// クリップボードが空、またはテキスト形式ではない
    #[error("クリップボードにテキストがありません")]
    TextUnavailable,
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

    /// クリップボードへテキストを、履歴・クラウド同期・監視から除外する形式付きで書き込む
    ///
    /// 本文と除外形式を 1 回の書き込みで載せる。既定実装は除外形式を付けず `set_text` と同じ
    ///
    /// # Errors
    /// クリップボードへの書き込みに失敗した場合
    fn set_excluded_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        self.set_text(text)
    }

    /// 現在の内容がパスワードマネージャー等により監視除外を指定されているか判定する
    ///
    /// 本文を読む前に呼ぶ。既定実装は常に `false`
    ///
    /// # Arguments
    /// * `self` - 判定対象のクリップボード
    ///
    /// # Returns
    /// * `bool` - 除外指定がある場合は `true`
    fn is_excluded_from_monitoring(&mut self) -> bool {
        false
    }

    /// クリップボードの内容が変わると進むトークンを返す
    ///
    /// 読み取りと書き戻しの間に別のコピーが入っていないかの確認に使う
    /// 既定実装は変更を検知できないため `None`
    ///
    /// # Arguments
    /// * `self` - 判定対象のクリップボード
    ///
    /// # Returns
    /// * `Option<u64>` - 取得できた場合は `Some(トークン)`、未対応なら `None`
    fn change_token(&mut self) -> Option<u64> {
        None
    }
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

    /// クリップボードへ RGBA 画像を書き込み、エクスプローラへもファイルとして貼り付けられるようにする
    ///
    /// ビットマップに加えて、PNG の仮想ファイルを同時に載せる実装向け。ディスクへは書き出さない
    /// ファイルを載せられない実装 (既定) はビットマップのみを書き込む
    ///
    /// # Errors
    /// ビットマップの書き込みに失敗した場合。仮想ファイルを載せられなくてもビットマップは書き込み、エラーにしない
    fn set_image_as_file(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        self.set_image(width, height, rgba)
    }
}

/// HTML 形式 (`CF_HTML`) のクリップボード読み書き
pub trait HtmlClipboard {
    /// クリップボードから HTML 断片を取得する
    ///
    /// `CF_HTML` の `StartFragment` から `EndFragment` までを返す
    ///
    /// # Returns
    /// * `Ok(Some(html))` - HTML 形式が存在する
    /// * `Ok(None)` - HTML 形式が存在しない
    ///
    /// # Errors
    /// HTML 形式は存在するが読み取りに失敗した場合
    fn get_html(&mut self) -> Result<Option<String>, ClipboardIoError>;

    /// クリップボードへ HTML と代替テキストを書き込む
    ///
    /// 既存の内容を消去し、`CF_UNICODETEXT` に `alt_text`、`CF_HTML` に `html` を置く
    ///
    /// # Errors
    /// クリップボードへの書き込みに失敗した場合
    fn set_html(&mut self, html: String, alt_text: String) -> Result<(), ClipboardIoError>;

    /// `set_html` と同じ内容を、履歴・クラウド同期・監視から除外する形式付きで書き込む
    ///
    /// 既定実装は除外形式を付けず `set_html` と同じ
    ///
    /// # Errors
    /// クリップボードへの書き込みに失敗した場合
    fn set_excluded_html(
        &mut self,
        html: String,
        alt_text: String,
    ) -> Result<(), ClipboardIoError> {
        self.set_html(html, alt_text)
    }
}

#[cfg(feature = "clipboard")]
impl TextClipboard for arboard::Clipboard {
    fn get_text(&mut self) -> Result<String, ClipboardIoError> {
        arboard::Clipboard::get_text(self).map_err(|e| match e {
            arboard::Error::ContentNotAvailable => ClipboardIoError::TextUnavailable,
            other => ClipboardIoError::ReadFailed(other.to_string()),
        })
    }

    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        arboard::Clipboard::set_text(self, text)
            .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))
    }

    fn set_excluded_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        use arboard::SetExtWindows;

        self.set()
            .exclude_from_monitoring()
            .exclude_from_history()
            .exclude_from_cloud()
            .text(text)
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

#[cfg(feature = "clipboard")]
impl HtmlClipboard for arboard::Clipboard {
    fn get_html(&mut self) -> Result<Option<String>, ClipboardIoError> {
        // 存在確認は `OpenClipboard` を伴わない。無い場合は arboard の汎用エラーと区別できないため先に判定する
        let available = clipboard_win::register_format("HTML Format")
            .is_some_and(|format| clipboard_win::is_format_avail(format.get()));
        if !available {
            return Ok(None);
        }

        self.get()
            .html()
            .map(Some)
            .map_err(|e| ClipboardIoError::ReadFailed(e.to_string()))
    }

    fn set_html(&mut self, html: String, alt_text: String) -> Result<(), ClipboardIoError> {
        arboard::Clipboard::set_html(self, html, Some(alt_text))
            .map_err(|e| ClipboardIoError::WriteFailed(e.to_string()))
    }

    fn set_excluded_html(
        &mut self,
        html: String,
        alt_text: String,
    ) -> Result<(), ClipboardIoError> {
        use arboard::SetExtWindows;

        self.set()
            .exclude_from_monitoring()
            .exclude_from_history()
            .exclude_from_cloud()
            .html(html, Some(alt_text))
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
