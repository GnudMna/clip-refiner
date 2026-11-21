use std::time::Duration;

use super::base64_clipboard::{write_base64_image, write_image_as_base64};
use super::context::{RefineContext, RefineSkipReason};
use super::mode::RefineMode;
use super::pipeline::{apply_text_pipeline, split_pipeline};
use super::text_clipboard::{ClipboardIoError, HtmlClipboard, ImageClipboard, TextClipboard};
use super::transform::markdown::markdown_to_rich_text_html;
use crate::security::{is_within_clipboard_limit, is_within_parser_limit};

// ======================================================================
// 定数
// ======================================================================
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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClipboardProcessError {
    /// クリップボードが空、またはテキスト形式ではない
    #[error("クリップボードにテキストがありません")]
    NoText,
    /// クリップボード本文が処理上限を超えている
    #[error("クリップボードのテキストが処理上限を超えています")]
    TextTooLarge,
    /// 正規表現マッチが制限時間を超過した
    #[error("正規表現の実行が制限時間を超えました")]
    RegexTimedOut,
    /// JSON / YAML / Markdown パーサー入力が上限を超えている
    #[error("テキストがパーサー上限を超えています")]
    ParserInputTooLarge,
    /// HTML の要素の入れ子が上限を超えている
    #[error("HTML の入れ子が上限を超えています")]
    HtmlNestingTooDeep,
    /// クリップボードの読み取りに失敗
    #[error("クリップボードの読み取りに失敗しました")]
    ReadFailed(String),
    /// クリップボードへの書き込みに失敗
    #[error("クリップボードへの書き込みに失敗しました")]
    WriteFailed(String),
    /// クリップボードに Excel の描画画像がない
    #[error("クリップボードに Excel の描画画像がありません")]
    NoImage,
    /// Base64 と相互変換する画像が辺長またはピクセル数の上限を超えている
    #[error("画像が処理上限を超えています")]
    ImageTooLarge,
}

impl ClipboardProcessError {
    /// ユーザー向けのエラーメッセージを返す
    ///
    /// # Arguments
    /// * `self` - この `ClipboardProcessError`
    ///
    /// # Returns
    /// * `&str` - 各エラー種別に対応する表示用文字列
    #[must_use]
    pub fn user_message(&self) -> &str {
        match self {
            Self::NoText => "クリップボードにテキストがありません",
            Self::TextTooLarge => "テキストが処理上限 (2 MiB) を超えているため加工しませんでした",
            Self::RegexTimedOut => {
                "正規表現の実行が制限時間 (2 秒) を超えたため、元のテキストを維持しました"
            }
            Self::ParserInputTooLarge => {
                "テキストがパーサー上限 (1 MiB) を超えているため加工しませんでした"
            }
            Self::HtmlNestingTooDeep => {
                "HTML の入れ子が上限 (256 階層) を超えているため加工しませんでした"
            }
            Self::ReadFailed(_) => "クリップボードの読み取りに失敗しました",
            Self::WriteFailed(_) => "クリップボードへの書き込みに失敗しました",
            Self::NoImage => "クリップボードに Excel の描画画像がありません",
            Self::ImageTooLarge => {
                "画像が処理上限 (8192 px / 4096×4096 ピクセル) を超えているため変換しませんでした"
            }
        }
    }

    /// 上限超過・タイムアウトなど、加工を意図的に打ち切ったスキップか判定する
    ///
    /// # Arguments
    /// * `self` - この `ClipboardProcessError`
    ///
    /// # Returns
    /// * `bool` - スキップ理由なら `true`
    #[must_use]
    pub fn is_skip(&self) -> bool {
        matches!(
            self,
            Self::TextTooLarge
                | Self::RegexTimedOut
                | Self::ParserInputTooLarge
                | Self::HtmlNestingTooDeep
                | Self::ImageTooLarge
        )
    }
}

