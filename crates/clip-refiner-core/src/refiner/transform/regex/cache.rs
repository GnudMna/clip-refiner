use crate::config::RegexSettings;
use crate::consts::MAX_REGEX_PATTERN_BYTES;

use regex::{Regex, RegexBuilder};

// ======================================================================
// コンパイルキャッシュ
// ======================================================================
/// 正規表現パターンのコンパイル結果を保持するキャッシュ
///
/// パターンとフラグが変わらない限り、コンパイル済み `Regex` を再利用する
#[derive(Debug, Default)]
pub(crate) struct RegexPatternCache {
    key: Option<RegexCompileKey>,
    compiled: Option<Regex>,
}

/// コンパイル結果のキャッシュキー (`replacement` はコンパイルに影響しない)
#[derive(Clone, Debug, PartialEq, Eq)]
struct RegexCompileKey {
    pattern: String,
    case_insensitive: bool,
    multiline: bool,
}

impl RegexPatternCache {
    /// 設定に対応するコンパイル済み正規表現を返す
    ///
    /// 設定が前回と同じ場合はキャッシュを再利用する
    pub(crate) fn get_or_compile(&mut self, settings: &RegexSettings) -> Option<&Regex> {
        let key = RegexCompileKey {
            pattern: settings.pattern.clone(),
            case_insensitive: settings.case_insensitive,
            multiline: settings.multiline,
        };

        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.compiled = compile_pattern(settings);
        }

        self.compiled.as_ref()
    }
}

// ======================================================================
// 正規表現コンパイル
// ======================================================================
/// 設定から正規表現をコンパイルする
///
/// パターンが空、上限超過、または構文エラーの場合は `None` を返す
fn compile_pattern(settings: &RegexSettings) -> Option<Regex> {
    if settings.pattern.is_empty() || settings.pattern.len() > MAX_REGEX_PATTERN_BYTES {
        return None;
    }

    RegexBuilder::new(&settings.pattern)
        .case_insensitive(settings.case_insensitive)
        .multi_line(settings.multiline)
        .build()
        .ok()
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::RegexPatternCache;

    use crate::config::RegexSettings;

    fn settings(pattern: &str) -> RegexSettings {
        RegexSettings {
            pattern: pattern.to_string(),
            ..RegexSettings::default()
        }
    }

    /// 同一設定ではコンパイル結果を再利用すること
    #[test]
    fn test_regex_cache_reuses_compiled_pattern() {
        let settings = settings(r"\d+");
        let mut cache = RegexPatternCache::default();

        let first = cache.get_or_compile(&settings).map(std::ptr::from_ref);
        let second = cache.get_or_compile(&settings).map(std::ptr::from_ref);

        assert_eq!(first, second);
    }
}
