use arboard::Clipboard;

use super::dispatch::Refiner;
use super::mode::RefineMode;

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
    clipboard: &mut Clipboard,
    mode: RefineMode,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    let text = clipboard
        .get_text()
        .map_err(|e| ClipboardProcessError::ReadFailed(e.to_string()))?;

    match apply_refinement_to_text(&text, mode)? {
        ClipboardProcessOutcome::Unchanged => Ok(ClipboardProcessOutcome::Unchanged),
        ClipboardProcessOutcome::Processed(result) => {
            clipboard
                .set_text(result.clone())
                .map_err(|e| ClipboardProcessError::WriteFailed(e.to_string()))?;
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
    use arboard::Clipboard;

    /// `ClipboardProcessError` がユーザー向けメッセージを返すこと
    #[test]
    fn test_clipboard_process_error_user_message() {
        assert_eq!(
            ClipboardProcessError::NoText.user_message(),
            "クリップボードにテキストがありません"
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

    /// クリップボード処理の統合テスト
    ///
    /// システムクリップボードへのアクセスが必要なため、通常の `cargo test` では除外される
    /// 手動実行: `cargo test test_process_clipboard_integration -- --ignored`
    #[test]
    #[ignore = "システムクリップボードへのアクセスが必要"]
    fn test_process_clipboard_integration() {
        let mut cb = Clipboard::new().expect("クリップボードの初期化に失敗");

        let unique_str_1 = "  clip_refiner_test_1  ";
        cb.set_text(unique_str_1.to_string())
            .expect("クリップボードへの書き込みに失敗");
        assert_eq!(
            cb.get_text().expect("クリップボードの読み取りに失敗"),
            unique_str_1
        );
        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Processed(
                "clip_refiner_test_1".to_string()
            ))
        );

        let unique_str_2 = "clip_refiner_test_2";
        cb.set_text(unique_str_2.to_string())
            .expect("クリップボードへの書き込みに失敗");
        assert_eq!(
            cb.get_text().expect("クリップボードの読み取りに失敗"),
            unique_str_2
        );
        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }
}
