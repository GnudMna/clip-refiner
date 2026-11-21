use std::borrow::Cow;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::config::RegexSettings;
use crate::consts::{MAX_REGEX_PATTERN_BYTES, REGEX_MATCH_TIMEOUT_MS};

use regex::{Regex, RegexBuilder};
use tracing::warn;

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
// マッチ実行タイムアウト
// ======================================================================
/// 正規表現マッチ操作のタイムアウト
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegexMatchTimeout;

/// 正規表現マッチを制限時間内で実行する
///
/// 制限時間を超えた場合は `Err(RegexMatchTimeout)` を返し、実行中のスレッドは
/// 完了までバックグラウンドで残る (打ち切り不可のため)
fn run_with_match_timeout<T, F>(op: F) -> Result<T, RegexMatchTimeout>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    run_with_match_timeout_duration(Duration::from_millis(REGEX_MATCH_TIMEOUT_MS), op)
}

/// 指定した制限時間で正規表現マッチを実行する
fn run_with_match_timeout_duration<T, F>(timeout: Duration, op: F) -> Result<T, RegexMatchTimeout>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("clip-refiner-regex".into())
        .spawn(move || {
            let _ = tx.send(op());
        })
        .map_err(|_| RegexMatchTimeout)?;

    match rx.recv_timeout(timeout) {
        Ok(value) => Ok(value),
        Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
            Err(RegexMatchTimeout)
        }
    }
}

/// タイムアウト時に警告を出し、元テキストを返す
fn on_match_timeout(text: &str) -> Cow<'_, str> {
    warn!(
        timeout_ms = REGEX_MATCH_TIMEOUT_MS,
        "正規表現の実行がタイムアウトしたため元のテキストを維持する"
    );
    Cow::Borrowed(text)
}

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
            match run_with_match_timeout(move || {
                re.replace_all(&sample_owned, replacement.as_str())
                    .into_owned()
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
    match run_with_match_timeout(move || {
        re.replace_all(&owned_text, replacement.as_str())
            .into_owned()
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
    match run_with_match_timeout(move || {
        let matches: Vec<&str> = re.find_iter(&owned_text).map(|m| m.as_str()).collect();
        if matches.is_empty() {
            None
        } else {
            Some(matches.join("\n"))
        }
    }) {
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
    match run_with_match_timeout(move || re.replace_all(&owned_text, "").into_owned()) {
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
    match run_with_match_timeout(move || {
        let parts: Vec<&str> = re.split(&owned_text).collect();
        parts.join("\n")
    }) {
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

    /// 置換の基本動作できるようにすること
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

    /// キャプチャグループを使った置換できるようにすること
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

    /// 抽出の基本動作できるようにすること
    #[test]
    fn test_regex_extract() {
        let mut cache = mut_cache();
        assert_eq!(
            regex_extract("a1b22c", &settings(r"\d+"), &mut cache),
            "1\n22"
        );
        assert_eq!(regex_extract("abc", &settings(r"\d+"), &mut cache), "abc");
    }

    /// 削除の基本動作できるようにすること
    #[test]
    fn test_regex_delete() {
        let mut cache = mut_cache();
        assert_eq!(regex_delete("a1b2c", &settings(r"\d"), &mut cache), "abc");
    }

    /// 分割の基本動作できるようにすること
    #[test]
    fn test_regex_split() {
        let mut cache = mut_cache();
        assert_eq!(regex_split("a,b,c", &settings(r","), &mut cache), "a\nb\nc");
        assert_eq!(
            regex_split("a  b   c", &settings(r"\s+"), &mut cache),
            "a\nb\nc"
        );
    }

    /// 無効なパターンは元のテキストを返すようにすること
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

    /// 空パターンは元のテキストを返すようにすること
    #[test]
    fn test_empty_pattern_returns_original() {
        let mut cache = mut_cache();
        assert!(matches!(
            regex_replace("text", &RegexSettings::default(), &mut cache),
            Cow::Borrowed(_)
        ));
    }

    /// 大文字小文字を無視するフラグできるようにすること
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

    /// プレビューが置換結果とエラーを返すようにすること
    #[test]
    fn test_preview_regex_replace() {
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
    }

    /// 制限時間内に終わらないマッチはタイムアウトすること
    #[test]
    fn test_run_with_match_timeout_expires() {
        let result = run_with_match_timeout_duration(Duration::from_millis(30), || {
            thread::sleep(Duration::from_millis(300));
            "done"
        });
        assert_eq!(result, Err(RegexMatchTimeout));
    }

    /// 制限時間内に終わるマッチは結果を返すこと
    #[test]
    fn test_run_with_match_timeout_completes() {
        let result = run_with_match_timeout_duration(Duration::from_millis(500), || {
            thread::sleep(Duration::from_millis(10));
            42
        });
        assert_eq!(result, Ok(42));
    }

    /// タイムアウト時は置換せず元テキストを返すこと
    #[test]
    fn test_regex_replace_timeout_keeps_original() {
        // ヘルパー経由で短い制限時間を検証し、本番パスは on_match_timeout と同じ挙動
        let text = "keep-me";
        let timed_out = run_with_match_timeout_duration(Duration::from_millis(20), || {
            thread::sleep(Duration::from_millis(200));
            "replaced".to_string()
        });
        assert!(timed_out.is_err());
        assert!(matches!(on_match_timeout(text), Cow::Borrowed("keep-me")));
    }
}
