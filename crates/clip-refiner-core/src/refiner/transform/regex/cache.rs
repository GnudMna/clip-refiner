use crate::config::RegexStep;
use crate::consts::{MAX_REGEX_PATTERN_BYTES, REGEX_PATTERN_CACHE_CAPACITY};

use regex::{Regex, RegexBuilder};

// ======================================================================
// コンパイルキャッシュ
// ======================================================================
/// 正規表現パターンのコンパイル結果を保持する LRU キャッシュ
///
/// パターンとフラグが変わらない限り、コンパイル済み `Regex` を再利用する
#[derive(Debug, Default)]
pub(crate) struct RegexPatternCache {
    /// 先頭が直近利用。容量は `REGEX_PATTERN_CACHE_CAPACITY`
    entries: Vec<(RegexCompileKey, Regex)>,
}

/// コンパイル結果のキャッシュキー (`replacement` はコンパイルに影響しない)
#[derive(Clone, Debug, PartialEq, Eq)]
struct RegexCompileKey {
    pattern: String,
    case_insensitive: bool,
    multiline: bool,
}

impl RegexPatternCache {
    /// ステップに対応するコンパイル済み正規表現を返す
    ///
    /// 同一キーがあれば先頭へ移動して再利用する
    pub(crate) fn get_or_compile(&mut self, step: &RegexStep) -> Option<&Regex> {
        let key = RegexCompileKey {
            pattern: step.pattern.clone(),
            case_insensitive: step.case_insensitive,
            multiline: step.multiline,
        };

        if let Some(index) = self
            .entries
            .iter()
            .position(|(existing, _)| existing == &key)
        {
            let entry = self.entries.remove(index);
            self.entries.insert(0, entry);
            return self.entries.first().map(|(_, regex)| regex);
        }

        let compiled = compile_pattern(step)?;
        self.entries.insert(0, (key, compiled));
        if self.entries.len() > REGEX_PATTERN_CACHE_CAPACITY {
            self.entries.pop();
        }
        self.entries.first().map(|(_, regex)| regex)
    }
}

// ======================================================================
// 正規表現コンパイル
// ======================================================================
/// ステップから正規表現をコンパイルする
///
/// パターンが空、上限超過、または構文エラーの場合は `None` を返す
fn compile_pattern(step: &RegexStep) -> Option<Regex> {
    if step.pattern.is_empty() || step.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return None;
    }

    RegexBuilder::new(&step.pattern)
        .case_insensitive(step.case_insensitive)
        .multi_line(step.multiline)
        .build()
        .ok()
}

/// 設定 UI / パッチ検証向けにステップをコンパイルできるか確認する
///
/// # Arguments
/// * `step` - 検証対象ステップ
///
/// # Returns
/// * `Result<(), String>` - 成功時は `Ok`、失敗時は理由
pub fn validate_regex_step(step: &RegexStep) -> Result<(), String> {
    if step.pattern.is_empty() {
        return Ok(());
    }
    if step.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return Err(format!(
            "パターンが長すぎます (最大 {MAX_REGEX_PATTERN_BYTES} バイト)"
        ));
    }
    RegexBuilder::new(&step.pattern)
        .case_insensitive(step.case_insensitive)
        .multi_line(step.multiline)
        .build()
        .map(|_| ())
        .map_err(|err| format!("パターンが無効です: {err}"))
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::RegexPatternCache;
    use crate::config::RegexStep;

    fn step(pattern: &str) -> RegexStep {
        RegexStep {
            pattern: pattern.to_string(),
            ..RegexStep::default()
        }
    }

    /// 同一設定ではコンパイル結果を再利用すること
    #[test]
    fn test_regex_cache_reuses_compiled_pattern() {
        let settings = step(r"\d+");
        let mut cache = RegexPatternCache::default();

        let first = cache.get_or_compile(&settings).map(std::ptr::from_ref);
        let second = cache.get_or_compile(&settings).map(std::ptr::from_ref);

        assert_eq!(first, second);
    }

    /// 異なるパターンを複数キャッシュできること
    #[test]
    fn test_regex_cache_keeps_multiple_entries() {
        let mut cache = RegexPatternCache::default();
        let digit = cache.get_or_compile(&step(r"\d")).expect("digit").clone();
        let alpha = cache
            .get_or_compile(&step(r"[a-z]"))
            .expect("alpha")
            .clone();

        assert!(digit.is_match("1"));
        assert!(!digit.is_match("a"));
        assert!(alpha.is_match("a"));
        assert!(!alpha.is_match("1"));

        let digit_again = cache.get_or_compile(&step(r"\d")).expect("digit again");
        assert!(digit_again.is_match("9"));
    }
}
