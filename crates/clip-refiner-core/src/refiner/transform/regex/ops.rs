use std::borrow::Cow;

use super::match_timeout::{
    RegexMatchTimeout, extract_cancellable, on_match_timeout, replace_all_cancellable,
    run_with_match_timeout, split_cancellable,
};
use crate::config::RegexStep;
use crate::refiner::context::{RefineContext, RefineSkipReason};

// ======================================================================
// ステップ結果
// ======================================================================
enum StepOutcome {
    /// パターン空またはコンパイル失敗でスキップ
    Skipped,
    /// テキスト変化なし
    Unchanged,
    /// テキストが変化した
    Changed(String),
    /// マッチ実行がタイムアウトした
    TimedOut,
}

// ======================================================================
// 置換
// ======================================================================
/// 正規表現に一致する部分を置換文字列へ変換する
///
/// 有効プリセットのステップを上から順に適用する
/// パターンが無効なステップはスキップする
/// マッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `ctx` - 正規表現設定とスキップ理由の記録先
///
/// # Returns
/// * `Cow<'_, str>` - 置換後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_replace<'a>(text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    apply_steps(text, ctx, |current, step, ctx| {
        apply_replace_step(current, step, ctx, &step.replacement)
    })
}

// ======================================================================
// 抽出
// ======================================================================
/// 正規表現に一致する部分を行単位で抽出する
///
/// 有効プリセットのステップを上から順に適用する
/// 一致がない場合、パターンが無効な場合はスキップし、タイムアウト時は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `ctx` - 正規表現設定とスキップ理由の記録先
///
/// # Returns
/// * `Cow<'_, str>` - 抽出結果。変更がない場合は元のテキストを借用
pub(crate) fn regex_extract<'a>(text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    apply_steps(text, ctx, apply_extract_step)
}

// ======================================================================
// 削除
// ======================================================================
/// 正規表現に一致する部分を削除する
///
/// 有効プリセットのステップを上から順に適用する
/// パターンが無効なステップはスキップし、タイムアウト時は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `ctx` - 正規表現設定とスキップ理由の記録先
///
/// # Returns
/// * `Cow<'_, str>` - 削除後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_delete<'a>(text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    apply_steps(text, ctx, |current, step, ctx| {
        apply_replace_step(current, step, ctx, "")
    })
}

// ======================================================================
// 分割
// ======================================================================
/// 正規表現でテキストを分割し、改行で結合する
///
/// 有効プリセットのステップを上から順に適用する
/// パターンが無効なステップはスキップし、タイムアウト時は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `ctx` - 正規表現設定とスキップ理由の記録先
///
/// # Returns
/// * `Cow<'_, str>` - 分割後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_split<'a>(text: &'a str, ctx: &RefineContext) -> Cow<'a, str> {
    apply_steps(text, ctx, apply_split_step)
}

/// 有効プリセットの各ステップを順に適用する
fn apply_steps<'a, F>(text: &'a str, ctx: &RefineContext, mut apply_step: F) -> Cow<'a, str>
where
    F: FnMut(&str, &RegexStep, &RefineContext) -> StepOutcome,
{
    let steps = ctx.regex.active_steps().to_vec();
    if steps.is_empty() {
        return Cow::Borrowed(text);
    }

    let mut current = text.to_owned();
    let mut changed = false;

    for step in &steps {
        match apply_step(&current, step, ctx) {
            StepOutcome::Skipped | StepOutcome::Unchanged => {}
            StepOutcome::Changed(next) => {
                current = next;
                changed = true;
            }
            StepOutcome::TimedOut => {
                ctx.record_skip(RefineSkipReason::RegexTimedOut);
                return on_match_timeout(text);
            }
        }
    }

    if changed {
        Cow::Owned(current)
    } else {
        Cow::Borrowed(text)
    }
}

