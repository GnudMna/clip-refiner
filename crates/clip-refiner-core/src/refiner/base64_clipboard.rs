//! Base64 モードのクリップボード画像連携
//!
//! `Base64Decode` の結果が画像ならクリップボードの画像として書き込み、
//! `Base64Encode` はテキストが無く画像だけがあるとき、その画像を PNG の Base64 テキストへ変換する

use super::clipboard::{ClipboardProcessError, ClipboardProcessOutcome, apply_pipeline_to_text};
use super::context::RefineContext;
use super::mode::RefineMode;
use super::pipeline::apply_text_pipeline;
use super::text_clipboard::{ImageClipboard, TextClipboard};
use super::transform::base64_image::{Base64ImageError, decode_base64_image, encode_base64_png};
use crate::security::is_within_clipboard_limit;

// ======================================================================
// Base64 → 画像
// ======================================================================
/// テキスト加工の最終段が `Base64Decode` で、その入力が画像の Base64 なら画像として書き込む
///
/// 前段までの加工結果を `Base64Decode` の入力とする。画像として扱えない場合は何もしない
/// (呼び出し側が通常のテキスト加工へ進む)
///
/// # Arguments
/// * `clipboard` - 画像の書き込み先
/// * `source` - 加工前のテキスト
/// * `text_modes` - テキスト加工のモード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(Some(ClipboardProcessOutcome::ImageProcessed))` - 画像を書き込んだ
/// * `Ok(None)` - 対象外、または画像として扱えない
///
/// # Errors
/// 画像が上限を超える場合、またはクリップボードへの書き込みに失敗した場合
pub(super) fn write_base64_image<C: ImageClipboard>(
    clipboard: &mut C,
    source: &str,
    text_modes: &[RefineMode],
    ctx: &RefineContext,
) -> Result<Option<ClipboardProcessOutcome>, ClipboardProcessError> {
    let Some((&RefineMode::Base64Decode, preceding)) = text_modes.split_last() else {
        return Ok(None);
    };

    // 前段の失敗 (空・上限超過など) は通常のテキスト加工の側で報告する
    let input = match apply_pipeline_to_text(source, preceding, ctx) {
        Ok(ClipboardProcessOutcome::Processed(result)) => result,
        Ok(_) => source.to_string(),
        Err(_) => return Ok(None),
    };

    match decode_base64_image(&input) {
        Ok(image) => {
            clipboard.set_image_as_file(image.width, image.height, image.rgba)?;
            Ok(Some(ClipboardProcessOutcome::ImageProcessed {
                width: image.width,
                height: image.height,
            }))
        }
        Err(Base64ImageError::Invalid) => Ok(None),
        Err(Base64ImageError::TooLarge) => Err(ClipboardProcessError::ImageTooLarge),
    }
}

