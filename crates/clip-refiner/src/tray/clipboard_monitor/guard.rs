use crate::refiner::{ClipboardIoError, HtmlClipboard, ImageClipboard, TextClipboard};

// ======================================================================
// 読み取り済み本文の固定
// ======================================================================
/// 監視で 1 回読んだ本文を加工に引き継ぎ、書き戻し前に別のコピーを検知するクリップボード
///
/// - `get_text` は監視が読み取り済みの本文を 1 度だけ返す。除外判定・サイズ上限・重複判定・
///   履歴・Undo 元と、実際に加工する本文を同じ値にそろえる
/// - 書き込みは、観測を始めた時点から変更トークンが進んでいない場合のみ行う。
///   途中で別のコピー (パスワードマネージャー等) が入った内容は加工も上書きもしない
///
/// 変更トークンを取得できない環境では、書き込みの確認は行わない
pub(crate) struct ObservedClipboard<'a, C> {
    inner: &'a mut C,
    observed_text: Option<&'a str>,
    expected_token: Option<u64>,
    superseded: bool,
}

impl<'a, C: TextClipboard> ObservedClipboard<'a, C> {
    /// 読み取り済みの本文と観測開始時の変更トークンで包む
    ///
    /// # Arguments
    /// * `inner` - 実際のクリップボード
    /// * `observed_text` - 監視が読み取り済みの本文
    /// * `token` - 観測を始める前に取得した変更トークン
    ///
    /// # Returns
    /// * `Self` - 読み取り済み本文を固定したクリップボード
    pub(crate) fn new(inner: &'a mut C, observed_text: &'a str, token: Option<u64>) -> Self {
        Self::with_observed(inner, Some(observed_text), token)
    }

    /// 読み取りに失敗しうる本文と観測開始時の変更トークンで包む
    ///
    /// 本文が `None` のとき `get_text` は実際のクリップボードを読む。書き込みの照合は `new` と同じ
    ///
    /// # Arguments
    /// * `inner` - 実際のクリップボード
    /// * `observed_text` - 読み取り済みの本文 (読めなかった場合は `None`)
    /// * `token` - 観測を始める前に取得した変更トークン
    ///
    /// # Returns
    /// * `Self` - 読み取り済み本文を固定したクリップボード
    pub(crate) fn with_observed(
        inner: &'a mut C,
        observed_text: Option<&'a str>,
        token: Option<u64>,
    ) -> Self {
        Self {
            inner,
            observed_text,
            expected_token: token,
            superseded: false,
        }
    }

    /// 観測の途中で別のコピーが入り、書き込みを見送ったか判定する
    ///
    /// # Arguments
    /// * `self` - 判定対象
    ///
    /// # Returns
    /// * `bool` - 書き込みを見送った場合は `true`
    pub(crate) fn is_superseded(&self) -> bool {
        self.superseded
    }

    /// 書き込み直前に変更トークンを照合する
    fn ensure_unchanged(&mut self) -> Result<(), ClipboardIoError> {
        let Some(expected) = self.expected_token else {
            return Ok(());
        };
        if self.inner.change_token() == Some(expected) {
            return Ok(());
        }
        self.superseded = true;
        Err(ClipboardIoError::WriteFailed(
            "加工中にクリップボードが更新された".to_string(),
        ))
    }

    /// 自身の書き込みで進んだトークンを期待値へ取り込む
    ///
    /// 読み取り済み本文は書き込み後の内容と一致しないため破棄する
    fn accept_own_write(&mut self) {
        self.observed_text = None;
        if self.expected_token.is_some() {
            self.expected_token = self.inner.change_token();
        }
    }
}

impl<C: TextClipboard> TextClipboard for ObservedClipboard<'_, C> {
    fn get_text(&mut self) -> Result<String, ClipboardIoError> {
        match self.observed_text.take() {
            Some(text) => Ok(text.to_string()),
            None => self.inner.get_text(),
        }
    }

    fn set_text(&mut self, text: String) -> Result<(), ClipboardIoError> {
        self.ensure_unchanged()?;
        self.inner.set_text(text)?;
        self.accept_own_write();
        Ok(())
    }

    fn is_excluded_from_monitoring(&mut self) -> bool {
        self.inner.is_excluded_from_monitoring()
    }

    fn change_token(&mut self) -> Option<u64> {
        self.inner.change_token()
    }
}

