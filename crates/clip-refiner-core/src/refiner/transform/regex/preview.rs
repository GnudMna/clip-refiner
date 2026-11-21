use regex::RegexBuilder;

use super::cache::validate_regex_step;
use super::match_timeout::{RegexMatchTimeout, replace_all_cancellable, run_with_match_timeout};
use crate::config::{RegexSettings, RegexStep};
use crate::consts::REGEX_MATCH_TIMEOUT_MS;

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
    /// 置換後テキスト (`preview_regex_replace` で `valid` のとき。パターンが空ならサンプルのまま)
    pub result: Option<String>,
}

/// 全プリセットの全ステップをコンパイル検証する
///
/// # Arguments
/// * `settings` - 検証対象の正規表現設定
///
/// # Returns
/// * `RegexPreviewResult` - すべて有効なら `valid: true`
pub fn validate_regex_settings(settings: &RegexSettings) -> RegexPreviewResult {
    for preset in &settings.presets {
        for (index, step) in preset.steps.iter().enumerate() {
            if let Err(message) = validate_regex_step(step) {
                return RegexPreviewResult {
                    valid: false,
                    message: format!(
                        "プリセット「{}」のステップ {} : {message}",
                        preset.name,
                        index + 1
                    ),
                    result: None,
                };
            }
        }
    }

    RegexPreviewResult {
        valid: true,
        message: "パターンは有効です".to_string(),
        result: None,
    }
}

/// 有効プリセットのステップを順に置換してプレビュー結果を返す
///
/// 実行時の `regex_replace` と同じ `regex` クレートで評価する
/// マッチ実行が制限時間を超えた場合は無効として扱う
///
/// # Arguments
/// * `sample` - プレビュー対象のサンプル文字列
/// * `settings` - 正規表現プリセット設定
///
/// # Returns
/// * `RegexPreviewResult` - パターンの有効性、状態メッセージ、置換後テキスト
pub fn preview_regex_replace(sample: &str, settings: &RegexSettings) -> RegexPreviewResult {
    let validation = validate_regex_settings(settings);
    if !validation.valid {
        return validation;
    }

    let steps = settings.active_steps();
    if steps.is_empty() || steps.iter().all(|step| step.pattern.is_empty()) {
        return RegexPreviewResult {
            valid: true,
            message: "パターンが空です (マッチしません)".to_string(),
            result: Some(sample.to_string()),
        };
    }

    let mut current = sample.to_owned();
    for step in steps {
        match apply_preview_replace(&current, step) {
            Ok(next) => current = next,
            Err(message) => {
                return RegexPreviewResult {
                    valid: false,
                    message,
                    result: None,
                };
            }
        }
    }

    RegexPreviewResult {
        valid: true,
        message: "パターンは有効です".to_string(),
        result: Some(current),
    }
}

fn apply_preview_replace(sample: &str, step: &RegexStep) -> Result<String, String> {
    if step.pattern.is_empty() {
        return Ok(sample.to_string());
    }

    let re = RegexBuilder::new(&step.pattern)
        .case_insensitive(step.case_insensitive)
        .multi_line(step.multiline)
        .build()
        .map_err(|err| format!("パターンが無効です: {err}"))?;

    let sample_owned = sample.to_owned();
    let replacement = step.replacement.clone();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &sample_owned, &replacement, cancel)
    }) {
        Ok(result) => Ok(result),
        Err(RegexMatchTimeout) => Err(format!(
            "パターンの実行がタイムアウトしました (上限 {REGEX_MATCH_TIMEOUT_MS} ms)"
        )),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::super::match_timeout::with_pool_test_lock;
    use super::{REGEX_PREVIEW_SAMPLE, preview_regex_replace, validate_regex_settings};
    use crate::config::{RegexPreset, RegexSettings, RegexStep};

    /// プレビューが置換結果とエラーを返すようにすること
    #[test]
    fn test_preview_regex_replace() {
        with_pool_test_lock(|| {
            let ok =
                preview_regex_replace(REGEX_PREVIEW_SAMPLE, &RegexSettings::single(r"\d+", "#"));
            assert!(ok.valid);
            assert_eq!(ok.result.as_deref(), Some("Hello world #"));

            let invalid = preview_regex_replace(
                REGEX_PREVIEW_SAMPLE,
                &RegexSettings::single("[unclosed", ""),
            );
            assert!(!invalid.valid);
            assert!(invalid.result.is_none());
        });
    }

    /// 複数ステップの順次置換をプレビューできるようにすること
    #[test]
    fn preview_applies_steps_in_order() {
        with_pool_test_lock(|| {
            let settings = RegexSettings {
                active_preset_id: "default".to_string(),
                presets: vec![RegexPreset {
                    id: "default".to_string(),
                    name: "デフォルト".to_string(),
                    steps: vec![
                        RegexStep {
                            pattern: "Hello".to_string(),
                            replacement: "Hi".to_string(),
                            ..RegexStep::default()
                        },
                        RegexStep {
                            pattern: r"\d+".to_string(),
                            replacement: "#".to_string(),
                            ..RegexStep::default()
                        },
                    ],
                }],
            };
            let preview = preview_regex_replace(REGEX_PREVIEW_SAMPLE, &settings);
            assert!(preview.valid);
            assert_eq!(preview.result.as_deref(), Some("Hi world #"));
        });
    }

    /// 他プリセットの無効ステップも検証で拒否すること
    #[test]
    fn validate_checks_all_presets() {
        let settings = RegexSettings {
            active_preset_id: "ok".to_string(),
            presets: vec![
                RegexPreset {
                    id: "ok".to_string(),
                    name: "OK".to_string(),
                    steps: vec![RegexStep::default()],
                },
                RegexPreset {
                    id: "bad".to_string(),
                    name: "Bad".to_string(),
                    steps: vec![RegexStep {
                        pattern: "[unclosed".to_string(),
                        ..RegexStep::default()
                    }],
                },
            ],
        };
        let result = validate_regex_settings(&settings);
        assert!(!result.valid);
        assert!(result.message.contains("Bad"));
    }
}