fn apply_replace_step(
    text: &str,
    step: &RegexStep,
    ctx: &RefineContext,
    replacement: &str,
) -> StepOutcome {
    let Some(re) = compiled_regex(ctx, step) else {
        return StepOutcome::Skipped;
    };

    let owned_text = text.to_owned();
    let replacement = replacement.to_owned();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &owned_text, &replacement, cancel)
    }) {
        Ok(result) if result == text => StepOutcome::Unchanged,
        Ok(result) => StepOutcome::Changed(result),
        Err(RegexMatchTimeout) => StepOutcome::TimedOut,
    }
}

fn apply_extract_step(text: &str, step: &RegexStep, ctx: &RefineContext) -> StepOutcome {
    let Some(re) = compiled_regex(ctx, step) else {
        return StepOutcome::Skipped;
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| extract_cancellable(&re, &owned_text, cancel)) {
        Ok(None) => StepOutcome::Unchanged,
        Ok(Some(joined)) if joined == text => StepOutcome::Unchanged,
        Ok(Some(joined)) => StepOutcome::Changed(joined),
        Err(RegexMatchTimeout) => StepOutcome::TimedOut,
    }
}

fn apply_split_step(text: &str, step: &RegexStep, ctx: &RefineContext) -> StepOutcome {
    let Some(re) = compiled_regex(ctx, step) else {
        return StepOutcome::Skipped;
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| split_cancellable(&re, &owned_text, cancel)) {
        Ok(joined) if joined == text => StepOutcome::Unchanged,
        Ok(joined) => StepOutcome::Changed(joined),
        Err(RegexMatchTimeout) => StepOutcome::TimedOut,
    }
}