// ======================================================================
// 画像 → Base64
// ======================================================================
/// 先頭が `Base64Encode` のとき、クリップボードの画像を PNG の Base64 テキストとして書き込む
///
/// 先頭のモードの出力を画像から得て、残りのモードを順に適用する。
/// クリップボードに画像が無い場合は何もしない (呼び出し側が通常のエラー処理へ進む)
///
/// # Arguments
/// * `clipboard` - 画像の読み取り元、テキストの書き込み先
/// * `text_modes` - テキスト加工のモード列
/// * `ctx` - 設定依存の加工パラメータ
///
/// # Returns
/// * `Ok(Some(ClipboardProcessOutcome::Processed))` - 変換して書き込んだ Base64 テキスト
/// * `Ok(None)` - 対象外、または画像が無い
///
/// # Errors
/// 画像またはテキストが上限を超える場合、画像を変換できない場合、書き込みに失敗した場合
pub(super) fn write_image_as_base64<C: TextClipboard + ImageClipboard>(
    clipboard: &mut C,
    text_modes: &[RefineMode],
    ctx: &RefineContext,
) -> Result<Option<ClipboardProcessOutcome>, ClipboardProcessError> {
    let Some((&RefineMode::Base64Encode, following)) = text_modes.split_first() else {
        return Ok(None);
    };
    let Ok((width, height, rgba)) = clipboard.get_image() else {
        return Ok(None);
    };

    let encoded = encode_base64_png(width, height, rgba).map_err(|e| match e {
        Base64ImageError::TooLarge => ClipboardProcessError::ImageTooLarge,
        Base64ImageError::Invalid => {
            ClipboardProcessError::ReadFailed("画像を PNG へ変換できませんでした".to_string())
        }
    })?;
    if !is_within_clipboard_limit(&encoded) {
        return Err(ClipboardProcessError::TextTooLarge);
    }

    let result = apply_text_pipeline(&encoded, following, ctx).unwrap_or(encoded);
    clipboard.set_text(result.clone())?;
    Ok(Some(ClipboardProcessOutcome::Processed(result)))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts;
    use crate::refiner::clipboard::process_clipboard_pipeline_io;
    use crate::refiner::transform::base64::base64_encode_bytes;
    use crate::refiner::transform::base64_image::{decode_base64_image, png_with_width};
    use crate::test_helpers::InMemoryTextClipboard;

    fn rgba_2x2() -> Vec<u8> {
        vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
    }

    fn png_base64() -> String {
        encode_base64_png(2, 2, rgba_2x2()).expect("encode")
    }

    fn run(
        clipboard: &mut InMemoryTextClipboard,
        pipeline: &[RefineMode],
    ) -> Result<ClipboardProcessOutcome, ClipboardProcessError> {
        process_clipboard_pipeline_io(clipboard, pipeline, &RefineContext::default())
    }

    /// 画像の Base64 を `Base64Decode` で画像として書き込むこと
    #[test]
    fn decode_writes_image_to_clipboard() {
        let mut clipboard = InMemoryTextClipboard::with_text(png_base64());

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Decode]),
            Ok(ClipboardProcessOutcome::ImageProcessed {
                width: 2,
                height: 2
            })
        );
        assert_eq!(clipboard.written_image_size(), Some((2, 2)));
        // エクスプローラへ貼り付けられるよう、ファイルとしても載せる
        assert!(clipboard.image_written_as_file());
    }

    /// data URI と前後の空白があっても、前段の加工を経て画像を書き込むこと
    #[test]
    fn decode_writes_image_after_preceding_modes() {
        let text = format!("  data:image/png;base64,{}\n", png_base64());
        let mut clipboard = InMemoryTextClipboard::with_text(text);

        assert_eq!(
            run(
                &mut clipboard,
                &[RefineMode::Trim, RefineMode::Base64Decode]
            ),
            Ok(ClipboardProcessOutcome::ImageProcessed {
                width: 2,
                height: 2
            })
        );
    }

    /// 画像でない Base64 は、従来どおりテキストとしてデコードすること
    #[test]
    fn decode_keeps_text_behavior_for_non_image() {
        let mut clipboard = InMemoryTextClipboard::with_text("aGVsbG8=");

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Decode]),
            Ok(ClipboardProcessOutcome::Processed("hello".to_string()))
        );
        assert_eq!(clipboard.text(), "hello");
        assert_eq!(clipboard.written_image_size(), None);
    }

    /// 画像でも文字列でもない Base64 は変更せず、画像も書き込まないこと
    #[test]
    fn decode_leaves_binary_non_image_unchanged() {
        let binary = base64_encode_bytes(&[0xff, 0xfe, 0x00, 0x01]);
        let mut clipboard = InMemoryTextClipboard::with_text(binary.clone());

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Decode]),
            Ok(ClipboardProcessOutcome::Unchanged)
        );
        assert_eq!(clipboard.text(), binary);
        assert_eq!(clipboard.written_image_size(), None);
    }

    /// 最終段が `Base64Decode` でないパイプラインは画像を書き込まないこと
    ///
    /// 画像のデコード結果は UTF-8 でないためテキストとしては変わらず、後段は元のテキストに作用する
    #[test]
    fn decode_in_the_middle_does_not_write_image() {
        let mut clipboard = InMemoryTextClipboard::with_text(png_base64());

        let outcome = run(
            &mut clipboard,
            &[RefineMode::Base64Decode, RefineMode::Base64Encode],
        );
        assert_eq!(
            outcome,
            Ok(ClipboardProcessOutcome::Processed(base64_encode_bytes(
                png_base64().as_bytes()
            )))
        );
        assert_eq!(clipboard.written_image_size(), None);
    }

    /// 上限を超える寸法の画像は `ImageTooLarge` になり、クリップボードを変更しないこと
    #[test]
    fn decode_rejects_oversized_image() {
        let png = png_with_width(consts::MAX_BASE64_IMAGE_DIMENSION + 1);
        let text = base64_encode_bytes(&png);
        let mut clipboard = InMemoryTextClipboard::with_text(text.clone());

        let outcome = run(&mut clipboard, &[RefineMode::Base64Decode]);
        assert_eq!(outcome, Err(ClipboardProcessError::ImageTooLarge));
        assert_eq!(clipboard.text(), text);
        assert_eq!(clipboard.written_image_size(), None);
    }

    /// テキストが無く画像だけがあるとき、`Base64Encode` が PNG の Base64 を書き込むこと
    #[test]
    fn encode_converts_image_only_clipboard() {
        let mut clipboard =
            InMemoryTextClipboard::with_text("").with_source_image(2, 2, rgba_2x2());

        let Ok(ClipboardProcessOutcome::Processed(encoded)) =
            run(&mut clipboard, &[RefineMode::Base64Encode])
        else {
            panic!("画像が Base64 へ変換されていない");
        };
        assert_eq!(clipboard.text(), encoded);

        let decoded = decode_base64_image(&encoded).expect("PNG として復元できる");
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.rgba, rgba_2x2());
    }

    /// 後続のモードが画像から得た Base64 に適用されること
    #[test]
    fn encode_applies_following_modes_to_image_base64() {
        let mut clipboard =
            InMemoryTextClipboard::with_text("").with_source_image(2, 2, rgba_2x2());

        let Ok(ClipboardProcessOutcome::Processed(result)) = run(
            &mut clipboard,
            &[RefineMode::Base64Encode, RefineMode::ToHalfwidth],
        ) else {
            panic!("画像が Base64 へ変換されていない");
        };
        assert_eq!(result, png_base64());

        let Ok(ClipboardProcessOutcome::Processed(result)) = run(
            &mut InMemoryTextClipboard::with_text("").with_source_image(2, 2, rgba_2x2()),
            &[RefineMode::Base64Encode, RefineMode::Base64Encode],
        ) else {
            panic!("画像が Base64 へ変換されていない");
        };
        assert_eq!(result, base64_encode_bytes(png_base64().as_bytes()));
    }

    /// テキストがあるときは画像より優先してテキストを Base64 へ変換すること
    #[test]
    fn encode_prefers_text_over_image() {
        let mut clipboard =
            InMemoryTextClipboard::with_text("hello").with_source_image(2, 2, rgba_2x2());

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Encode]),
            Ok(ClipboardProcessOutcome::Processed("aGVsbG8=".to_string()))
        );
    }

    /// テキストも画像も無いときは `NoText` になること
    #[test]
    fn encode_without_text_or_image_is_no_text() {
        let mut clipboard = InMemoryTextClipboard::with_text("");

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Encode]),
            Err(ClipboardProcessError::NoText)
        );
    }

    /// `Base64Encode` で始まらないパイプラインは、画像だけのクリップボードを変換しないこと
    #[test]
    fn other_modes_do_not_convert_image_only_clipboard() {
        let mut clipboard =
            InMemoryTextClipboard::with_text("").with_source_image(2, 2, rgba_2x2());

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Trim]),
            Err(ClipboardProcessError::NoText)
        );
        assert_eq!(clipboard.text(), "");
    }

    /// 画像が上限を超えるときは `ImageTooLarge` になること
    #[test]
    fn encode_rejects_oversized_image() {
        let side = consts::MAX_BASE64_IMAGE_DIMENSION + 1;
        let mut clipboard =
            InMemoryTextClipboard::with_text("").with_source_image(side, 1, Vec::new());

        assert_eq!(
            run(&mut clipboard, &[RefineMode::Base64Encode]),
            Err(ClipboardProcessError::ImageTooLarge)
        );
    }
}
