use std::cell::{Cell, RefCell};

use super::parse_failure::ParseFailure;
use super::transform::regex::RegexPatternCache;
#[cfg(feature = "config")]
use crate::config::AppConfig;
use crate::config::RegexSettings;

// ======================================================================
// 加工スキップ理由
// ======================================================================
/// 変換が変更なし相当で打ち切られた理由
///
/// パイプライン全体が未変更のとき、`ClipboardProcessError` へ昇格する
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefineSkipReason {
    /// 正規表現マッチが制限時間を超過した
    RegexTimedOut,
    /// JSON / YAML / Markdown パーサー入力が上限を超えた
    ParserInputTooLarge,
    /// HTML の要素の入れ子が上限を超えた
    HtmlNestingTooDeep,
    /// JSON / YAML を読み取れない、または変換できなかった
    ParseFailed(ParseFailure),
}

// ======================================================================
// 加工コンテキスト
// ======================================================================
/// 設定依存の変換モード向けコンテキスト
///
/// 正規表現パターンなど、テキスト以外に必要なパラメータを保持する
/// 正規表現のコンパイル結果は `regex_cache` にキャッシュされる
#[derive(Debug)]
pub struct RefineContext {
    /// 正規表現加工用の設定
    pub regex: RegexSettings,
    regex_cache: RefCell<RegexPatternCache>,
    /// 直近の加工で検出したスキップ理由 (先勝ち)
    skip_reason: Cell<Option<RefineSkipReason>>,
}

impl Default for RefineContext {
    fn default() -> Self {
        Self {
            regex: RegexSettings::default(),
            regex_cache: RefCell::new(RegexPatternCache::default()),
            skip_reason: Cell::new(None),
        }
    }
}

impl Clone for RefineContext {
    fn clone(&self) -> Self {
        Self {
            regex: self.regex.clone(),
            regex_cache: RefCell::new(RegexPatternCache::default()),
            skip_reason: Cell::new(None),
        }
    }
}

impl RefineContext {
    /// 設定ファイルの内容からコンテキストを生成する
    ///
    /// feature `config` が必要
    ///
    /// # Arguments
    /// * `config` - アプリケーション設定
    ///
    /// # Returns
    /// * `RefineContext` - 設定から構築した加工コンテキスト
    #[cfg(feature = "config")]
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            regex: config.regex.clone(),
            ..Self::default()
        }
    }

    /// 正規表現設定のみを指定してコンテキストを生成する
    ///
    /// # Arguments
    /// * `regex` - 正規表現加工用の設定
    ///
    /// # Returns
    /// * `RefineContext` - 指定した正規表現設定を持つ加工コンテキスト
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

    /// スキップ理由を記録する (未記録のときのみ)
    ///
    /// # Arguments
    /// * `reason` - 記録するスキップ理由
    pub(crate) fn record_skip(&self, reason: RefineSkipReason) {
        if self.skip_reason.get().is_none() {
            self.skip_reason.set(Some(reason));
        }
    }

    /// 記録済みのスキップ理由を取り出し、状態をクリアする
    ///
    /// # Returns
    /// * `Option<RefineSkipReason>` - 記録があればその理由
    pub(crate) fn take_skip_reason(&self) -> Option<RefineSkipReason> {
        self.skip_reason.take()
    }

    /// スキップ理由をクリアする
    pub(crate) fn clear_skip_reason(&self) {
        self.skip_reason.set(None);
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::refiner::{RefineMode, Refiner};

    /// `with_regex` で正規表現設定を渡せるようにすること
    #[test]
    fn with_regex_applies_pattern() {
        let ctx = RefineContext::with_regex(RegexSettings::single(r"\s+", "-"));
        assert_eq!(RefineMode::RegexReplace.refine("a   b", &ctx), "a-b");
    }

    /// スキップ理由は先勝ちで記録し、取り出し後は空になるようにすること
    #[test]
    fn skip_reason_is_first_wins_and_take_clears() {
        let ctx = RefineContext::default();
        ctx.record_skip(RefineSkipReason::RegexTimedOut);
        ctx.record_skip(RefineSkipReason::ParserInputTooLarge);
        assert_eq!(
            ctx.take_skip_reason(),
            Some(RefineSkipReason::RegexTimedOut)
        );
        assert_eq!(ctx.take_skip_reason(), None);
    }
}
