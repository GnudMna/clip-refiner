use std::time::Duration;

use super::context::RefineContext;
use super::mode::RefineMode;
use super::pipeline::{apply_text_pipeline, split_pipeline};
use super::text_clipboard::{ImageClipboard, TextClipboard};

use crate::security::{is_within_clipboard_limit, is_within_parser_limit};

/// Excel コピー時に描画ビットマップの到着を待つ最大試行回数
#[cfg(test)]
const EXCEL_IMAGE_RETRY_ATTEMPTS: u32 = 1;
#[cfg(not(test))]
const EXCEL_IMAGE_RETRY_ATTEMPTS: u32 = 12;

/// 描画ビットマップ待機のリトライ間隔
const EXCEL_IMAGE_RETRY_INTERVAL: Duration = Duration::from_millis(50);

// ======================================================================
// クリップボード処理
// ======================================================================
/// クリップボード加工の成功結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardProcessOutcome {
    /// 加工してクリップボードへ書き戻した
    Processed(String),
    /// 加工して画像をクリップボードへ書き込んだ
    ImageProcessed { width: u32, height: u32 },
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
    /// クリップボードに Excel の描画画像がない
    NoImage,
}

impl ClipboardProcessError {
    /// ユーザー向けのエラーメッセージを返す
    pub fn user_message(&self) -> &str {
        match self {
            Self::NoText => "クリップボードにテキストがありません",
            Self::TextTooLarge => "クリップボードのテキストが大きすぎます",
            Self::ReadFailed(_) => "クリップボードの読み取りに失敗しました",
            Self::WriteFailed(_) => "クリップボードへの書き込みに失敗しました",
            Self::NoImage => "クリップボードに Excel の描画画像がありません",
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
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工結果がある
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError::NoText)` - テキストが空
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn apply_refinement_to_text(
    text: &str,
    mode: RefineMode,
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    apply_refinement_pipeline_to_text(text, &[mode], ctx)
}

/// テキストに加工パイプラインを順に適用する
///
/// クリップボード I/O は行わない
///
/// # Arguments
/// * `text` - 加工前のテキスト
/// * `pipeline` - 適用する加工モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工結果がある
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError::NoText)` - テキストが空
pub(crate) fn apply_refinement_pipeline_to_text(
    text: &str,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    if text.is_empty() {
        return Err(ClipboardProcessError::NoText);
    }
    if !is_within_clipboard_limit(text) {
        return Err(ClipboardProcessError::TextTooLarge);
    }

    let (text_modes, image_tail) = split_pipeline(pipeline);
    if text_modes.is_empty() && image_tail.is_some() {
        return Ok(ClipboardProcessOutcome::Unchanged);
    }

    match apply_text_pipeline(text, &text_modes, ctx) {
        Some(result) => Ok(ClipboardProcessOutcome::Processed(result)),
        None => Ok(ClipboardProcessOutcome::Unchanged),
    }
}

/// クリップボードのテキストを取得し、指定されたモードで加工して書き戻す
///
/// テキストが変更された場合のみクリップボードを更新する
///
/// # Arguments
/// * `clipboard` - `arboard::Clipboard` のミュータブルなインスタンス
/// * `mode` - 適用する加工モード (`RefineMode`)
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
pub fn process_clipboard(
    clipboard: &mut arboard::Clipboard,
    mode: RefineMode,
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_clipboard_pipeline(clipboard, &[mode], ctx)
}

/// クリップボードのテキストを取得し、指定されたパイプラインで加工して書き戻す
///
/// テキストが変更された場合のみクリップボードを更新する
///
/// # Arguments
/// * `clipboard` - `arboard::Clipboard` のミュータブルなインスタンス
/// * `pipeline` - 適用する加工モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
pub fn process_clipboard_pipeline(
    clipboard: &mut arboard::Clipboard,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_clipboard_pipeline_io(clipboard, pipeline, ctx)
}

/// テキストクリップボード実装に対して加工を適用する
///
/// # Arguments
/// * `clipboard` - テキストクリップボード実装
/// * `mode` - 適用する加工モード (`RefineMode`)
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn process_text_clipboard<C: TextClipboard>(
    clipboard: &mut C,
    mode: RefineMode,
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_text_clipboard_pipeline(clipboard, &[mode], ctx)
}

/// テキストクリップボード実装に対して加工パイプラインを適用する
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn process_text_clipboard_pipeline<C: TextClipboard>(
    clipboard: &mut C,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    let text = clipboard
        .get_text()
        .map_err(ClipboardProcessError::ReadFailed)?;

    match apply_refinement_pipeline_to_text(&text, pipeline, ctx)? {
        ClipboardProcessOutcome::Processed(result) => {
            clipboard
                .set_text(result.clone())
                .map_err(ClipboardProcessError::WriteFailed)?;
            Ok(ClipboardProcessOutcome::Processed(result))
        }
        ClipboardProcessOutcome::Unchanged | ClipboardProcessOutcome::ImageProcessed { .. } => {
            Ok(ClipboardProcessOutcome::Unchanged)
        }
    }
}

/// クリップボード上の Excel 描画ビットマップを画像として書き込む
pub(crate) fn process_image_clipboard<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    mode: RefineMode,
    _ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    debug_assert!(mode.produces_image());

    // Excel コピーは Unicode テキストより描画ビットマップが先に載ることがある
    let text = clipboard.get_text().unwrap_or_default();

    if !text.is_empty() && !is_within_clipboard_limit(&text) {
        return Err(ClipboardProcessError::TextTooLarge);
    }

    let expect_excel_bitmap = text.is_empty() || is_excel_tsv(&text);
    let (width, height, rgba) = match wait_for_clipboard_image(clipboard, expect_excel_bitmap) {
        Ok(image) => image,
        Err(()) if text.is_empty() => return Err(ClipboardProcessError::NoText),
        Err(()) if is_excel_tsv(&text) => return Err(ClipboardProcessError::NoImage),
        Err(()) => return Ok(ClipboardProcessOutcome::Unchanged),
    };

    clipboard
        .set_image(width, height, rgba)
        .map_err(ClipboardProcessError::WriteFailed)?;

    Ok(ClipboardProcessOutcome::ImageProcessed { width, height })
}

/// クリップボードから画像を取得する
///
/// Excel コピーは Unicode テキストより `CF_DIB` の到着が遅れることがあるため、
/// TSV 形式の場合は短い間隔でリトライする
fn wait_for_clipboard_image<C: ImageClipboard>(
    clipboard: &mut C,
    excel_tsv: bool,
) -> Result<(u32, u32, Vec<u8>), ()> {
    let attempts = if excel_tsv {
        EXCEL_IMAGE_RETRY_ATTEMPTS
    } else {
        1
    };

    for attempt in 0..attempts {
        if attempt > 0 {
            std::thread::sleep(EXCEL_IMAGE_RETRY_INTERVAL);
        }
        if let Ok(image) = clipboard.get_image() {
            return Ok(image);
        }
    }

    Err(())
}

/// Excel(TSV) としてパース可能か判定する
fn is_excel_tsv(text: &str) -> bool {
    if text.is_empty() || !is_within_parser_limit(text) {
        return false;
    }

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    reader.records().any(|r| r.is_ok())
}

/// テキスト・画像クリップボード実装に対して加工を適用する
#[expect(
    dead_code,
    reason = "単一モード用ラッパー。テストおよび後方互換のため維持"
)]
pub(crate) fn process_clipboard_io<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    mode: RefineMode,
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_clipboard_pipeline_io(clipboard, &[mode], ctx)
}