impl<C: TextClipboard + ImageClipboard> ImageClipboard for ObservedClipboard<'_, C> {
    fn get_image(&mut self) -> Result<(u32, u32, Vec<u8>), ClipboardIoError> {
        self.inner.get_image()
    }

    fn set_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        self.ensure_unchanged()?;
        self.inner.set_image(width, height, rgba)?;
        self.accept_own_write();
        Ok(())
    }

    fn set_image_as_file(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<(), ClipboardIoError> {
        self.ensure_unchanged()?;
        self.inner.set_image_as_file(width, height, rgba)?;
        self.accept_own_write();
        Ok(())
    }
}

impl<C: TextClipboard + HtmlClipboard> HtmlClipboard for ObservedClipboard<'_, C> {
    fn get_html(&mut self) -> Result<Option<String>, ClipboardIoError> {
        self.inner.get_html()
    }

    fn set_html(&mut self, html: String, alt_text: String) -> Result<(), ClipboardIoError> {
        self.ensure_unchanged()?;
        self.inner.set_html(html, alt_text)?;
        self.accept_own_write();
        Ok(())
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::InMemoryTextClipboard;

    /// 読み取り済みの本文を 1 度だけ返し、以降は実際のクリップボードを読むこと
    #[test]
    fn get_text_returns_observed_text_once() {
        let mut inner = InMemoryTextClipboard::with_text("actual");
        let token = inner.change_token();
        let mut guarded = ObservedClipboard::new(&mut inner, "observed", token);

        assert_eq!(guarded.get_text().as_deref(), Ok("observed"));
        assert_eq!(guarded.get_text().as_deref(), Ok("actual"));
    }

    /// 変更トークンが進んでいなければ書き込めること
    #[test]
    fn set_text_writes_when_token_unchanged() {
        let mut inner = InMemoryTextClipboard::with_text("a");
        let token = inner.change_token();
        let mut guarded = ObservedClipboard::new(&mut inner, "a", token);

        assert!(guarded.set_text("b".to_string()).is_ok());
        assert!(!guarded.is_superseded());
        assert_eq!(inner.text(), "b");
    }

    /// 自身の書き込みでトークンが進んでも、続く書き込みを見送らないこと
    #[test]
    fn consecutive_own_writes_are_allowed() {
        let mut inner = InMemoryTextClipboard::with_text("a");
        let token = inner.change_token();
        let mut guarded = ObservedClipboard::new(&mut inner, "a", token);

        assert!(guarded.set_text("b".to_string()).is_ok());
        assert!(guarded.set_image(1, 1, vec![0; 4]).is_ok());
        assert!(!guarded.is_superseded());
    }

    /// 別のコピーでトークンが進んでいれば書き込みを見送ること
    #[test]
    fn writes_are_rejected_when_token_advanced() {
        let mut inner = InMemoryTextClipboard::with_text("a");
        let token = inner.change_token();
        inner.set_text("external".to_string()).unwrap();
        let mut guarded = ObservedClipboard::new(&mut inner, "a", token);

        assert!(guarded.set_text("b".to_string()).is_err());
        assert!(
            guarded
                .set_html("<p>b</p>".to_string(), "b".to_string())
                .is_err()
        );
        assert!(guarded.set_image(1, 1, vec![0; 4]).is_err());
        assert!(guarded.is_superseded());
        assert_eq!(inner.text(), "external");
    }

    /// 変更トークンを取得できない場合は書き込みを妨げないこと
    #[test]
    fn writes_are_allowed_without_token() {
        let mut inner = InMemoryTextClipboard::with_text("a");
        inner.set_text("external".to_string()).unwrap();
        let mut guarded = ObservedClipboard::new(&mut inner, "a", None);

        assert!(guarded.set_text("b".to_string()).is_ok());
        assert!(!guarded.is_superseded());
    }
}
