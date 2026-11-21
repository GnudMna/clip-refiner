use std::cell::RefCell;

use super::transform::regex::RegexPatternCache;

use crate::config::{AppConfig, RegexSettings};

// ======================================================================
// 加工コンテキスト
// ======================================================================
/// 設定依存の加工モード向けコンテキスト
///
/// 正規表現パターンなど、テキスト以外に必要なパラメータを保持する
/// 正規表現のコンパイル結果は `regex_cache` にキャッシュされる
#[derive(Debug)]
pub struct RefineContext {
    /// 正規表現加工用の設定
    pub regex: RegexSettings,
    regex_cache: RefCell<RegexPatternCache>,
}

impl Default for RefineContext {
    fn default() -> Self {
        Self {
            regex: RegexSettings::default(),
            regex_cache: RefCell::new(RegexPatternCache::default()),
        }
    }
}

impl Clone for RefineContext {
    fn clone(&self) -> Self {
        Self {
            regex: self.regex.clone(),
            regex_cache: RefCell::new(RegexPatternCache::default()),
        }
    }
}

impl RefineContext {
    /// 設定ファイルの内容からコンテキストを生成する
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            regex: config.regex.clone(),
            ..Self::default()
        }
    }

    /// 正規表現設定のみを指定してコンテキストを生成する
    pub fn with_regex(regex: RegexSettings) -> Self {
        Self {
            regex,
            ..Self::default()
        }
    }

    /// 正規表現コンパイルキャッシュへの可変参照を返す
    pub(crate) fn regex_cache_mut(&self) -> std::cell::RefMut<'_, RegexPatternCache> {
        self.regex_cache.borrow_mut()
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::refiner::{RefineMode, Refiner};

    /// `with_regex` で正規表現設定を渡せること
    #[test]
    fn with_regex_applies_pattern() {
        let ctx = RefineContext::with_regex(RegexSettings {
            pattern: r"\s+".to_string(),
            replacement: "-".to_string(),
            ..RegexSettings::default()
        });
        assert_eq!(RefineMode::RegexReplace.refine("a   b", &ctx), "a-b");
    }
}
