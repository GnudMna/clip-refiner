use crate::refiner::text_clipboard::{
    ClipboardIoError, HtmlClipboard, ImageClipboard, TextClipboard,
};

// ======================================================================
// インメモリクリップボード
// ======================================================================
/// 単体・統合テスト用のインメモリクリップボード
pub struct InMemoryTextClipboard {
    text: String,
    html: Option<String>,
    source_image: Option<(u32, u32, Vec<u8>)>,
    written_image: Option<(u32, u32, Vec<u8>)>,
    fail_on_read: bool,
    fail_on_write: bool,
    excluded_from_monitoring: bool,
    change_token: u64,
    external_copy_after_read: Option<ExternalCopy>,
}

/// `get_text` の直後に割り込む別アプリのコピー
struct ExternalCopy {
    text: String,
    excluded_from_monitoring: bool,
}

impl InMemoryTextClipboard {
    /// 指定テキストを保持するクリップボードを生成する
    ///
    /// # Arguments
    /// * `text` - 初期テキスト
    ///
    /// # Returns
    /// * `Self` - 生成したインメモリクリップボード
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            html: None,
            source_image: None,
            written_image: None,
            fail_on_read: false,
            fail_on_write: false,
            excluded_from_monitoring: false,
            change_token: 0,
            external_copy_after_read: None,
        }
    }

    /// Excel 描画ビットマップを同時に保持するクリップボードを生成する
    ///
    /// # Arguments
    /// * `self` - ベースとなるインメモリクリップボード
    /// * `width` - 画像幅 (ピクセル)
    /// * `height` - 画像高さ (ピクセル)
    /// * `rgba` - RGBA ピクセル列
    ///
    /// # Returns
    /// * `Self` - 画像付きクリップボード
    #[must_use]
    pub fn with_source_image(self, width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            source_image: Some((width, height, rgba)),
            ..self
        }
    }

    /// HTML 形式 (`CF_HTML` の断片) を同時に保持するクリップボードを生成する
    ///
    /// # Arguments
    /// * `self` - ベースとなるインメモリクリップボード
    /// * `html` - HTML 断片
    ///
    /// # Returns
    /// * `Self` - HTML 付きクリップボード
    #[must_use]
    pub fn with_html(self, html: impl Into<String>) -> Self {
        Self {
            html: Some(html.into()),
            ..self
        }
    }

    /// 次の `get_text` を失敗させる
    ///
    /// # Arguments
    /// * `self` - 設定対象のインメモリクリップボード
    ///
    /// # Returns
    /// * `Self` - 読み取り失敗フラグを立てたクリップボード
    #[must_use]
    pub fn fail_on_read(mut self) -> Self {
        self.fail_on_read = true;
        self
    }

    /// 次の `set_text` を失敗させる
    ///
    /// # Arguments
    /// * `self` - 設定対象のインメモリクリップボード
    ///
    /// # Returns
    /// * `Self` - 書き込み失敗フラグを立てたクリップボード
    #[must_use]
    pub fn fail_on_write(mut self) -> Self {
        self.fail_on_write = true;
        self
    }

    /// パスワードマネージャー等の監視除外指定付きとして扱う
    ///
    /// # Arguments
    /// * `self` - 設定対象のインメモリクリップボード
    ///
    /// # Returns
    /// * `Self` - 監視除外フラグを立てたクリップボード
    #[must_use]
    pub fn excluded_from_monitoring(mut self) -> Self {
        self.excluded_from_monitoring = true;
        self
    }

    /// 次の `get_text` の直後に別アプリがコピーしたものとして内容を差し替える
    ///
    /// 本文の読み取りと書き戻しの間に入るコピー (パスワードマネージャー等) を再現する。
    /// 差し替え時に変更トークンが進む
    ///
    /// # Arguments
    /// * `self` - 設定対象のインメモリクリップボード
    /// * `text` - 差し替え後のテキスト
    /// * `excluded_from_monitoring` - 差し替え後の内容が監視除外指定付きの場合は `true`
    ///
    /// # Returns
    /// * `Self` - 割り込みコピーを予約したクリップボード
    #[must_use]
    pub fn with_external_copy_after_read(
        mut self,
        text: impl Into<String>,
        excluded_from_monitoring: bool,
    ) -> Self {
        self.external_copy_after_read = Some(ExternalCopy {
            text: text.into(),
            excluded_from_monitoring,
        });
        self
    }

    /// 保持中のテキストを返す
    ///
    /// # Arguments
    /// * `self` - 参照元のインメモリクリップボード
    ///
    /// # Returns
    /// * `&str` - 保持中のテキスト
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 保持中の HTML 断片を返す
    ///
    /// # Arguments
    /// * `self` - 参照元のインメモリクリップボード
    ///
    /// # Returns
    /// * `Option<&str>` - 保持中の HTML。HTML 形式が無い場合は `None`
    pub fn html(&self) -> Option<&str> {
        self.html.as_deref()
    }

    /// 書き込み後の画像サイズを返す
    ///
    /// # Arguments
    /// * `self` - 参照元のインメモリクリップボード
    ///
    /// # Returns
    /// * `Option<(u32, u32)>` - 書き込み済み画像の幅と高さ。未書き込みの場合は `None`
    pub fn written_image_size(&self) -> Option<(u32, u32)> {
        self.written_image
            .as_ref()
            .map(|(width, height, _)| (*width, *height))
    }
}

impl TextClipboard for InMemoryTextClipboard {
    fn get_text(&mut self) -> Result<String, ClipboardIoError> {
        if self.fail_on_read {
            return Err(ClipboardIoError::ReadFailed("read failed".to_string()));
        }
        let text = self.text.clone();
        if let Some(copy) = self.external_copy_after_read.take() {
            self.text = copy.text;
            self.html = None;
            self.excluded_from_monitoring = copy.excluded_from_monitoring;
            self.change_token += 1;
        }
        Ok(text)
    }

    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        if self.fail_on_write {
            return Err(ClipboardIoError::WriteFailed("write failed".to_string()));
        }
        // OS クリップボードと同様に、テキストの書き込みで他の形式を消す
        self.text = text;
        self.html = None;
        self.change_token += 1;
        Ok(())
    }

    fn is_excluded_from_monitoring(&mut self) -> bool {
        self.excluded_from_monitoring
    }

    fn change_token(&mut self) -> Option<u64> {
        Some(self.change_token)
    }
}

impl ImageClipboard for InMemoryTextClipboard {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), ClipboardIoError> {
        self.source_image
            .clone()
            .ok_or(ClipboardIoError::ImageUnavailable)
    }

    fn set_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        self.written_image = Some((width, height, rgba));
        self.change_token += 1;
        Ok(())
    }
}

impl HtmlClipboard for InMemoryTextClipboard {
    fn get_html(&mut self) -> Result<Option<String>, ClipboardIoError> {
        Ok(self.html.clone())
    }

    fn set_html(&mut self, html: String, alt_text: String) -> Result<(), ClipboardIoError> {
        if self.fail_on_write {
            return Err(ClipboardIoError::WriteFailed("write failed".to_string()));
        }
        self.text = alt_text;
        self.html = Some(html);
        self.change_token += 1;
        Ok(())
    }
}
