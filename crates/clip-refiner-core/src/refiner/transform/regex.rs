use std::borrow::Cow;

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
// 置換
// ======================================================================
/// 正規表現に一致する部分を置換文字列へ変換する
///
/// パターンが無効な場合は元のテキストを返す
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
    let Some(re) = cache.get_or_compile(settings) else {
        return Cow::Borrowed(text);
    };

    let result = re.replace_all(text, settings.replacement.as_str());
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result.into_owned())
    }
}

// ======================================================================
// 抽出
// ======================================================================
/// 正規表現に一致する部分を行単位で抽出する
///
/// 一致がない場合、またはパターンが無効な場合は元のテキストを返す
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
    let Some(re) = cache.get_or_compile(settings) else {
        return Cow::Borrowed(text);
    };

    let matches: Vec<&str> = re.find_iter(text).map(|m| m.as_str()).collect();
    if matches.is_empty() {
        return Cow::Borrowed(text);
    }

    let joined = matches.join("\n");
    if joined == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(joined)
    }
}

// ======================================================================
// 削除
// ======================================================================
/// 正規表現に一致する部分を削除する
///
/// パターンが無効な場合は元のテキストを返す
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
    let Some(re) = cache.get_or_compile(settings) else {
        return Cow::Borrowed(text);
    };

    let result = re.replace_all(text, "");
    if result == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(result.into_owned())
    }
}

// ======================================================================
// 分割
// ======================================================================
/// 正規表現でテキストを分割し、改行で結合する
///
/// パターンが無効な場合は元のテキストを返す
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
    let Some(re) = cache.get_or_compile(settings) else {
        return Cow::Borrowed(text);
    };

    let parts: Vec<&str> = re.split(text).collect();
    let joined = parts.join("\n");
    if joined == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(joined)
    }
}

// ======================================================================
// テスト
// ======================================================================
#[cfg(test)]
mod tests {
    use super::*;

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

    /// 置換の基本動作
    #[test]
    fn test_regex_replace() {
        let mut cache = mut_cache();
        assert_eq!(
            regex_replace("a1b2c", &settings_with_replacement(r"\d", "X"), &mut cache),
            "aXbXc"
        );
        assert_eq!(
            regex_replace("plain", &settings(r"\d"), &mut cache),
            "plain"
        );
    }

    /// キャプチャグループを使った置換
    #[test]
    fn test_regex_replace_capture() {
        let mut cache = mut_cache();
        assert_eq!(
            regex_replace(
                "2024-01-02",
                &settings_with_replacement(r"(\d+)-(\d+)", r"$2/$1"),
                &mut cache
            ),
            "01/2024-02"
        );
    }

    /// 抽出の基本動作
    #[test]
    fn test_regex_extract() {
        let mut cache = mut_cache();
        assert_eq!(
            regex_extract("a1b22c", &settings(r"\d+"), &mut cache),
            "1\n22"
        );
        assert_eq!(regex_extract("abc", &settings(r"\d+"), &mut cache), "abc");
    }

    /// 削除の基本動作
    #[test]
    fn test_regex_delete() {
        let mut cache = mut_cache();
        assert_eq!(regex_delete("a1b2c", &settings(r"\d"), &mut cache), "abc");
    }

    /// 分割の基本動作
    #[test]
    fn test_regex_split() {
        let mut cache = mut_cache();
        assert_eq!(regex_split("a,b,c", &settings(r","), &mut cache), "a\nb\nc");
        assert_eq!(
            regex_split("a  b   c", &settings(r"\s+"), &mut cache),
            "a\nb\nc"
        );
    }

    /// 無効なパターンは元のテキストを返すこと
    #[test]
    fn test_invalid_pattern_returns_original() {
        let invalid = RegexSettings {
            pattern: "[unclosed".to_string(),
            ..RegexSettings::default()
        };
        let mut cache = mut_cache();
        assert!(matches!(
            regex_replace("text", &invalid, &mut cache),
            Cow::Borrowed(_)
        ));
    }

    /// 空パターンは元のテキストを返すこと
    #[test]
    fn test_empty_pattern_returns_original() {
        let mut cache = mut_cache();
        assert!(matches!(
            regex_replace("text", &RegexSettings::default(), &mut cache),
            Cow::Borrowed(_)
        ));
    }

    /// 大文字小文字を無視するフラグ
    #[test]
    fn test_case_insensitive() {
        let settings = RegexSettings {
            pattern: "abc".to_string(),
            replacement: "X".to_string(),
            case_insensitive: true,
            ..RegexSettings::default()
        };
        let mut cache = mut_cache();
        assert_eq!(regex_replace("AbC", &settings, &mut cache), "X");
    }

    /// 同一設定ではコンパイル結果を再利用すること
    #[test]
    fn test_regex_cache_reuses_compiled_pattern() {
        let settings = settings(r"\d+");
        let mut cache = mut_cache();

        let first = cache.get_or_compile(&settings).map(std::ptr::from_ref);
        let second = cache.get_or_compile(&settings).map(std::ptr::from_ref);

        assert_eq!(first, second);
    }

    /// 設定変更時はキャッシュを無効化すること
    #[test]
    fn test_regex_cache_invalidates_on_settings_change() {
        let mut cache = mut_cache();

        assert_eq!(regex_extract("a1b", &settings(r"\d"), &mut cache), "1");
        assert_eq!(
            regex_extract("a1b", &settings(r"[a-z]"), &mut cache),
            "a\nb"
        );
    }
}
