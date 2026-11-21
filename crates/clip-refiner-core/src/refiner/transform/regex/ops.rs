use std::borrow::Cow;

use super::cache::RegexPatternCache;
use super::match_timeout::{
    RegexMatchTimeout, extract_cancellable, on_match_timeout, replace_all_cancellable,
    run_with_match_timeout, split_cancellable,
};

use crate::config::RegexSettings;

// ======================================================================
// 置換
// ======================================================================
/// 正規表現に一致する部分を置換文字列へ変換する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターンと置換文字列
///
/// # Returns
/// * `Cow<'_, str>` - 置換後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_replace<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    let replacement = settings.replacement.clone();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &owned_text, &replacement, cancel)
    }) {
        Ok(result) if result == text => Cow::Borrowed(text),
        Ok(result) => Cow::Owned(result),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 抽出
// ======================================================================
/// 正規表現に一致する部分を行単位で抽出する
///
/// 一致がない場合、パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は
/// 元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 抽出結果。変更がない場合は元のテキストを借用
pub(crate) fn regex_extract<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| extract_cancellable(&re, &owned_text, cancel)) {
        Ok(None) => Cow::Borrowed(text),
        Ok(Some(joined)) if joined == text => Cow::Borrowed(text),
        Ok(Some(joined)) => Cow::Owned(joined),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 削除
// ======================================================================
/// 正規表現に一致する部分を削除する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 削除後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_delete<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| {
        replace_all_cancellable(&re, &owned_text, "", cancel)
    }) {
        Ok(result) if result == text => Cow::Borrowed(text),
        Ok(result) => Cow::Owned(result),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// 分割
// ======================================================================
/// 正規表現でテキストを分割し、改行で結合する
///
/// パターンが無効な場合、またはマッチ実行が制限時間を超えた場合は元のテキストを返す
///
/// # Arguments
/// * `text` - 加工対象のテキスト
/// * `settings` - 正規表現パターン
///
/// # Returns
/// * `Cow<'_, str>` - 分割後のテキスト。変更がない場合は元のテキストを借用
pub(crate) fn regex_split<'a>(
    text: &'a str,
    settings: &RegexSettings,
    cache: &mut RegexPatternCache,
) -> Cow<'a, str> {
    let Some(re) = cache.get_or_compile(settings).cloned() else {
        return Cow::Borrowed(text);
    };

    let owned_text = text.to_owned();
    match run_with_match_timeout(move |cancel| split_cancellable(&re, &owned_text, cancel)) {
        Ok(joined) if joined == text => Cow::Borrowed(text),
        Ok(joined) => Cow::Owned(joined),
        Err(RegexMatchTimeout) => on_match_timeout(text),
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::super::match_timeout::with_pool_test_lock;
    use super::{RegexPatternCache, regex_delete, regex_extract, regex_replace, regex_split};

    use crate::config::RegexSettings;

    fn settings(pattern: &str) -> RegexSettings {
        RegexSettings {
            pattern: pattern.to_string(),
            ..RegexSettings::default()
        }
    }

    fn settings_with_replacement(pattern: &str, replacement: &str) -> RegexSettings {
        RegexSettings {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            ..RegexSettings::default()
        }
    }

    fn mut_cache() -> RegexPatternCache {
        RegexPatternCache::default()
    }

    /// 置換の基本動作できるようにすること
    #[test]
    fn test_regex_replace() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_replace("a1b2c", &settings_with_replacement(r"\d", "X"), &mut cache),
                "aXbXc"
            );
            assert_eq!(
                regex_replace("plain", &settings(r"\d"), &mut cache),
                "plain"
            );
        });
    }

    /// キャプチャグループを使った置換できるようにすること
    #[test]
    fn test_regex_replace_capture() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_replace(
                    "2024-01-02",
                    &settings_with_replacement(r"(\d+)-(\d+)", r"$2/$1"),
                    &mut cache
                ),
                "01/2024-02"
            );
        });
    }

    /// 抽出の基本動作できるようにすること
    #[test]
    fn test_regex_extract() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(
                regex_extract("a1b22c", &settings(r"\d+"), &mut cache),
                "1\n22"
            );
            assert_eq!(regex_extract("abc", &settings(r"\d+"), &mut cache), "abc");
        });
    }

    /// 削除の基本動作できるようにすること
    #[test]
    fn test_regex_delete() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(regex_delete("a1b2c", &settings(r"\d"), &mut cache), "abc");
        });
    }

    /// 分割の基本動作できるようにすること
    #[test]
    fn test_regex_split() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert_eq!(regex_split("a,b,c", &settings(r","), &mut cache), "a\nb\nc");
            assert_eq!(
                regex_split("a  b   c", &settings(r"\s+"), &mut cache),
                "a\nb\nc"
            );
        });
    }

    /// 無効なパターンは元のテキストを返すようにすること
    #[test]
    fn test_invalid_pattern_returns_original() {
        with_pool_test_lock(|| {
            let invalid = RegexSettings {
                pattern: "[unclosed".to_string(),
                ..RegexSettings::default()
            };
            let mut cache = mut_cache();
            assert!(matches!(
                regex_replace("text", &invalid, &mut cache),
                Cow::Borrowed(_)
            ));
        });
    }

    /// 空パターンは元のテキストを返すようにすること
    #[test]
    fn test_empty_pattern_returns_original() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();
            assert!(matches!(
                regex_replace("text", &RegexSettings::default(), &mut cache),
                Cow::Borrowed(_)
            ));
        });
    }

    /// 大文字小文字を無視するフラグできるようにすること
    #[test]
    fn test_case_insensitive() {
        with_pool_test_lock(|| {
            let settings = RegexSettings {
                pattern: "abc".to_string(),
                replacement: "X".to_string(),
                case_insensitive: true,
                ..RegexSettings::default()
            };
            let mut cache = mut_cache();
            assert_eq!(regex_replace("AbC", &settings, &mut cache), "X");
        });
    }

    /// 設定変更時はキャッシュを無効化すること
    #[test]
    fn test_regex_cache_invalidates_on_settings_change() {
        with_pool_test_lock(|| {
            let mut cache = mut_cache();

            assert_eq!(regex_extract("a1b", &settings(r"\d"), &mut cache), "1");
            assert_eq!(
                regex_extract("a1b", &settings(r"[a-z]"), &mut cache),
                "a\nb"
            );
        });
    }
}
