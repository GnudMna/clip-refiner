use super::match_timeout::{RegexMatchTimeout, replace_all_cancellable, run_with_match_timeout};
use crate::config::RegexSettings;
use crate::consts::{MAX_REGEX_PATTERN_BYTES, REGEX_MATCH_TIMEOUT_MS};

use regex::RegexBuilder;

// ======================================================================
// プレビュー
// ======================================================================
/// 設定 UI 向けプレビュー用サンプル文字列
pub const REGEX_PREVIEW_SAMPLE: &str = "Hello world 123";

/// 正規表現プレビューの結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexPreviewResult {
    /// パターンがコンパイル可能かどうか
    pub valid: bool,
    /// 状態メッセージ
    pub message: String,
    /// 置換後テキスト (`valid` かつパターン非空のとき)
    pub result: Option<String>,
}

/// サンプル文字列へ正規表現置換を適用してプレビュー結果を返す
///
/// 実行時の `regex_replace` と同じ `regex` クレートで評価する
/// マッチ実行が制限時間を超えた場合は無効として扱う
///
/// # Arguments
/// * `sample` - プレビュー対象のサンプル文字列
/// * `settings` - 正規表現パターンと置換文字列
///
/// # Returns
/// * `RegexPreviewResult` - パターンの有効性、状態メッセージ、置換後テキスト
pub fn preview_regex_replace(sample: &str, settings: &RegexSettings) -> RegexPreviewResult {
    if settings.pattern.is_empty() {
        return RegexPreviewResult {
            valid: true,
            message: "パターンが空です (マッチしません)".to_string(),
            result: Some(sample.to_string()),
        };
    }

    if settings.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return RegexPreviewResult {
            valid: false,
            message: format!("パターンが長すぎます (最大 {MAX_REGEX_PATTERN_BYTES} バイト)"),
            result: None,
        };
    }

    match RegexBuilder::new(&settings.pattern)
        .case_insensitive(settings.case_insensitive)
        .multi_line(settings.multiline)
        .build()
    {
        Ok(re) => {
            let sample_owned = sample.to_owned();
            let replacement = settings.replacement.clone();
            match run_with_match_timeout(move |cancel| {
                replace_all_cancellable(&re, &sample_owned, &replacement, cancel)
            }) {
                Ok(result) => RegexPreviewResult {
                    valid: true,
                    message: "パターンは有効です".to_string(),
                    result: Some(result),
                },
                Err(RegexMatchTimeout) => RegexPreviewResult {
                    valid: false,
                    message: format!(
                        "パターンの実行がタイムアウトしました (上限 {REGEX_MATCH_TIMEOUT_MS} ms)"
                    ),
                    result: None,
                },
            }
        }
        Err(err) => RegexPreviewResult {
            valid: false,
            message: format!("パターンが無効です: {err}"),
            result: None,
        },
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::match_timeout::with_pool_test_lock;
    use super::{REGEX_PREVIEW_SAMPLE, preview_regex_replace};
    use crate::config::RegexSettings;

    /// プレビューが置換結果とエラーを返すようにすること
    #[test]
    fn test_preview_regex_replace() {
        with_pool_test_lock(|| {
            let ok = preview_regex_replace(
                REGEX_PREVIEW_SAMPLE,
                &RegexSettings {
                    pattern: r"\d+".to_string(),
                    replacement: "#".to_string(),
                    ..RegexSettings::default()
                },
            );
            assert!(ok.valid);
            assert_eq!(ok.result.as_deref(), Some("Hello world #"));

            let invalid = preview_regex_replace(
                REGEX_PREVIEW_SAMPLE,
                &RegexSettings {
                    pattern: "[unclosed".to_string(),
                    ..RegexSettings::default()
                },
            );
            assert!(!invalid.valid);
            assert!(invalid.result.is_none());
        });
    }
}
