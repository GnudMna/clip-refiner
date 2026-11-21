//! 加工パイプラインとクリップボード API の統合テスト

mod common;

use clip_refiner_core::consts::MAX_CLIPBOARD_TEXT_BYTES;
use clip_refiner_core::refiner::process_clipboard_pipeline_io;
use clip_refiner_core::test_helpers::InMemoryTextClipboard;
use clip_refiner_core::{
    ClipboardProcessError, ClipboardProcessOutcome, RefineContext, RefineMode,
    apply_pipeline_to_text, apply_text_pipeline, is_within_clipboard_limit, is_within_parser_limit,
    split_pipeline,
};

fn default_ctx() -> RefineContext {
    RefineContext::default()
}

// ======================================================================
// テスト
// ======================================================================

/// 空文字列は `NoText` になるようにすること
#[test]
fn apply_pipeline_rejects_empty_text() {
    assert_eq!(
        apply_pipeline_to_text("", &[RefineMode::Trim], &default_ctx()),
        Err(ClipboardProcessError::NoText)
    );
}

/// 上限超過は `TextTooLarge` になるようにすること
#[test]
fn apply_pipeline_rejects_oversized_text() {
    let oversized = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
    assert_eq!(
        apply_pipeline_to_text(&oversized, &[RefineMode::Trim], &default_ctx()),
        Err(ClipboardProcessError::TextTooLarge)
    );
}

/// パーサー上限超過は `ParserInputTooLarge` になるようにすること
#[test]
fn apply_pipeline_rejects_parser_oversized_text() {
    use clip_refiner_core::consts::MAX_PARSER_INPUT_BYTES;

    let oversized = "a".repeat(MAX_PARSER_INPUT_BYTES + 1);
    assert!(is_within_clipboard_limit(&oversized));
    assert!(!is_within_parser_limit(&oversized));
    assert_eq!(
        apply_pipeline_to_text(&oversized, &[RefineMode::JsonFormat], &default_ctx()),
        Err(ClipboardProcessError::ParserInputTooLarge)
    );
}

/// 上限ちょうどの入力は処理できるようにすること
#[test]
fn apply_pipeline_accepts_text_at_limit() {
    let at_limit = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES);
    assert!(is_within_clipboard_limit(&at_limit));
    assert_eq!(
        apply_pipeline_to_text(&at_limit, &[RefineMode::Trim], &default_ctx()),
        Ok(ClipboardProcessOutcome::Unchanged)
    );
}

/// 変更がない場合は `Unchanged` を返すようにすること
#[test]
fn apply_pipeline_returns_unchanged_when_no_op() {
    assert_eq!(
        apply_pipeline_to_text("hello", &[RefineMode::Trim], &default_ctx()),
        Ok(ClipboardProcessOutcome::Unchanged)
    );
}

/// 画像モードのみのパイプラインはテキスト API では `Unchanged` になるようにすること
#[test]
fn apply_pipeline_image_only_yields_unchanged_for_text_api() {
    assert_eq!(
        apply_pipeline_to_text("hello", &[RefineMode::ExcelToImage], &default_ctx()),
        Ok(ClipboardProcessOutcome::Unchanged)
    );
}

/// テキスト加工後に末尾画像モードが分割されるようにすること
#[test]
fn split_pipeline_separates_trailing_image_mode() {
    let pipeline = vec![RefineMode::UrlDecode, RefineMode::ExcelToImage];
    assert_eq!(
        split_pipeline(&pipeline),
        (vec![RefineMode::UrlDecode], Some(RefineMode::ExcelToImage))
    );
}

/// `apply_text_pipeline` が空モード列で `None` を返すようにすること
#[test]
fn apply_text_pipeline_empty_modes_returns_none() {
    assert_eq!(apply_text_pipeline("hello", &[], &default_ctx()), None);
}

/// 設定の正規表現がパイプライン経由で適用されるようにすること
#[test]
fn config_regex_applies_through_pipeline() {
    let ctx = RefineContext::with_regex(clip_refiner_core::config::RegexSettings::single(
        r"\s+", "-",
    ));

    assert_eq!(
        apply_text_pipeline("a   b", &[RefineMode::RegexReplace], &ctx),
        Some("a-b".to_string())
    );
}

/// `process_clipboard_pipeline_io` が空パイプラインを拒否すること
#[test]
fn process_clipboard_io_rejects_empty_pipeline() {
    let mut cb = InMemoryTextClipboard::with_text("hello");
    assert_eq!(
        process_clipboard_pipeline_io(&mut cb, &[], &default_ctx()),
        Err(ClipboardProcessError::NoText)
    );
}

/// テキスト加工と Excel 画像書き込みを連鎖できるようにすること
#[test]
fn process_clipboard_io_chains_text_then_image() {
    let rgba = vec![255_u8; 4 * 2 * 2];
    let mut cb = InMemoryTextClipboard::with_text("  %E3%81%82  ").with_source_image(2, 2, rgba);

    let pipeline = vec![
        RefineMode::UrlDecode,
        RefineMode::Trim,
        RefineMode::ExcelToImage,
    ];
    assert_eq!(
        process_clipboard_pipeline_io(&mut cb, &pipeline, &default_ctx()),
        Ok(ClipboardProcessOutcome::ImageProcessed {
            width: 2,
            height: 2
        })
    );
    assert_eq!(cb.text(), "あ");
    assert_eq!(cb.written_image_size(), Some((2, 2)));
}

/// 読み取り失敗時に詳細付き `ReadFailed` になるようにすること
#[test]
fn process_clipboard_io_surfaces_read_failure() {
    let mut cb = InMemoryTextClipboard::with_text("x").fail_on_read();
    assert_eq!(
        process_clipboard_pipeline_io(&mut cb, &[RefineMode::Trim], &default_ctx()),
        Err(ClipboardProcessError::ReadFailed("read failed".to_string()))
    );
}

/// 書き込み失敗時に元テキストを維持すること
#[test]
fn process_clipboard_io_surfaces_write_failure() {
    let mut cb = InMemoryTextClipboard::with_text("  x  ").fail_on_write();
    assert_eq!(
        process_clipboard_pipeline_io(&mut cb, &[RefineMode::Trim], &default_ctx()),
        Err(ClipboardProcessError::WriteFailed(
            "write failed".to_string()
        ))
    );
    assert_eq!(cb.text(), "  x  ");
}

/// パーサー上限境界が公開 API で参照できるようにすること
#[test]
fn parser_limit_boundary_is_exposed() {
    let within = "x".repeat(clip_refiner_core::consts::MAX_PARSER_INPUT_BYTES);
    let over = "x".repeat(clip_refiner_core::consts::MAX_PARSER_INPUT_BYTES + 1);
    assert!(is_within_parser_limit(&within));
    assert!(!is_within_parser_limit(&over));
}

/// 全モードをパイプライン API 経由でパニックせずに呼び出せるようにすること
#[test]
fn every_mode_runs_through_apply_pipeline_without_panic() {
    let ctx = default_ctx();
    for (mode, input, _) in common::TEXT_MODE_CASES {
        let _ = apply_pipeline_to_text(input, &[*mode], &ctx);
    }
    let ts_input = "1672531200";
    let _ = apply_pipeline_to_text(ts_input, &[RefineMode::ToDatetimeString], &ctx);
    let regex_ctx = common::regex_ctx(r"\d", "X");
    let _ = apply_pipeline_to_text("a1", &[RefineMode::RegexReplace], &regex_ctx);
}