impl From<ClipboardIoError> for ClipboardProcessError {
    fn from(err: ClipboardIoError) -> Self {
        match err {
            ClipboardIoError::ReadFailed(detail) => Self::ReadFailed(detail),
            ClipboardIoError::TextUnavailable => Self::NoText,
            ClipboardIoError::WriteFailed(detail) => Self::WriteFailed(detail),
            ClipboardIoError::ImageUnavailable => Self::NoImage,
        }
    }
}

/// テキストに加工パイプラインを順に適用する
///
/// クリップボード I/O は行わない。入力サイズ上限を超える場合は `TextTooLarge` を返す
///
/// # Arguments
/// * `text` - 加工前のテキスト
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工結果がある
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError::NoText)` - テキストが空
///
/// # Errors
/// テキストが空、サイズ上限を超えている場合、または変更がなく正規表現のタイムアウト・パーサー入力上限超過・HTML の入れ子上限超過で打ち切られた場合
pub fn apply_pipeline_to_text(
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

    ctx.clear_skip_reason();
    match apply_text_pipeline(text, &text_modes, ctx) {
        Some(result) => {
            // 前段で変更済みなら後段のスキップは部分成功として扱う
            let _ = ctx.take_skip_reason();
            Ok(ClipboardProcessOutcome::Processed(result))
        }
        None => match ctx.take_skip_reason() {
            Some(RefineSkipReason::RegexTimedOut) => Err(ClipboardProcessError::RegexTimedOut),
            Some(RefineSkipReason::ParserInputTooLarge) => {
                Err(ClipboardProcessError::ParserInputTooLarge)
            }
            Some(RefineSkipReason::HtmlNestingTooDeep) => {
                Err(ClipboardProcessError::HtmlNestingTooDeep)
            }
            None => Ok(ClipboardProcessOutcome::Unchanged),
        },
    }
}

/// クリップボードのテキストを取得し、指定されたモードで加工して書き戻す
///
/// テキストが変更された場合のみクリップボードを更新する
///
/// # Arguments
/// * `clipboard` - テキスト・画像・HTML クリップボード実装 (`OwnedClipboard` など)
/// * `mode` - 適用する変換モード (`RefineMode`)
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
///
/// # Errors
/// クリップボード I/O 失敗、空テキスト、サイズ上限超過、正規表現のタイムアウト、画像取得失敗のいずれか
pub fn process_clipboard<C: TextClipboard + ImageClipboard + HtmlClipboard>(
    clipboard: &mut C,
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
/// * `clipboard` - テキスト・画像・HTML クリップボード実装 (`OwnedClipboard` など)
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
///
/// # Errors
/// クリップボード I/O 失敗、空テキスト、サイズ上限超過、正規表現のタイムアウト、画像取得失敗のいずれか
pub fn process_clipboard_pipeline<C: TextClipboard + ImageClipboard + HtmlClipboard>(
    clipboard: &mut C,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    process_clipboard_pipeline_io(clipboard, pipeline, ctx)
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

    clipboard.set_image_as_file(width, height, rgba)?;

    Ok(ClipboardProcessOutcome::ImageProcessed { width, height })
}

/// クリップボードから画像を取得する
///
/// Excel コピーは Unicode テキストより `CF_DIB` の到着が遅れることがあるため、
/// テキストが空または TSV 形式の場合 (`excel_tsv` が `true`) は短い間隔でリトライする
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

/// テキスト・画像・HTML クリップボード実装に対して加工パイプラインを適用する
///
/// 先頭が HTML 入力モードなら `CF_HTML` をテキストより優先して読む
/// テキスト加工の最終段がリッチテキスト出力モードなら、`CF_HTML` と代替テキストを書き込む
///
/// # Arguments
/// * `clipboard` - テキスト・画像・HTML クリップボード実装
/// * `pipeline` - 適用する変換モード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 加工して書き戻した
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - 変更がなかった
/// * `Ok(ClipboardProcessOutcome::ImageProcessed)` - 画像をクリップボードへ書き込んだ
/// * `Err(ClipboardProcessError)` - 読み取り・書き込み失敗、またはテキストがない
///
/// # Errors
/// パイプラインが空、クリップボード I/O 失敗、空テキスト、サイズ上限超過、正規表現のタイムアウト、画像取得失敗のいずれか
pub fn process_clipboard_pipeline_io<C: TextClipboard + ImageClipboard + HtmlClipboard>(
    clipboard: &mut C,
    pipeline: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    if pipeline.is_empty() {
        return Err(ClipboardProcessError::NoText);
    }

    let (text_modes, image_tail) = split_pipeline(pipeline);

    let Some(&first_mode) = text_modes.first() else {
        let Some(image_mode) = image_tail else {
            return Err(ClipboardProcessError::NoText);
        };
        return process_image_clipboard(clipboard, image_mode, ctx);
    };

    let html_source = if first_mode.reads_html() {
        read_html_source(clipboard)
    } else {
        None
    };
    let text_result = clipboard.get_text();

    // テキストが無く画像だけがあるときは、画像を Base64 テキストへ変換する
    if image_tail.is_none()
        && html_source.is_none()
        && text_result.as_ref().map_or(true, String::is_empty)
        && let Some(outcome) = write_image_as_base64(clipboard, &text_modes, ctx)?
    {
        return Ok(outcome);
    }

    let text = match text_result {
        Ok(text) => text,
        // HTML 形式だけのコピーでも変換できるようにする
        Err(_) if html_source.is_some() => String::new(),
        Err(e) => return Err(e.into()),
    };
    let source = html_source.as_deref().unwrap_or(&text);

    // Base64 デコードの結果が画像なら、テキストではなく画像として書き込む
    if image_tail.is_none()
        && let Some(outcome) = write_base64_image(clipboard, source, &text_modes, ctx)?
    {
        return Ok(outcome);
    }

    let text_outcome = if text_modes
        .last()
        .is_some_and(|mode| mode.produces_rich_text())
    {
        write_rich_text(clipboard, source, &text_modes, ctx)?
    } else {
        match apply_pipeline_to_text(source, &text_modes, ctx)? {
            // HTML 入力の変換結果が既存のテキストと同じなら書き戻さない
            ClipboardProcessOutcome::Processed(result) if result == text => {
                ClipboardProcessOutcome::Unchanged
            }
            ClipboardProcessOutcome::Processed(result) => {
                clipboard.set_text(result.clone())?;
                ClipboardProcessOutcome::Processed(result)
            }
            outcome => outcome,
        }
    };

    match text_outcome {
        ClipboardProcessOutcome::Processed(result) => {
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

/// HTML 入力モード向けに `CF_HTML` の断片を読む
///
/// 読み取りに失敗した場合はテキストへフォールバックさせるため `None` を返す
fn read_html_source<C: HtmlClipboard>(clipboard: &mut C) -> Option<String> {
    match clipboard.get_html() {
        Ok(html) => html.filter(|html| !html.trim().is_empty()),
        Err(e) => {
            tracing::debug!("CF_HTML の読み取りに失敗したためテキストを使用: {}", e);
            None
        }
    }
}

/// 最終段を除くテキスト加工を適用し、結果の Markdown をリッチテキストとして書き込む
///
/// `CF_UNICODETEXT` には Markdown を、`CF_HTML` には変換した HTML を置く
///
/// # Arguments
/// * `clipboard` - 書き込み先の HTML クリップボード実装
/// * `source` - 加工前のテキスト
/// * `text_modes` - 最終段がリッチテキスト出力モードのテキスト加工列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(ClipboardProcessOutcome::Processed)` - 書き込んだ代替テキスト (Markdown)
/// * `Ok(ClipboardProcessOutcome::Unchanged)` - HTML が空になり書き込まなかった
fn write_rich_text<C: HtmlClipboard>(
    clipboard: &mut C,
    source: &str,
    text_modes: &[RefineMode],
    ctx: &RefineContext,
) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
    let preceding = &text_modes[..text_modes.len() - 1];
    let markdown = match apply_pipeline_to_text(source, preceding, ctx)? {
        ClipboardProcessOutcome::Processed(result) => result,
        ClipboardProcessOutcome::Unchanged | ClipboardProcessOutcome::ImageProcessed { .. } => {
            source.to_string()
        }
    };
    if !is_within_parser_limit(&markdown) {
        return Err(ClipboardProcessError::ParserInputTooLarge);
    }

    let html = markdown_to_rich_text_html(&markdown);
    if html.is_empty() {
        return Ok(ClipboardProcessOutcome::Unchanged);
    }

    clipboard.set_html(html, markdown.clone())?;
    Ok(ClipboardProcessOutcome::Processed(markdown))
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

    /// `ClipboardProcessError` がユーザー向けメッセージを返すようにすること
    #[test]
    fn test_clipboard_process_error_user_message() {
        assert_eq!(
            ClipboardProcessError::NoText.user_message(),
            "クリップボードにテキストがありません"
        );
        assert_eq!(
            ClipboardProcessError::TextTooLarge.user_message(),
            "テキストが処理上限 (2 MiB) を超えているため加工しませんでした"
        );
        assert_eq!(
            ClipboardProcessError::RegexTimedOut.user_message(),
            "正規表現の実行が制限時間 (2 秒) を超えたため、元のテキストを維持しました"
        );
        assert_eq!(
            ClipboardProcessError::ParserInputTooLarge.user_message(),
            "テキストがパーサー上限 (1 MiB) を超えているため加工しませんでした"
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
        assert!(ClipboardProcessError::ImageTooLarge.is_skip());
        // 案内文の上限値は定数と一致させる
        assert!(
            ClipboardProcessError::ImageTooLarge
                .user_message()
                .contains(&format!("{} px", crate::consts::MAX_BASE64_IMAGE_DIMENSION))
        );
        assert!(ClipboardProcessError::TextTooLarge.is_skip());
        assert!(ClipboardProcessError::RegexTimedOut.is_skip());
        assert!(ClipboardProcessError::ParserInputTooLarge.is_skip());
        assert!(ClipboardProcessError::HtmlNestingTooDeep.is_skip());
        // 案内文の上限値は定数と一致させる
        assert!(
            ClipboardProcessError::HtmlNestingTooDeep
                .user_message()
                .contains(&format!("{} 階層", crate::consts::MAX_HTML_NESTING_DEPTH))
        );
        assert!(!ClipboardProcessError::NoText.is_skip());
    }

    /// パーサー上限超過は `ParserInputTooLarge` になるようにすること
    #[test]
    fn apply_pipeline_to_text_reports_parser_input_too_large() {
        let oversized = "a".repeat(crate::consts::MAX_PARSER_INPUT_BYTES + 1);
        assert_eq!(
            apply_pipeline_to_text(&oversized, &[RefineMode::JsonFormat], &empty_ctx()),
            Err(ClipboardProcessError::ParserInputTooLarge)
        );
    }

    /// 空文字列は `NoText` エラーになるようにすること
    #[test]
    fn apply_pipeline_to_text_rejects_empty() {
        assert_eq!(
            apply_pipeline_to_text("", &[RefineMode::Trim], &empty_ctx()),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// 上限超過は `TextTooLarge` エラーになるようにすること
    #[test]
    fn apply_pipeline_to_text_rejects_oversized() {
        let oversized = "a".repeat(crate::consts::MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert_eq!(
            apply_pipeline_to_text(&oversized, &[RefineMode::Trim], &empty_ctx()),
            Err(ClipboardProcessError::TextTooLarge)
        );
    }

    /// 変更がある場合は `Processed` を返すようにすること
    #[test]
    fn apply_pipeline_to_text_returns_processed() {
        assert_eq!(
            apply_pipeline_to_text("  hello  ", &[RefineMode::Trim], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
    }

    /// 加工パイプラインが順に適用されるようにすること
    #[test]
    fn apply_pipeline_to_text_chains_modes() {
        let input = "  %E3%81%82  ";
        assert_eq!(
            apply_pipeline_to_text(
                input,
                &[RefineMode::UrlDecode, RefineMode::Trim],
                &empty_ctx(),
            ),
            Ok(ClipboardProcessOutcome::Processed("あ".to_string()))
        );
    }

    /// 加工パイプラインで変更がない場合は `Unchanged` を返すようにすること
    #[test]
    fn apply_pipeline_to_text_returns_unchanged() {
        assert_eq!(
            apply_pipeline_to_text("hello", &[RefineMode::Trim], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
    }

    /// 加工してクリップボードへ書き戻すようにすること
    #[test]
    fn process_clipboard_trims_and_writes_back() {
        let mut cb = InMemoryTextClipboard::with_text("  hello  ");

        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 変更がない場合はクリップボードを更新しないようにすること
    #[test]
    fn process_clipboard_leaves_unchanged_text() {
        let mut cb = InMemoryTextClipboard::with_text("hello");

        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
        assert_eq!(cb.text(), "hello");
    }

    /// 読み取り失敗時は `ReadFailed` を返すようにすること
    #[test]
    fn process_clipboard_read_failure() {
        let mut cb = InMemoryTextClipboard::with_text("x").fail_on_read();

        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::ReadFailed("read failed".to_string()))
        );
    }

    /// 書き込み失敗時は `WriteFailed` を返し、元の内容を維持すること
    #[test]
    fn process_clipboard_write_failure() {
        let mut cb = InMemoryTextClipboard::with_text("  x  ").fail_on_write();

        assert_eq!(
            process_clipboard(&mut cb, RefineMode::Trim, &empty_ctx()),
            Err(ClipboardProcessError::WriteFailed(
                "write failed".to_string()
            ))
        );
        assert_eq!(cb.text(), "  x  ");
    }

    /// TSV 形式だが描画画像がない場合は `NoImage` を返すようにすること
    #[test]
    fn process_image_clipboard_returns_no_image_without_bitmap() {
        let mut cb = InMemoryTextClipboard::with_text("A\tB\n1\t2");

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Err(ClipboardProcessError::NoImage)
        );
    }

    /// 描画画像がある場合はクリップボードへ書き込むようにすること
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
        // エクスプローラへ貼り付けられるよう、ファイルとしても載せる
        assert!(cb.image_written_as_file());
    }

    /// Unicode テキストが無くても描画画像があれば書き込むようにすること
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

    /// Unicode テキストも描画画像も無い場合は `NoText` を返すようにすること
    #[test]
    fn process_image_clipboard_returns_no_text_without_bitmap() {
        let mut cb = InMemoryTextClipboard::with_text("unused").fail_on_read();

        assert_eq!(
            process_image_clipboard(&mut cb, RefineMode::ExcelToImage, &empty_ctx()),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// `CF_HTML` があれば HTML→Markdown の入力に優先して使うようにすること
    #[test]
    fn html_to_markdown_prefers_clipboard_html() {
        let mut cb = InMemoryTextClipboard::with_text(
            "Title
bold",
        )
        .with_html("<h1>Title</h1><p><b>bold</b></p>");

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::HtmlToMarkdown], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed(
                "# Title

**bold**"
                    .to_string()
            ))
        );
        // テキストとして書き戻し、HTML 形式は残さない
        assert_eq!(
            cb.text(),
            "# Title

**bold**"
        );
        assert_eq!(cb.html(), None);
    }

    /// `CF_HTML` が無い場合はテキストを HTML ソースとして変換するようにすること
    #[test]
    fn html_to_markdown_falls_back_to_text() {
        let mut cb = InMemoryTextClipboard::with_text("<p><em>x</em></p>");

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::HtmlToMarkdown], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Processed("*x*".to_string()))
        );
    }

    /// HTML 由来の変換結果が既存テキストと同じなら書き戻さないようにすること
    #[test]
    fn html_to_markdown_unchanged_when_result_matches_text() {
        let mut cb = InMemoryTextClipboard::with_text("hello").with_html("<span>hello</span>");

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::HtmlToMarkdown], &empty_ctx()),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
        assert_eq!(cb.html(), Some("<span>hello</span>"));
    }

    /// 先頭以外の HTML→Markdown は `CF_HTML` を読まないようにすること
    #[test]
    fn html_to_markdown_reads_clipboard_html_only_at_head() {
        let mut cb = InMemoryTextClipboard::with_text("  <b>text</b>  ").with_html("<i>html</i>");

        assert_eq!(
            process_clipboard_pipeline_io(
                &mut cb,
                &[RefineMode::Trim, RefineMode::HtmlToMarkdown],
                &empty_ctx(),
            ),
            Ok(ClipboardProcessOutcome::Processed("**text**".to_string()))
        );
    }

    /// 入れ子が上限を超える HTML は `HtmlNestingTooDeep` になるようにすること
    #[test]
    fn html_to_markdown_reports_too_deep_nesting() {
        let depth = crate::consts::MAX_HTML_NESTING_DEPTH + 1;
        let html = format!("{}x{}", "<div>".repeat(depth), "</div>".repeat(depth));
        let mut cb = InMemoryTextClipboard::with_text("x").with_html(html);

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::HtmlToMarkdown], &empty_ctx()),
            Err(ClipboardProcessError::HtmlNestingTooDeep)
        );
        assert_eq!(cb.text(), "x");
    }

    /// Markdown をリッチテキストとして書き込み、代替テキストに Markdown を残すようにすること
    #[test]
    fn markdown_to_rich_text_writes_html_with_markdown_alt() {
        let mut cb = InMemoryTextClipboard::with_text("**bold**");

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::MarkdownToRichText], &empty_ctx(),),
            Ok(ClipboardProcessOutcome::Processed("**bold**".to_string()))
        );
        assert_eq!(cb.text(), "**bold**");
        assert_eq!(cb.html(), Some("<p><strong>bold</strong></p>"));
    }

    /// 前段の加工結果をリッチテキストにするようにすること
    #[test]
    fn markdown_to_rich_text_applies_preceding_modes() {
        let mut cb = InMemoryTextClipboard::with_text(
            "A	B
1	2",
        );

        assert_eq!(
            process_clipboard_pipeline_io(
                &mut cb,
                &[RefineMode::ExcelToMarkdown, RefineMode::MarkdownToRichText],
                &empty_ctx(),
            ),
            Ok(ClipboardProcessOutcome::Processed(
                "| A | B |
|---|---|
| 1 | 2 |"
                    .to_string()
            ))
        );
        let html = cb.html().expect("CF_HTML が書き込まれる");
        assert!(html.contains("<table border=\"1\""), "{html}");
        assert!(html.contains("<td>1</td>"), "{html}");
    }

    /// HTML→Markdown→リッチテキストの連鎖で `CF_HTML` を読み書きするようにすること
    #[test]
    fn html_to_markdown_then_rich_text_roundtrips() {
        let mut cb = InMemoryTextClipboard::with_text("x").with_html("<p><b>x</b></p>");

        assert_eq!(
            process_clipboard_pipeline_io(
                &mut cb,
                &[RefineMode::HtmlToMarkdown, RefineMode::MarkdownToRichText],
                &empty_ctx(),
            ),
            Ok(ClipboardProcessOutcome::Processed("**x**".to_string()))
        );
        assert_eq!(cb.html(), Some("<p><strong>x</strong></p>"));
    }

    /// リッチテキストの書き込み失敗時は `WriteFailed` を返すようにすること
    #[test]
    fn markdown_to_rich_text_write_failure() {
        let mut cb = InMemoryTextClipboard::with_text("**x**").fail_on_write();

        assert_eq!(
            process_clipboard_pipeline_io(&mut cb, &[RefineMode::MarkdownToRichText], &empty_ctx(),),
            Err(ClipboardProcessError::WriteFailed(
                "write failed".to_string()
            ))
        );
        assert_eq!(cb.html(), None);
    }

    /// Excel TSV 判定は空文字列を除外し、表形式と単一セルを TSV とみなすようにすること
    #[test]
    fn is_excel_tsv_detects_tabular_and_single_cell() {
        assert!(!is_excel_tsv(""));
        assert!(is_excel_tsv("A\tB\n1\t2"));
        assert!(is_excel_tsv("hello"));
    }
}