/// テキスト・画像クリップボード実装に対して加工パイプラインを適用する
pub(crate) fn process_clipboard_pipeline_io<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    if pipeline.is_empty() {
        return Err(ClipboardProcessError::NoText);
    }

    let (text_modes, image_tail) = split_pipeline(pipeline);

    if text_modes.is_empty() {
        return process_image_clipboard(clipboard, image_tail.expect("画像モードが存在する"), ctx);
    }

    let text = clipboard
        .get_text()
        .map_err(ClipboardProcessError::ReadFailed)?;

    let text_outcome = apply_refinement_pipeline_to_text(&text, pipeline, ctx)?;

    match text_outcome {
        ClipboardProcessOutcome::Processed(result) => {
            clipboard
                .set_text(result.clone())
                .map_err(ClipboardProcessError::WriteFailed)?;
            if let Some(image_mode) = image_tail {
                return process_image_clipboard(clipboard, image_mode, ctx);
            }
            Ok(ClipboardProcessOutcome::Processed(result))
        }
        ClipboardProcessOutcome::Unchanged => {
            if let Some(image_mode) = image_tail {
                process_image_clipboard(clipboard, image_mode, ctx)
            } else {
                Ok(ClipboardProcessOutcome::Unchanged)
            }
        }
        ClipboardProcessOutcome::ImageProcessed { .. } => Ok(ClipboardProcessOutcome::Unchanged),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::InMemoryTextClipboard;

    fn empty_ctx() -> RefineContext {
        RefineContext::default()
    }

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
        assert_eq!(
            ClipboardProcessError::NoImage.user_message(),
            "クリップボードに Excel の描画画像がありません"
        );
    }

    /// 空文字列は `NoText` エラーになること
    #[test]
    fn apply_refinement_to_text_rejects_empty() {
        assert_eq!(
            apply_refinement_to_text("", RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// 上限超過は `TextTooLarge` エラーになること
    #[test]
    fn apply_refinement_to_text_rejects_oversized() {
        let oversized = "a".repeat(crate::consts::MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert_eq!(
            apply_refinement_to_text(&oversized, RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::TextTooLarge)
        );
    }

    /// 変更がある場合は `Processed` を返すこと
    #[test]
    fn apply_refinement_to_text_returns_processed() {
        assert_eq!(
            apply_refinement_to_text("  hello  ", RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
    }

    /// 変更がない場合は `Unchanged` を返すこと
    #[test]
    fn apply_refinement_to_text_returns_unchanged() {
        assert_eq!(
            apply_refinement_to_text("hello", RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }

    /// 加工パイプラインが順に適用されること
    #[test]
    fn apply_refinement_pipeline_to_text_chains_modes() {
        let input = "  %E3%81%82  ";
        assert_eq!(
            apply_refinement_pipeline_to_text(
                input,
                &[RefineMode::UrlDecode, RefineMode::Trim],
                &empty_ctx(),
            ),
            Ok(ClipboardProcessOutcome::Processed("あ".to_string()))
        );
    }

    /// 加工パイプラインで変更がない場合は `Unchanged` を返すこと
    #[test]
    fn apply_refinement_pipeline_to_text_returns_unchanged() {
        assert_eq!(
            apply_refinement_pipeline_to_text("hello", &[RefineMode::Trim], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }

    /// 加工してクリップボードへ書き戻すこと
    #[test]
    fn process_clipboard_trims_and_writes_back() {
        let mut cb = InMemoryTextClipboard::with_text("  hello  ");

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 変更がない場合はクリップボードを更新しないこと
    #[test]
    fn process_clipboard_leaves_unchanged_text() {
        let mut cb = InMemoryTextClipboard::with_text("hello");

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 読み取り失敗時は `ReadFailed` を返すこと
    #[test]
    fn process_clipboard_read_failure() {
        let mut cb = InMemoryTextClipboard::with_text("x").fail_on_read();

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::ReadFailed("read failed".to_string()))
        );
    }

    /// 書き込み失敗時は `WriteFailed` を返し、元の内容を維持すること
    #[test]
    fn process_clipboard_write_failure() {
        let mut cb = InMemoryTextClipboard::with_text("  x  ").fail_on_write();

        assert_eq!(
            process_text_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::WriteFailed(
                "write failed".to_string()
            ))
        );
        assert_eq!(cb.text(), "  x  ");
    }

    /// TSV 形式だが描画画像がない場合は `NoImage` を返すこと
    #[test]
    fn process_image_clipboard_returns_no_image_without_bitmap() {
        let mut cb = InMemoryTextClipboard::with_text("A\tB\n1\t2");

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Err(ClipboardProcessError::NoImage)
        );
    }

    /// 描画画像がある場合はクリップボードへ書き込むこと
    #[test]
    fn process_image_clipboard_writes_excel_bitmap() {
        let rgba = vec![255_u8; 4 * 2 * 2];
        let mut cb = InMemoryTextClipboard::with_text("A\tB\n1\t2").with_source_image(2, 2, rgba);

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Ok(ClipboardProcessOutcome::ImageProcessed {
                width: 2,
                height: 2
            })
        );
        assert_eq!(cb.written_image_size(), Some((2, 2)));
    }

    /// Unicode テキストが無くても描画画像があれば書き込むこと
    #[test]
    fn process_image_clipboard_succeeds_when_text_unavailable() {
        let rgba = vec![255_u8; 4 * 2 * 2];
        let mut cb = InMemoryTextClipboard::with_text("unused")
            .fail_on_read()
            .with_source_image(2, 2, rgba);

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Ok(ClipboardProcessOutcome::ImageProcessed {
                width: 2,
                height: 2
            })
        );
        assert_eq!(cb.written_image_size(), Some((2, 2)));
    }

    /// Unicode テキストも描画画像も無い場合は `NoText` を返すこと
    #[test]
    fn process_image_clipboard_returns_no_text_without_bitmap() {
        let mut cb = InMemoryTextClipboard::with_text("unused").fail_on_read();

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// Excel TSV 判定の境界値
    #[test]
    fn is_excel_tsv_detects_tabular_and_single_cell() {
        assert!(!is_excel_tsv(""));
        assert!(is_excel_tsv("A\tB\n1\t2"));
        assert!(is_excel_tsv("hello"));
    }
}
