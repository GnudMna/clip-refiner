use super::dispatch::Refiner;
use super::mode::RefineMode;
use super::text_clipboard::TextClipboard;

use crate::security::is_within_clipboard_limit;

// ======================================================================
// クリップボード処理
// ======================================================================
/// クリップボード加工の成功結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardProcessOutcome {
    /// 加工してクリップボードへ書き戻した
    Processed(String),
    /// テキストに変更がなかった
    Unchanged,
}

/// クリップボード加工の失敗理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardProcessError {
    /// クリップボードが空、またはテキスト形式ではない
    NoText,
    /// クリップボード本文が処理上限を超えている
    TextTooLarge,
    /// クリップボードの読み取りに失敗
    ReadFailed(String),
    /// クリップボードへの書き込みに失敗
    WriteFailed(String),
}

impl ClipboardProcessError {
    /// ユーザー向けのエラーメッセージを返す
    pub fn user_message(&self) -> &str {
        match self {
            Self::NoText => "クリップボードにテキストがありません",
            Self::TextTooLarge => "クリップボードのテキストが大きすぎます",
            Self::ReadFailed(_) => "クリップボードの読み取りに失敗しました",
            Self::WriteFailed(_) => "クリップボードへの書き込みに失敗しました",
        }
    }
}

/// テキストに加工モードを適用する
///
/// クリップボード I/O は行わない
///
/// # Arguments
/// * `text` - 加工前のテキスト
/// * `mode` - 適用する加工モード (`RefineMode`)
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工結果がある
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError::NoText)` - テキストが空
pub(crate) fn apply_refinement_to_text(
    text: &str,
    mode: RefineMode,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    if text.is_empty() {
        return Err(ClipboardProcessError::NoText);
    }
    if !is_within_clipboard_limit(text) {
        return Err(ClipboardProcessError::TextTooLarge);
    }

    let refined = mode.refine(text);

    if refined == text {
        Ok(ClipboardProcessOutcome::Unchanged)
    } else {
        Ok(ClipboardProcessOutcome::Processed(refined.into_owned()))
    }
}

/// クリップボードのテキストを取得し、指定されたモードで加工して書き戻す
///
/// テキストが変更された場合のみクリップボードを更新する
///
/// # Arguments
/// * `clipboard` - `arboard::Clipboard` のミュータブルなインスタンス
/// * `mode` - 適用する加工モード (`RefineMode`)
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
pub fn process_clipboard(
    clipboard: &mut arboard::Clipboard,
    mode: RefineMode,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_text_clipboard(clipboard, mode)
}

/// テキストクリップボード実装に対して加工を適用する
///
/// # Arguments
/// * `clipboard` - テキストクリップボード実装
/// * `mode` - 適用する加工モード (`RefineMode`)
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
pub(crate) fn process_text_clipboard<C: TextClipboard>(
    clipboard: &mut C,
    mode: RefineMode,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    let text = clipboard
        .get_text()
        .map_err(ClipboardProcessError::ReadFailed)?;

    match apply_refinement_to_text(&text, mode)? {
        ClipboardProcessOutcome::Unchanged => Ok(ClipboardProcessOutcome::Unchanged),
        ClipboardProcessOutcome::Processed(result) => {
            clipboard
                .set_text(result.clone())
                .map_err(ClipboardProcessError::WriteFailed)?;
            Ok(ClipboardProcessOutcome::Processed(result))
        }
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::InMemoryTextClipboard;

    /// `ClipboardProcessError` がユーザー向けメッセージを返すこと
    #[test]
    fn test_clipboard_process_error_user_message() {
        assert_eq!(
            ClipboardProcessError::NoText.user_message(),
            "クリップボードにテキストがありません"
        );
        assert_eq!(
            ClipboardProcessError::TextTooLarge.user_message(),
            "クリップボードのテキストが大きすぎます"
        );
        assert_eq!(
            ClipboardProcessError::ReadFailed("detail".to_string()).user_message(),
            "クリップボードの読み取りに失敗しました"
        );
        assert_eq!(
            ClipboardProcessError::WriteFailed("detail".to_string()).user_message(),
            "クリップボードへの書き込みに失敗しました"
        );
    }

    /// 空文字列は `NoText` エラーになること
    #[test]
    fn apply_refinement_to_text_rejects_empty() {
        assert_eq!(
            apply_refinement_to_text("", RefineMode::Trim),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// 上限超過は `TextTooLarge` エラーになること
    #[test]
    fn apply_refinement_to_text_rejects_oversized() {
        let oversized = "a".repeat(crate::consts::MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert_eq!(
            apply_refinement_to_text(&oversized, RefineMode::Trim),
            Err(ClipboardProcessError::TextTooLarge)
        );
    }

    /// 変更がある場合は `Processed` を返すこと
    #[test]
    fn apply_refinement_to_text_returns_processed() {
        assert_eq!(
            apply_refinement_to_text("  hello  ", RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
    }

    /// 変更がない場合は `Unchanged` を返すこと
    #[test]
    fn apply_refinement_to_text_returns_unchanged() {
        assert_eq!(
            apply_refinement_to_text("hello", RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }

    /// 加工してクリップボードへ書き戻すこと
    #[test]
    fn process_clipboard_trims_and_writes_back() {
        let mut cb = InMemoryTextClipboard::with_text("  hello  ");

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 変更がない場合はクリップボードを更新しないこと
    #[test]
    fn process_clipboard_leaves_unchanged_text() {
        let mut cb = InMemoryTextClipboard::with_text("hello");

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 読み取り失敗時は `ReadFailed` を返すこと
    #[test]
    fn process_clipboard_read_failure() {
        let mut cb = InMemoryTextClipboard::with_text("x").fail_on_read();

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim),
            Err(ClipboardProcessError::ReadFailed("read failed".to_string()))
        );
    }

    /// 書き込み失敗時は `WriteFailed` を返し、元の内容を維持すること
    #[test]
    fn process_clipboard_write_failure() {
        let mut cb = InMemoryTextClipboard::with_text("  x  ").fail_on_write();

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim),
            Err(ClipboardProcessError::WriteFailed(
                "write failed".to_string()
            ))
        );
        assert_eq!(cb.text(), "  x  ");
    }
}