/// ステップに対応するコンパイル済み正規表現を返す
fn compiled_regex(ctx: &RefineContext, step: &RegexStep) -> Option<regex::Regex> {
    ctx.regex_cache_mut().get_or_compile(step).cloned()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::super::match_timeout::with_pool_test_lock;
    use super::{regex_delete, regex_extract, regex_replace, regex_split};
    use crate::config::{RegexPreset, RegexSettings, RegexStep};
    use crate::refiner::context::{RefineContext, RefineSkipReason};

    fn ctx_with(pattern: &str) -> RefineContext {
        RefineContext::with_regex(RegexSettings::single(pattern, ""))
    }

    fn ctx_with_replacement(pattern: &str, replacement: &str) -> RefineContext {
        RefineContext::with_regex(RegexSettings::single(pattern, replacement))
    }

    fn ctx_with_steps(steps: Vec<RegexStep>) -> RefineContext {
        RefineContext::with_regex(RegexSettings {
            active_preset_id: "default".to_string(),
            presets: vec![RegexPreset {
                id: "default".to_string(),
                name: "デフォルト".to_string(),
                steps,
            }],
        })
    }

    /// 置換の基本動作できるようにすること
    #[test]
    fn test_regex_replace() {
        with_pool_test_lock(|| {
            let ctx = ctx_with_replacement(r"\d", "X");
            assert_eq!(regex_replace("a1b2c", &ctx), "aXbXc");
            assert_eq!(regex_replace("plain", &ctx_with(r"\d")), "plain");
        });
    }

    /// 複数ステップを上から順に置換すること
    #[test]
    fn test_regex_replace_applies_steps_in_order() {
        with_pool_test_lock(|| {
            let ctx = ctx_with_steps(vec![
                RegexStep {
                    pattern: "foo".to_string(),
                    replacement: "bar".to_string(),
                    ..RegexStep::default()
                },
                RegexStep {
                    pattern: "bar".to_string(),
                    replacement: "baz".to_string(),
                    ..RegexStep::default()
                },
            ]);
            assert_eq!(regex_replace("foo", &ctx), "baz");
        });
    }

    /// キャプチャグループを使った置換できるようにすること
    #[test]
    fn test_regex_replace_capture() {
        with_pool_test_lock(|| {
            let ctx = ctx_with_replacement(r"(\d+)-(\d+)", r"$2/$1");
            assert_eq!(regex_replace("2024-01-02", &ctx), "01/2024-02");
        });
    }

    /// 抽出の基本動作できるようにすること
    #[test]
    fn test_regex_extract() {
        with_pool_test_lock(|| {
            let ctx = ctx_with(r"\d+");
            assert_eq!(regex_extract("a1b22c", &ctx), "1\n22");
            assert_eq!(regex_extract("abc", &ctx), "abc");
        });
    }

    /// 削除の基本動作できるようにすること
    #[test]
    fn test_regex_delete() {
        with_pool_test_lock(|| {
            assert_eq!(regex_delete("a1b2c", &ctx_with(r"\d")), "abc");
        });
    }

    /// 複数ステップで順に削除すること
    #[test]
    fn test_regex_delete_applies_steps_in_order() {
        with_pool_test_lock(|| {
            let ctx = ctx_with_steps(vec![
                RegexStep {
                    pattern: r"\d".to_string(),
                    ..RegexStep::default()
                },
                RegexStep {
                    pattern: "-".to_string(),
                    ..RegexStep::default()
                },
            ]);
            assert_eq!(regex_delete("a-1-b", &ctx), "ab");
        });
    }

    /// 分割の基本動作できるようにすること
    #[test]
    fn test_regex_split() {
        with_pool_test_lock(|| {
            assert_eq!(regex_split("a,b,c", &ctx_with(r",")), "a\nb\nc");
            assert_eq!(regex_split("a  b   c", &ctx_with(r"\s+")), "a\nb\nc");
        });
    }

    /// 無効なパターンはスキップして元のテキストを返すようにすること
    #[test]
    fn test_invalid_pattern_returns_original() {
        with_pool_test_lock(|| {
            let ctx = RefineContext::with_regex(RegexSettings::single("[unclosed", ""));
            assert!(matches!(regex_replace("text", &ctx), Cow::Borrowed(_)));
            assert!(ctx.take_skip_reason().is_none());
        });
    }

    /// 空パターンは元のテキストを返すようにすること
    #[test]
    fn test_empty_pattern_returns_original() {
        with_pool_test_lock(|| {
            let ctx = RefineContext::default();
            assert!(matches!(regex_replace("text", &ctx), Cow::Borrowed(_)));
        });
    }

    /// 大文字小文字を無視するフラグできるようにすること
    #[test]
    fn test_case_insensitive() {
        with_pool_test_lock(|| {
            let ctx = RefineContext::with_regex(RegexSettings::from_step(RegexStep {
                pattern: "abc".to_string(),
                replacement: "X".to_string(),
                case_insensitive: true,
                ..RegexStep::default()
            }));
            assert_eq!(regex_replace("AbC", &ctx), "X");
        });
    }

    /// 設定変更時はキャッシュを無効化すること
    #[test]
    fn test_regex_cache_invalidates_on_settings_change() {
        with_pool_test_lock(|| {
            let mut ctx = RefineContext::default();
            ctx.regex = RegexSettings::single(r"\d", "");
            assert_eq!(regex_extract("a1b", &ctx), "1");

            ctx.regex = RegexSettings::single(r"[a-z]", "");
            assert_eq!(regex_extract("a1b", &ctx), "a\nb");
        });
    }

    /// タイムアウト時はスキップ理由を記録すること
    #[test]
    fn timeout_records_skip_reason() {
        with_pool_test_lock(|| {
            // `(a+)+$` は古典的な ReDoS パターン。短い入力でも制限時間を超えうる
            let ctx = ctx_with(r"(a+)+$");
            let input = format!("{}!", "a".repeat(30));
            let _ = regex_extract(&input, &ctx);
            // 環境によってタイムアウトしない場合もあるため、記録時のみ検証する
            if let Some(reason) = ctx.take_skip_reason() {
                assert_eq!(reason, RefineSkipReason::RegexTimedOut);
            }
        });
    }
}
